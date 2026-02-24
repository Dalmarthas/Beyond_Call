import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

enum RecorderError: Error {
    case missingOutputPath
    case unsupportedMacOS
    case noDisplayFound
    case writerInputRejected
    case invalidMicrophoneArguments
}

final class SystemAudioRecorder: NSObject, SCStreamOutput {
    private let outputURL: URL
    private let microphoneOutputURL: URL?
    private let microphoneDeviceID: String?
    private let sampleHandlerQueue = DispatchQueue(label: "com.niberium.ai-transcribe.sck-audio")
    private var stream: SCStream?
    private var systemWriter: AVAssetWriter?
    private var systemWriterInput: AVAssetWriterInput?
    private var microphoneWriter: AVAssetWriter?
    private var microphoneWriterInput: AVAssetWriterInput?
    private var stopContinuation: CheckedContinuation<Void, Never>?
    private var stopRequested = false
    private var startedAt = Date()
    private var lastTelemetry = Date.distantPast
    private var smoothedLevel: Float = 0
    private var hasReceivedSystemSample = false
    private var hasReceivedMicrophoneSample = false
    private var emittedNoSystemSampleError = false
    private var emittedNoMicrophoneSampleError = false

    var capturesMicrophone: Bool {
        microphoneOutputURL != nil
    }

    init(outputURL: URL, microphoneOutputURL: URL?, microphoneDeviceID: String?) {
        self.outputURL = outputURL
        self.microphoneOutputURL = microphoneOutputURL
        self.microphoneDeviceID = microphoneDeviceID
        super.init()
    }

