#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${1:-models}"
MODEL_FILE="${2:-ggml-tiny.bin}"
MODEL_PATH="${MODEL_DIR}/${MODEL_FILE}"
MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${MODEL_FILE}?download=true"

mkdir -p "${MODEL_DIR}"

if [ -f "${MODEL_PATH}" ]; then
  echo "Model already exists: ${MODEL_PATH}"
  exit 0
fi

echo "Downloading ${MODEL_FILE} to ${MODEL_PATH}..."
curl -L --fail --retry 5 --retry-delay 2 --retry-all-errors --continue-at - --progress-bar "${MODEL_URL}" -o "${MODEL_PATH}"

echo "Done: ${MODEL_PATH}"
