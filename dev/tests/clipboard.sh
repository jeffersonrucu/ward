#!/bin/bash -p
# Phase 39f (S2, S12; V5.4, V5.5): the app's "copy" in the real graphical session, with no
# disk and no `wl-copy`.
#
# Builds the tests of `app/src-tauri` (the same `target/` as `make app-check`) and passes
# the binary to `dev/tests/clipboard.py`, which runs the ignored real-session test of the
# `gtk_clipboard` module (the real GTK clipboard, with the core's copier) under `strace -f`
# and verifies, only with fake values generated on the spot:
# - first, the self-test: the file watcher finds the value that `wl-copy` writes in
#   `/tmp/wl-copy-buffer-*/` (the attack of the M5 audit is caught);
# - no file in `/tmp`, `/var/tmp` or `$XDG_RUNTIME_DIR` receives the value
#   (inotify during the run and a scan at the end), the test process executes no
#   program (no `wl-copy`) and opens no file for writing in those folders;
# - V5.4: `wl-paste` returns the copied value, with `x-kde-passwordManagerHint=secret`;
# - V5.5: it is gone after the deadline; text the user copies afterwards stays;
# - a lock coming from outside (clearing off the main thread) removes the value at once;
#   "copy" off the main thread is refused; when the app exits the value is gone.
#
# The compositor (COSMIC) only accepts the selection from a Wayland client with keyboard
# focus, which a test without a click does not have: the full run uses GTK's X11 backend
# on the session's Xwayland (COSMIC forwards the selection to Wayland, and `wl-paste`
# reads it there). A 2nd run with the Wayland backend only checks strace and the watcher
# (without focus the compositor refuses the selection; the Phase 39f handoff explains).
# At the end the clipboard is cleared.
#
# Requires `strace`, `wl-paste`/`wl-copy` (only the test uses them, to read and to
# imitate the user), python3-gi (GTK 3), `libwebkit2gtk-4.1-dev` (or PKG_CONFIG_PATH) and
# a session with `WAYLAND_DISPLAY` and `DISPLAY`; out of `make check`.
# Usage: dev/tests/clipboard.sh <cargo>   (or `make test-clipboard`)

set -euo pipefail
export PATH=/usr/bin:/bin
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
. "$ROOT/dev/env.sh"

CARGO="${1:?uso: clipboard.sh <cargo>}"
[[ -x "$CARGO" ]] || { echo "area-transferencia: cargo não encontrado em $CARGO" >&2; exit 1; }
for f in strace wl-paste wl-copy python3; do
    command -v "$f" > /dev/null || { echo "area-transferencia: falta $f" >&2; exit 1; }
done

JSON="$(cd "$ROOT/app/src-tauri" && "$CARGO" test --all-features --locked --no-run \
    --message-format=json --quiet)"
BIN="$(printf '%s\n' "$JSON" | /usr/bin/python3 -I -B -c '
import json, sys
for line in sys.stdin:
    try:
        m = json.loads(line)
    except ValueError:
        continue
    target = m.get("target", {})
    if m.get("reason") == "compiler-artifact" and m.get("executable") \
            and target.get("name") == "ward-app" and m.get("profile", {}).get("test"):
        print(m["executable"])
')"
[[ -n "$BIN" && -x "$BIN" ]] || { echo "area-transferencia: binário de teste do app não encontrado" >&2; exit 1; }

exec /usr/bin/python3 -I -B "$ROOT/dev/tests/clipboard.py" "$BIN"
