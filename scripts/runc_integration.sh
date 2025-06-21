#!/bin/bash -u

RUNTIME=${1:-./runc}
ROOT=$(git rev-parse --show-toplevel)
RUNC_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc"
RUNC_TEST_DIR="${ROOT}/tests/runc/src/github.com/opencontainers/runc/tests/integration"

cd "$RUNC_DIR"

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
mounts.bats:runc run [tmpcopyup]
mounts.bats:runc run [/proc is a symlink]
mounts.bats:runc run [ro /sys/fs/cgroup mounts + cgroupns]
mounts.bats:runc run [mount order, container bind-mount source]
mounts.bats:runc run [mount order, container bind-mount source] (userns)
mounts.bats:runc run [mount order, container idmap source]
mounts.bats:runc run [mount order, container idmap source] (userns)
env.bats:env var HOME is set only once
idmap.bats:simple idmap mount [userns]
mounts_propagation.bats:runc run [rootfsPropagation shared]
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

BATS_FILES=$(find "$RUNC_DIR" -type f -name "*.bats")
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

    # if ! sudo -E PATH="$PATH" script -q -e -c "$BATS_PATH -t '$file'" "$logfile"; then
    if ! sudo -E PATH="$PATH" script -q -e -c "$BATS_PATH -t '$file'"; then
      echo "Test failed (even with script): $file"
      cat "$logfile"
      exit 1
    fi
  fi

  echo "Test passed: $file"
  count=$((count + 1))
done

echo "Successfully executed $count / $TOTAL test files"
