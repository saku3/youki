#!/bin/bash -u

ROOT=$(git rev-parse --show-toplevel)
RUNC_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc"
RUNC_TEST_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc/tests/integration"

if [[ ! -x ./youki ]]; then
  echo "youki binary not found"
  exit 1
fi

cp ./youki "$RUNC_DIR/runc"
chmod +x "$RUNC_DIR/runc"

cd "$RUNC_DIR"

make test-binaries

BATS_PATH=$(command -v bats)

if [ -z "$BATS_PATH" ]; then
  echo "bats not found"
  exit 1
fi

# Skipping this test because it hangs and stops responding.
SKIP_PATTERN=$(cat <<EOF
cgroups.bats:runc run/create should refuse pre-existing frozen cgroup
run.bats:runc run [execve error]
events.bats:events oom
events.bats:events --interval default
rlimits.bats:runc run with RLIMIT_NOFILE(The same as system's hard value)
mounts.bats:runc run [mount order, container bind-mount source]
mounts.bats:runc run [mount order, container bind-mount source] (userns)
mounts.bats:runc run [mount order, container idmap source]
mounts.bats:runc run [mount order, container idmap source] (userns)
EOF
)

while IFS= read -r line; do
  [[ -z "$line" ]] && continue

  file_part="${line%%:*}"
  test_pattern="${line#*:}"

  file_path=$(find "$RUNC_TEST_DIR" -name "$file_part")
  if [[ -z "$file_path" || ! -f "$file_path" ]]; then
    echo "Warning: file $file_part not found"
    continue
  fi

  escaped_pattern=$(printf '%s\n' "$test_pattern" | sed 's/[^^]/[&]/g; s/\^/\\^/g')
  sed -i "/$escaped_pattern/a skip \"skip runc integration test in youki\"" "$file_path"
done <<< "$SKIP_PATTERN"

mkdir -p log
FAILED=0

while IFS= read -r test_case; do
    echo "Running $test_case"
    logfile="./log/$(basename "$test_case").log"
    mkdir -p "$(dirname "$logfile")"

    if ! sudo -E PATH="$PATH" "$BATS_PATH" "$test_case" > "$logfile" 2>&1; then
        echo "Test failed: $test_case"
        FAILED=1
    else
        echo "Test passed: $test_case"
    fi
done < <(find "$RUNC_TEST_DIR" -name "*.bats")

find "$RUNC_TEST_DIR" -name "*.bats" -exec sed -i '/skip "skip runc integration test in youki"/d' {} +

echo "Runc integration test finished"
exit $FAILED
