#!/usr/bin/env bats

load helpers

function setup() {
	requires root
	setup_busybox
}

function teardown() {
	[ ! -v ROOT ] && return 0 # nothing to teardown

	# XXX runc does not unmount a container which
	# shares mount namespace with the host.
	umount -R --lazy "$ROOT"/bundle/rootfs

	teardown_bundle
}

@test "runc run [host mount ns + hooks]" {
	echo "==> Updating config with custom args, hooks, and namespaces" >&2
	update_config '	  .process.args = ["/bin/echo", "Hello World"]
			| .hooks |= . + {"createRuntime": [{"path": "/bin/sh", "args": ["/bin/sh", "-c", "touch createRuntimeHook.$$"]}]}
			| .linux.namespaces -= [{"type": "mount"}]
			| .linux.maskedPaths = []
			| .linux.readonlyPaths = []'

	echo "==> Running container with runc" >&2
	runc run test_host_mntns
	echo "==> runc run exit status: $status" >&2
	[ "$status" -eq 0 ]

	echo "==> Cleaning up container" >&2
	runc delete -f test_host_mntns

	echo "==> Looking for hook output file: createRuntimeHook.*" >&2
	run -0 ls createRuntimeHook.*
	echo "==> ls output: $output" >&2

	local hook_count
	hook_count=$(echo "$output" | wc -w)
	echo "==> Number of hook files found: $hook_count" >&2
	[ "$hook_count" -eq 1 ]
}
