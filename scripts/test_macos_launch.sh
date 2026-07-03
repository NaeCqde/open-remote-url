#!/bin/bash
# Automated regression test for the macOS "double-click doesn't launch a
# window while the daemon is running" bug.
#
# Steps:
#   1. Build release binaries + package .app bundles.
#   2. Fully uninstall/kill any previous sender & receiver install.
#   3. Install both (this also starts the --daemon LaunchAgents).
#   4. `open` each installed .app, exactly like double-clicking it in Finder.
#   5. Check the process list for a freshly spawned, args-less GUI process
#      (distinct PID from the --daemon process) for each app.
#
# Exit 0 and print PASS/FAIL per app; nonzero exit if either fails.
set -uo pipefail
cd "$(dirname "$0")/.."

SENDER_APP="$HOME/Applications/OpenRemoteURLSender.app"
RECEIVER_APP="$HOME/Applications/OpenRemoteURLReceiver.app"

echo "== 1. Build & package =="
cargo build --release --package open_remote_url_sender --package open_remote_url_receiver || exit 1
cargo post build --package open_remote_url_sender --locked --release || exit 1
cargo post build --package open_remote_url_receiver --locked --release || exit 1

echo "== 2. Clean previous install/processes =="
target/release/open-remote-url-sender --uninstall >/dev/null 2>&1
target/release/open-remote-url-receiver --uninstall >/dev/null 2>&1
pkill -f "OpenRemoteURLSender.app/Contents/MacOS" 2>/dev/null
pkill -f "OpenRemoteURLReceiver.app/Contents/MacOS" 2>/dev/null
sleep 1

echo "== 3. Install (starts --daemon via LaunchAgent) =="
target/release/open-remote-url-sender --install || exit 1
target/release/open-remote-url-receiver --install || exit 1
sleep 2

daemon_pid() {
    # PID of the running --daemon process for $1 ("sender"|"receiver")
    pgrep -f "OpenRemoteURL$(tr 'a-z' 'A-Z' <<<${1:0:1})${1:1}.app/Contents/MacOS/open-remote-url-$1 --daemon" | head -1
}

SENDER_DAEMON_PID=$(daemon_pid sender)
RECEIVER_DAEMON_PID=$(daemon_pid receiver)
echo "sender daemon pid:   ${SENDER_DAEMON_PID:-<none>}"
echo "receiver daemon pid: ${RECEIVER_DAEMON_PID:-<none>}"

test_double_click() {
    local name="$1" app_path="$2" daemon_pid="$3"
    echo "== open $app_path (simulates double-click) =="
    open "$app_path"
    sleep 3

    # Any process for this app that is NOT the daemon PID = a fresh GUI instance.
    local new_pid
    new_pid=$(pgrep -f "$app_path/Contents/MacOS" | grep -v -x "$daemon_pid" | head -1)

    if [ -n "$new_pid" ]; then
        echo "PASS: $name -- new process spawned (pid $new_pid)"
        return 0
    else
        echo "FAIL: $name -- no new process appeared (double-click did nothing / hung)"
        return 1
    fi
}

echo "== 4. Simulate double-click on each installed .app =="
sender_result=0
receiver_result=0
test_double_click sender "$SENDER_APP" "$SENDER_DAEMON_PID" || sender_result=1
test_double_click receiver "$RECEIVER_APP" "$RECEIVER_DAEMON_PID" || receiver_result=1

echo "== Result =="
[ $sender_result -eq 0 ] && echo "sender:   PASS" || echo "sender:   FAIL"
[ $receiver_result -eq 0 ] && echo "receiver: PASS" || echo "receiver: FAIL"

exit $(( sender_result || receiver_result ))
