#!/usr/bin/env bash
set -euo pipefail

MODEL_NAME="${1:-qwen3:8b}"

pass() { printf "[PASS] %s\n" "$1"; }
warn() { printf "[WARN] %s\n" "$1"; }
fail() { printf "[FAIL] %s\n" "$1"; }

require_cmd() {
  local cmd="$1"
  if command -v "$cmd" >/dev/null 2>&1; then
    pass "Found '$cmd' ($("$cmd" --version 2>/dev/null | head -n 1 || true))"
  else
    fail "Missing required command: $cmd"
    return 1
  fi
}

printf "== Mac Preflight: AI Transcribe Local ==\n"
printf "Timestamp: %s\n" "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
printf "Repo: %s\n" "$(pwd)"
printf "Target model: %s\n\n" "$MODEL_NAME"

hard_fail=0

for cmd in node npm rustc cargo ffmpeg ollama; do
  if ! require_cmd "$cmd"; then
    hard_fail=1
  fi
done

if command -v ffprobe >/dev/null 2>&1; then
  pass "Found optional command 'ffprobe'"
else
  warn "Optional command missing: ffprobe (duration display will be limited)"
fi

if command -v whisper-cli >/dev/null 2>&1; then
  pass "Found transcription command 'whisper-cli'"
elif command -v whisper >/dev/null 2>&1; then
  pass "Found transcription command 'whisper'"
else
  fail "Missing transcription command: install 'whisper-cli' or 'whisper'"
  hard_fail=1
fi

printf "\n== Runtime checks ==\n"

if command -v ollama >/dev/null 2>&1; then
  if curl -fsS http://127.0.0.1:11434/api/tags >/tmp/ai_transcribe_ollama_tags.json 2>/dev/null; then
    pass "Ollama API reachable at http://127.0.0.1:11434"
    if grep -F "\"name\":\"${MODEL_NAME}\"" /tmp/ai_transcribe_ollama_tags.json >/dev/null 2>&1; then
      pass "Model '${MODEL_NAME}' is available locally"
    else
      fail "Model '${MODEL_NAME}' not found. Run: ollama pull ${MODEL_NAME}"
      hard_fail=1
    fi
  else
    fail "Ollama API not reachable. Start Ollama before testing."
    hard_fail=1
  fi
fi

if command -v ffmpeg >/dev/null 2>&1; then
  if ffmpeg -f avfoundation -list_devices true -i "" >/tmp/ai_transcribe_ffmpeg_devices.log 2>&1; then
    pass "ffmpeg avfoundation device probe completed"
  else
    # ffmpeg usually exits non-zero for probe while still printing devices.
    if grep -E "AVFoundation|\[[0-9]+\]" /tmp/ai_transcribe_ffmpeg_devices.log >/dev/null 2>&1; then
      pass "ffmpeg avfoundation listed devices (non-zero probe exit is expected)"
    else
      warn "Could not parse avfoundation devices from ffmpeg output"
    fi
  fi

  if grep -i "BlackHole" /tmp/ai_transcribe_ffmpeg_devices.log >/dev/null 2>&1; then
    pass "BlackHole device detected"
  else
    warn "BlackHole device not detected. Install/configure loopback for system-call capture."
  fi
fi

printf "\n== Result ==\n"
if [ "$hard_fail" -eq 0 ]; then
  pass "Preflight passed. You can run the full smoke test."
  exit 0
fi

fail "Preflight failed. Fix required items before smoke testing."
exit 1
