import AVFoundation
import CoreMedia
import Foundation
import ScreenCaptureKit

enum RecorderError: Error {
    case missingOutputPath
    case unsupportedMacOS
    case noDisplayFound
    case writerInputRejected
}

final class SystemAudioRecorder: NSObject, SCStreamOutput {
    private let outputURL: URL
    private let sampleHandlerQueue = DispatchQueue(label: "com.niberium.ai-transcribe.sck-audio")
    private var stream: SCStream?
    private var writer: AVAssetWriter?
    private var writerInput: AVAssetWriterInput?
    private var stopContinuation: CheckedContinuation<Void, Never>?
    private var stopRequested = false
    private var startedAt = Date()
    private var lastTelemetry = Date.distantPast
    private var smoothedLevel: Float = 0
    private var hasReceivedAudioSample = false
    private var emittedNoSampleError = false

    init(outputURL: URL) {
        self.outputURL = outputURL
        super.init()
    }

    func run() async throws {
        guard #available(macOS 13.0, *) else {
            throw RecorderError.unsupportedMacOS
        }

        let parentDir = outputURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: parentDir, withIntermediateDirectories: true)
        if FileManager.default.fileExists(atPath: outputURL.path) {
            try FileManager.default.removeItem(at: outputURL)
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

        let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
        try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: sampleHandlerQueue)
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
        guard outputType == .audio else {
            return
        }
        guard CMSampleBufferIsValid(sampleBuffer) else {
            return
        }
        hasReceivedAudioSample = true

        if writer == nil {
            do {
                try initializeWriter(with: sampleBuffer)
            } catch {
                fputs("sck_error=Failed to initialize writer: \(error)\n", stderr)
                fflush(stderr)
                requestStop()
                return
            }
        }

        guard let writerInput else {
            return
        }

        if writerInput.isReadyForMoreMediaData {
            if !writerInput.append(sampleBuffer) {
                let details = writer?.error.map { "\($0)" } ?? "unknown append failure"
                fputs("sck_error=Failed to append audio sample to writer: \(details)\n", stderr)
                fflush(stderr)
                requestStop()
                return
            }
        }

        let level = computeRmsLevel(from: sampleBuffer)
        smoothedLevel = (smoothedLevel * 0.75 + level * 0.25).clamped(to: 0...1)
        emitTelemetry(force: false)
    }

    private func initializeWriter(with sampleBuffer: CMSampleBuffer) throws {
        _ = CMSampleBufferGetFormatDescription(sampleBuffer)

        let writer = try AVAssetWriter(outputURL: outputURL, fileType: .wav)
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
        guard writer.canAdd(input) else {
            throw RecorderError.writerInputRejected
        }
        writer.add(input)

        guard writer.startWriting() else {
            throw writer.error ?? RecorderError.writerInputRejected
        }

        let startTime = CMSampleBufferGetPresentationTimeStamp(sampleBuffer)
        writer.startSession(atSourceTime: startTime)
        self.writer = writer
        self.writerInput = input
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
        guard let writer, let writerInput else {
            return
        }
        writerInput.markAsFinished()
        await withCheckedContinuation { continuation in
            writer.finishWriting {
                continuation.resume()
            }
        }
    }

    private func emitTelemetry(force: Bool) {
        let now = Date()
        if !force && now.timeIntervalSince(lastTelemetry) < 0.20 {
            return
        }
        lastTelemetry = now

        if !hasReceivedAudioSample && !emittedNoSampleError && now.timeIntervalSince(startedAt) > 2.5 {
            emittedNoSampleError = true
            fputs("sck_error=No audio samples received from ScreenCaptureKit. Check Screen & System Audio Recording permission and make sure app/call audio is actively playing.\n", stderr)
        }

        let fileSize = (try? FileManager.default.attributesOfItem(atPath: outputURL.path)[.size] as? NSNumber)?
            .uint64Value ?? 0
        let micros = UInt64(now.timeIntervalSince(startedAt) * 1_000_000)
        fputs("total_size=\(fileSize)\n", stderr)
        fputs("out_time_us=\(micros)\n", stderr)
        fputs("level=\(smoothedLevel)\n", stderr)
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
            let recorder = SystemAudioRecorder(outputURL: URL(fileURLWithPath: outputPath))
            try await recorder.run()
        } catch {
            fputs("sck_error=\(error)\n", stderr)
            fflush(stderr)
            exit(1)
        }
    }
}
