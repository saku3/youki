#!/bin/bash -eu

ROOT=$(git rev-parse --show-toplevel)
RUNC_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc"
RUNC_TEST_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc/tests/integration"
PATTERN=${2:-.}

if [[ ! -x ./youki ]]; then
  echo "youki binary not found"
  exit 1
fi

cp ./youki "$RUNC_DIR/runc"
chmod +x "$RUNC_DIR/runc"

cd "$RUNC_TEST_DIR"

# Ensure bats is installed
if ! command -v bats >/dev/null 2>&1; then
    echo "bats is not installed"
    exit 1
fi

mkdir -p log
FAILED=0

find "$RUNC_TEST_DIR" -name "*.bats" | while read -r test_case; do
    echo "Running $test_case"
    logfile="./log/$(basename "$test_case").log"
    mkdir -p "$(dirname "$logfile")"

    if ! sudo -E bats "$test_case" >"$logfile" 2>&1; then
        echo "Test failed: $test_case"
        cat "$logfile"
        FAILED=1
    else
        echo "Test passed: $test_case"
    fi
done

exit $FAILED