    func run() async throws {
        guard #available(macOS 13.0, *) else {
            throw RecorderError.unsupportedMacOS
        }
        if capturesMicrophone, #unavailable(macOS 15.0) {
            throw RecorderError.unsupportedMacOS
        }

        let parentDir = outputURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
        if FileManager.default.fileExists(atPath: outputURL.path) {
            try FileManager.default.removeItem(at: outputURL)
        }
        if let microphoneOutputURL {
            let parentDir = microphoneOutputURL.deletingLastPathComponent()
            try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
            if FileManager.default.fileExists(atPath: microphoneOutputURL.path) {
                try FileManager.default.removeItem(at: microphoneOutputURL)
            }
        }

        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
        guard let display = content.displays.first else {
            throw RecorderError.noDisplayFound
        }

        let filter = SCContentFilter(display: display, excludingWindows: [])
        let configuration = SCStreamConfiguration()
        configuration.capturesAudio = true
        configuration.excludesCurrentProcessAudio = false
        configuration.sampleRate = 16_000
        configuration.channelCount = 1
        configuration.queueDepth = 8
        if capturesMicrophone, #available(macOS 15.0, *) {
            configuration.captureMicrophone = true
            if let microphoneDeviceID, !microphoneDeviceID.isEmpty {
                configuration.microphoneCaptureDeviceID = microphoneDeviceID
            } else {
                configuration.microphoneCaptureDeviceID = AVCaptureDevice.default(for: .audio)?.uniqueID
            }
        }

        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: sampleHandlerQueue)
        if capturesMicrophone, #available(macOS 15.0, *) {
            try stream.addStreamOutput(self, type: .microphone, sampleHandlerQueue: sampleHandlerQueue)
        }
        self.stream = stream

        monitorStdinForStopSignal()
        startedAt = Date()
        try await stream.startCapture()
        await waitUntilStopRequested()
        try await stream.stopCapture()
        await finalizeWriter()
        emitTelemetry(force: true)
    }

    nonisolated func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard CMSampleBufferIsValid(sampleBuffer) else {
            return
        }

        switch outputType {
        case .audio:
            hasReceivedSystemSample = true
            if systemWriter == nil {
                do {
                    try initializeSystemWriter(with: sampleBuffer)
                } catch {
                    fputs("sck_error=Failed to initialize system writer: \(error)\n", stderr)
                    fflush(stderr)
                    requestStop()
                    return
                }
            }
            appendToSystemWriter(sampleBuffer)
        case .microphone:
            hasReceivedMicrophoneSample = true
            if microphoneWriter == nil {
                do {
                    try initializeMicrophoneWriter(with: sampleBuffer)
                } catch {
                    fputs("sck_error=Failed to initialize microphone writer: \(error)\n", stderr)
                    fflush(stderr)
                    requestStop()
                    return
                }
            }
            appendToMicrophoneWriter(sampleBuffer)
        default:
            break
        }

        let level = computeRmsLevel(from: sampleBuffer)
        smoothedLevel = (smoothedLevel * 0.75 + level * 0.25).clamped(to: 0...1)
        emitTelemetry(force: false)
    }

    private func makeAudioWriterInput() -> AVAssetWriterInput {
        let outputSettings: [String: Any] = [
            AVFormatIDKey: kAudioFormatLinearPCM,
            AVSampleRateKey: 16_000,
            AVNumberOfChannelsKey: 1,
            AVLinearPCMBitDepthKey: 16,
            AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false,
            AVLinearPCMIsNonInterleaved: false
        ]
        let input = AVAssetWriterInput(
            mediaType: .audio,
            outputSettings: outputSettings
        )
        input.expectsMediaDataInRealTime = true
        return input
    }

    private func initializeSystemWriter(with sampleBuffer: CMSampleBuffer) throws {
        _ = CMSampleBufferGetFormatDescription(sampleBuffer)

        let writer = try AVAssetWriter(outputURL: outputURL, fileType: .wav)
        let input = makeAudioWriterInput()
        guard writer.canAdd(input) else {
            throw RecorderError.writerInputRejected
        }
        writer.add(input)

        guard writer.startWriting() else {
            throw writer.error ?? RecorderError.writerInputRejected
        }

        let startTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        writer.startSession(atSourceTime: startTime)
        self.systemWriter = writer
        self.systemWriterInput = input
    }

    private func initializeMicrophoneWriter(with sampleBuffer: CMSampleBuffer) throws {
        guard let microphoneOutputURL else {
            throw RecorderError.invalidMicrophoneArguments
        }
        _ = CMSampleBufferGetFormatDescription(sampleBuffer)

        let writer = try AVAssetWriter(outputURL: microphoneOutputURL, fileType: .wav)
        let input = makeAudioWriterInput()
        guard writer.canAdd(input) else {
            throw RecorderError.writerInputRejected
        }
        writer.add(input)

        guard writer.startWriting() else {
            throw writer.error ?? RecorderError.writerInputRejected
        }

        let startTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        writer.startSession(atSourceTime: startTime)
        self.microphoneWriter = writer
        self.microphoneWriterInput = input
    }

    private func appendToSystemWriter(_ sampleBuffer: CMSampleBuffer) {
        guard let systemWriterInput else {
            return
        }
        if systemWriterInput.isReadyForMoreMediaData {
            if !systemWriterInput.append(sampleBuffer) {
                let details = systemWriter?.error.map { "\($0)" } ?? "unknown append failure"
                fputs("sck_error=Failed to append system audio sample: \(details)\n", stderr)
                fflush(stderr)
                requestStop()
            }
        }
    }

    private func appendToMicrophoneWriter(_ sampleBuffer: CMSampleBuffer) {
        guard let microphoneWriterInput else {
            return
        }
        if microphoneWriterInput.isReadyForMoreMediaData {
            if !microphoneWriterInput.append(sampleBuffer) {
                let details = microphoneWriter?.error.map { "\($0)" } ?? "unknown append failure"
                fputs("sck_error=Failed to append microphone sample: \(details)\n", stderr)
                fflush(stderr)
                requestStop()
            }
        }
    }

    private func monitorStdinForStopSignal() {
        FileHandle.standardInput.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else {
                self?.requestStop()
                return
            }
            if let text = String(data: data, encoding: .utf8), text.contains("q") {
                self?.requestStop()
            }
        }
    }

    private func waitUntilStopRequested() async {
        if stopRequested {
            return
        }

        await withCheckedContinuation { continuation in
            stopContinuation = continuation
        }
    }

    private func requestStop() {
        if stopRequested {
            return
        }
        stopRequested = true
        FileHandle.standardInput.readabilityHandler = nil
        stopContinuation?.resume()
        stopContinuation = nil
    }

    private func finalizeWriter() async {
        if let systemWriter, let systemWriterInput {
            systemWriterInput.markAsFinished()
            await withCheckedContinuation { continuation in
                systemWriter.finishWriting {
                    continuation.resume()
                }
            }
        }
        if let microphoneWriter, let microphoneWriterInput {
            microphoneWriterInput.markAsFinished()
            await withCheckedContinuation { continuation in
                microphoneWriter.finishWriting {
                    continuation.resume()
                }
            }
        }
    }

    private func emitTelemetry(force: Bool) {
        let now = Date()
        if !force && now.timeIntervalSince(lastTelemetry) < 0.20 {
            return
        }
        lastTelemetry = now

        if !hasReceivedSystemSample && !emittedNoSystemSampleError && now.timeIntervalSince(startedAt) > 2.5 {
            emittedNoSystemSampleError = true
            fputs("sck_error=No audio samples received from ScreenCaptureKit. Check Screen & System Audio Recording permission and make sure app/call audio is actively playing.\n", stderr)
        }
        if capturesMicrophone && !hasReceivedMicrophoneSample && !emittedNoMicrophoneSampleError && now.timeIntervalSince(startedAt) > 2.5 {
            emittedNoMicrophoneSampleError = true
            fputs("sck_error=No microphone samples received from ScreenCaptureKit. Check microphone permission and input device.\n", stderr)
        }

        let fileSize = (try? FileManager.default.attributesOfItem(atPath: outputURL.path)[.size] as? NSNumber)?
            .uint64Value ?? 0
        let micros = UInt64(now.timeIntervalSince(startedAt) * 1_000_000)
        fputs("total_size=\(fileSize)\n", stderr)
        fputs("out_time_us=\(micros)\n", stderr)
        fputs("level=\(smoothedLevel)\n", stderr)
        if let microphoneOutputURL {
            let micFileSize = (try? FileManager.default.attributesOfItem(atPath: microphoneOutputURL.path)[.size] as? NSNumber)?
                .uint64Value ?? 0
            fputs("microphone_total_size=\(micFileSize)\n", stderr)
        }
        fflush(stderr)
    }

    private func computeRmsLevel(from sampleBuffer: CMSampleBuffer) -> Float {
        guard let formatDescription = CMSampleBufferGetFormatDescription(sampleBuffer),
              let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(formatDescription)
        else {
            return 0
        }

        var blockBuffer: CMBlockBuffer?
        var audioBufferList = AudioBufferList(
            mNumberBuffers: 1,
            mBuffers: AudioBuffer(mNumberChannels: 0, mDataByteSize: 0, mData: nil)
        )

        let status = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
            sampleBuffer,
            bufferListSizeNeededOut: nil,
            bufferListOut: &audioBufferList,
            bufferListSize: MemoryLayout<AudioBufferList>.size,
            blockBufferAllocator: nil,
            blockBufferMemoryAllocator: nil,
            flags: kCMSampleBufferFlag_AudioBufferList_Assure16ByteAlignment,
            blockBufferOut: &blockBuffer
        )
        guard status == noErr else {
            return 0
        }

        let asbd = asbdPtr.pointee
        let isFloat = (asbd.mFormatFlags & kAudioFormatFlagIsFloat) != 0
        let isSignedInt = (asbd.mFormatFlags & kAudioFormatFlagIsSignedInteger) != 0
        let bitsPerChannel = Int(asbd.mBitsPerChannel)

        var accum: Float = 0
        var sampleCount = 0
        let buffers = UnsafeMutableAudioBufferListPointer(&audioBufferList)

        for buffer in buffers {
            guard let data = buffer.mData else {
                continue
            }
            let byteCount = Int(buffer.mDataByteSize)
            if isFloat && bitsPerChannel == 32 {
                let count = byteCount / MemoryLayout<Float>.size
                let samples = data.bindMemory(to: Float.self, capacity: count)
                for idx in 0..<count {
                    let sample = samples[idx]
                    accum += sample * sample
                }
                sampleCount += count
            } else if isSignedInt && bitsPerChannel == 16 {
                let count = byteCount / MemoryLayout<Int16>.size
                let samples = data.bindMemory(to: Int16.self, capacity: count)
                for idx in 0..<count {
                    let sample = Float(samples[idx]) / 32768.0
                    accum += sample * sample
                }
                sampleCount += count
            }
        }

        if sampleCount == 0 {
            return 0
        }

        let rms = sqrt(accum / Float(sampleCount))
        return min(1, rms * 2.8)
    }
}

