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

if [[ -z "$BATS_FILES" ]]; then
  echo "No .bats files found in $TARGET_DIR"
  exit 0
fi

# for file in $BATS_FILES; do
#   echo "Running test: $file"
# # sudo "$BATS_PATH" "$file"
#   sudo -E PATH="$PATH" script -q -e -c "bats -t $file"
# # sudo -E PATH="$PATH" script -q -e -c "bats -t tests/integration/mounts_propagation.bats"
# done
for file in $BATS_FILES; do
  echo "Running test: $file"
  logfile="./script-log-$(basename "$file").log"
  mkdir -p "$(dirname "$logfile")"

  sudo -E PATH="$PATH" script -q -e -c "bats -t '$file'" "$logfile"
  cat $logfile
done