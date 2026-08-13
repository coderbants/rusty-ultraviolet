#!/usr/bin/env python3
"""PTY driver for capturing interactive TUI program output.

Spawns a command attached to a pseudo-terminal of a fixed size, feeds it a
scripted sequence of keys at fixed delays, and writes the raw captured output
(including escape sequences) to stdout. Both the Go and Rust sides of an
example pair are run through this identical driver so their outputs can be
diffed byte-for-byte.

Usage:
  pty_driver.py --cmd <cmd> [--args a b c] [--width 80] [--height 24]
                [--keys "q"] [--delay 0.5] [--settle 0.5] [--timeout 10]

Keys are literal bytes; use escapes like \\x03 (ctrl+c) or \\x1b.
"""
import argparse
import fcntl
import os
import pty
import select
import struct
import sys
import termios
import time


def set_win_size(fd, width, height):
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))


def run(cmd, args, width, height, keys, delay, settle, timeout):
    pid, master = pty.fork()
    if pid == 0:
        # Child: attach to the pty and exec the command.
        set_win_size(0, width, height)
        os.environ["TERM"] = "xterm-256color"
        os.execvp(cmd, [cmd] + args)

    set_win_size(master, width, height)
    out = bytearray()
    start = time.time()
    sent = False
    sent_at = None
    exited = False
    while time.time() - start < timeout:
        # Send the key sequence once after `delay` seconds AND after the
        # child has produced its first output: a cold-started binary under
        # load can otherwise take longer than the delay to render, and the
        # capture would miss it entirely.
        if not sent and time.time() - start >= delay and len(out) > 0:
            os.write(master, keys)
            sent = True
            sent_at = time.time()
        # Read any available output.
        r, _, _ = select.select([master], [], [], 0.05)
        if r:
            try:
                data = os.read(master, 4096)
            except OSError:
                break
            if not data:
                break
            out.extend(data)
        # After the keys are sent, give the program `settle` seconds to
        # render its final state before closing.
        if sent and time.time() - sent_at >= settle:
            break
        # Check if the child exited.
        wpid, status = os.waitpid(pid, os.WNOHANG)
        if wpid == pid:
            exited = True
            break
    if not exited:
        # Give the child a short grace period, then kill it so the driver
        # never hangs.
        try:
            os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            pass
        try:
            os.kill(pid, 9)
        except ProcessLookupError:
            pass
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass
    sys.stdout.buffer.write(bytes(out))
    sys.stdout.buffer.flush()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--cmd", required=True)
    p.add_argument("--args", nargs="*", default=[])
    p.add_argument("--width", type=int, default=80)
    p.add_argument("--height", type=int, default=24)
    p.add_argument("--keys", default="q")
    p.add_argument("--delay", type=float, default=0.8)
    p.add_argument("--settle", type=float, default=0.8)
    p.add_argument("--timeout", type=float, default=15.0)
    args = p.parse_args()
    keys = args.keys.encode("latin1").decode("unicode_escape").encode("latin1")
    run(args.cmd, args.args, args.width, args.height, keys, args.delay, args.settle, args.timeout)


if __name__ == "__main__":
    main()
