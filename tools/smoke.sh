#!/bin/sh
# Terminal-lifecycle smoke test.
#
# Grimmoire only changes terminal state in ways a real terminal can observe, so
# this drives the binary through a pseudo-terminal and checks the bytes it emits:
# the alternate screen is entered and left, the cursor is hidden and shown again,
# and every image this application created is deleted on the way out.
#
# The Kitty keyboard protocol is requested behind a query that a bare
# pseudo-terminal does not answer, so the pop cannot be observed here; it is
# exercised in a real Kitty session instead (see docs/renderer-design.md).
#
# Usage: tools/smoke.sh [path-to-binary]

set -eu

BIN="${1:-./target/release/grimmoire}"
[ -x "$BIN" ] || { printf 'no executable at %s\n' "$BIN" >&2; exit 1; }

python3 - "$BIN" <<'PY'
import fcntl, os, pty, select, struct, sys, termios, time

binary = os.path.abspath(sys.argv[1])
project = os.getcwd()
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    # Force the graphics path so the image lifecycle is exercised even though a
    # pseudo-terminal cannot confirm support.
    os.environ["GRIMMOIRE_ASSUME_KITTY"] = "1"
    os.environ.setdefault("GRIMMOIRE_ASSETS", os.path.join(project, "assets"))
    os.chdir(project)
    os.execvp(binary, [binary])

def resize(columns, rows):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))

def drain(seconds):
    out = b""
    deadline = time.time() + seconds
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.1)
        if ready:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
    return out

# Start at the reference size: the world should compose rather than suspend.
resize(120, 40)
time.sleep(1.0)
out = drain(0.2)
for keys in (b"s", b"w", b"a", b"d", b"h"):
    os.write(fd, keys)
    time.sleep(0.3)
    out += drain(0.2)
# Too small: the game suspends and says so, without losing its state.
resize(40, 12)
time.sleep(0.8)
suspended = drain(0.3)

# Big again: play resumes from where it was.
resize(120, 40)
time.sleep(0.8)
resumed = drain(0.3)

time.sleep(0.2)
os.write(fd, b"q")
status = 0
try:
    _, status = os.waitpid(pid, 0)
except ChildProcessError:
    pass
out += drain(1.5)

# The startup frame may have been composed before the window size reached the
# child, so everything observed counts as the session's output.
out += suspended + resumed

checks = [
    ("entered the alternate screen", b"\x1b[?1049h" in out),
    ("hid the cursor", b"\x1b[?25l" in out),
    ("composed the world", b"#" in out),
    ("showed the controls", b"interact" in out),
    ("suspended below the minimum viewport", b"too small" in suspended),
    ("resumed after the resize", b"Ruined Chapel" in resumed),
    ("showed the cursor again", b"\x1b[?25h" in out),
    ("left the alternate screen", b"\x1b[?1049l" in out),
    ("deleted its images", b"\x1b_Ga=d,d=A" in out),
    ("exited cleanly", status == 0),
]

failed = 0
for label, ok in checks:
    print(f"{'ok  ' if ok else 'FAIL'} {label}")
    if not ok:
        failed += 1

print()
print("smoke test passed" if failed == 0 else f"smoke test failed ({failed})")
sys.exit(1 if failed else 0)
PY
