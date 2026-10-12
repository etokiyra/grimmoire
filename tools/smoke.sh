#!/bin/sh
# Terminal-lifecycle smoke test.
#
# Grimmoire only changes terminal state in ways a real terminal can observe, so
# this drives the binary through a pseudo-terminal and checks the bytes it emits:
# the alternate screen is entered and left, the cursor is hidden and shown again,
# nothing is cleared up after leaving, and — by default — no graphics are sent at
# all, because the world is ASCII.
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
import fcntl, os, pty, select, struct, sys, tempfile, termios, time

binary = os.path.abspath(sys.argv[1])
project = os.getcwd()
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    # Still force the terminal capability on, so the startup contract is
    # exercised without a real Kitty window.
    os.environ["GRIMMOIRE_ASSUME_KITTY"] = "1"
    # Never touch the real save: a stale one would start the game somewhere else
    # and make this test lie.
    scratch = tempfile.mkdtemp(prefix="grimmoire-smoke-")
    os.environ["XDG_DATA_HOME"] = scratch
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

# Wait for something rather than sleeping a fixed time: on a pseudo-terminal the
# keyboard handshake can take a while to give up, and the title's own opening
# plays after that. Polling is what makes this check about the game and not about
# how fast the harness guessed the timings.
def wait_for(needle, limit=25.0):
    seen = b""
    deadline = time.time() + limit
    while time.time() < deadline:
        seen += drain(0.4)
        if needle in seen:
            break
    return seen

# Start at the reference size: the title should compose rather than suspend.
resize(120, 40)
title = wait_for(b"New Game", 20.0)

# Enter the world the way a player does, so everything after this is the game.
os.write(fd, b"\r")
out = wait_for(b"Ruined Chapel", 20.0)
for keys in (b"s", b"w", b"a", b"d", b"h"):
    os.write(fd, keys)
    time.sleep(0.3)
    out += drain(0.2)
out += title

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

# Everything observed counts as the session's output.
out += suspended + resumed + title

checks = [
    ("entered the alternate screen", b"\x1b[?1049h" in out),
    ("hid the cursor", b"\x1b[?25l" in out),
    # The world's own status line. The previous predicate here was `b"#" in out`,
    # which the *title's* tower stencil satisfied: it never once tested the world,
    # and it stopped looking true the day the title stopped drawing that tower.
    ("composed the ASCII world", b"Ruined Chapel" in out),
    # The title is the first thing the game says, and it says it in glyphs.
    ("composed the title screen", b"New Game" in title),
    # The wizard's outline ink. Nothing else in the game draws in it, and the
    # sprite cannot be matched as text because every ink change between its cells
    # emits an escape sequence.
    ("drew the player", b"38;5;55m" in out),
    ("showed the controls", b"interact" in out),
    ("suspended below the minimum viewport", b"too small" in suspended),
    ("resumed after the resize", b"Ruined Chapel" in resumed),
    ("kept the world through the title", b"Ruined Chapel" in out),
    ("showed the cursor again", b"\x1b[?25h" in out),
    ("left the alternate screen", b"\x1b[?1049l" in out),
    ("sent no graphics by default", b"\x1b_Ga=t" not in out),
    ("cleaned up any images", b"\x1b_Ga=d,d=A" in out),
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
