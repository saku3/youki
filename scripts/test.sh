#!/bin/bash -eu

TARGET_DIR=${1:-"tests/runc/src/github.com/opencontainers/runc/tests/integration"} 
BATS_PATH=$(command -v bats || true)

if [[ -z "$BATS_PATH" ]]; then
  echo "bats not found. Please install it (e.g., sudo apt-get install bats)"
  exit 1
fi

BATS_FILES=$(find "$TARGET_DIR" -type f -name "*.bats")

if [[ -z "$BATS_FILES" ]]; then
  echo "No .bats files found in $TARGET_DIR"
  exit 0
fi

for file in $BATS_FILES; do
  echo "Running test: $file"
  sudo "$BATS_PATH" "$file"
  echo "-------------------------"
done
