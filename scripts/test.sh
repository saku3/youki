#!/bin/bash -eu

[ -t 1 ] || NO_COLOR=1

TARGET_DIR=${1:-"tests/integration"} 
BATS_PATH=$(command -v bats || true)

cd "tests/runc/src/github.com/opencontainers/runc"

if [[ -z "$BATS_PATH" ]]; then
  echo "bats not found. Please install it (e.g., sudo apt-get install bats)"
  exit 1
fi

BATS_FILES=$(find "$TARGET_DIR" -type f -name "*.bats")
TOTAL=$(echo "$BATS_FILES" | wc -l)

if [[ -z "$BATS_FILES" || "$TOTAL" -eq 0 ]]; then
  echo "No .bats files found in $TARGET_DIR"
  exit 0
fi

echo "Found $TOTAL .bats test files to run"
count=0

for file in $BATS_FILES; do
  echo "Running test: $file"
  logfile="./$(basename "$file").log"
  mkdir -p "$(dirname "$logfile")"

  if ! sudo -E "$BATS_PATH" -t "$file" | tee "$logfile"; then
    echo "Direct run failed, retrying with script..."

    if ! sudo -E PATH="$PATH" script -q -e -c "$BATS_PATH -t '$file'" "$logfile"; then
      echo "Test failed (even with script): $file"
      cat "$logfile"
      exit 1
    fi
  fi

  echo "Test passed: $file"
  count=$((count + 1))
done

echo "Successfully executed $count / $TOTAL test files"