private func parseOutputPath() throws -> String {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--output"), args.count > index + 1 else {
        throw RecorderError.missingOutputPath
    }
    return args[index + 1]
}

private func parseMicrophoneOutputPath() -> String? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--microphone-output"), args.count > index + 1 else {
        return nil
    }
    return args[index + 1]
}

private func parseMicrophoneDeviceID() -> String? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--microphone-device-id"), args.count > index + 1 else {
        return nil
    }
    return args[index + 1]
}

private func parseCapturesMicrophone() -> Bool {
    CommandLine.arguments.contains("--with-microphone")
}

extension Comparable {
    fileprivate func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}

@main
struct Main {
    static func main() async {
        do {
            let outputPath = try parseOutputPath()
            let capturesMicrophone = parseCapturesMicrophone()
            let microphonePath = parseMicrophoneOutputPath()
            if capturesMicrophone && microphonePath == nil {
                throw RecorderError.invalidMicrophoneArguments
            }
            let recorder = SystemAudioRecorder(
                outputURL: URL(fileURLWithPath: outputPath),
                microphoneOutputURL: microphonePath.map { URL(fileURLWithPath: $0) },
                microphoneDeviceID: parseMicrophoneDeviceID()
            )
            try await recorder.run()
        } catch {
            fputs("sck_error=\(error)\n", stderr)
            fflush(stderr)
            exit(1)
        }
    }
}
