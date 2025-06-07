#!/bin/bash -u

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

cd "$RUNC_DIR"

make test-binaries

# Ensure bats is installed
if ! command -v bats >/dev/null 2>&1; then
    echo "bats is not installed"
    exit 1
fi


