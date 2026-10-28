#!/usr/bin/env python3
"""Locate and invoke an independently installed Waypost bridge, without setup."""
import os
import platform
import shutil
import sys


def main():
    args = sys.argv[1:]
    if not args or args[0] not in ("doctor", "bridge"):
        print("Usage: python3 bridge.py doctor | bridge [bridge arguments...]", file=sys.stderr)
        return 2
    executable = shutil.which("waypost-bridge")
    if args[0] == "doctor":
        if len(args) != 1:
            return 2
        print("Platform: " + platform.system() + " " + platform.machine())
        print("waypost-bridge: " + (executable or "not found on PATH"))
        print("The plugin does not install, configure, or start the bridge.")
        return 0 if executable else 1
    if not executable:
        print("waypost-bridge not found on PATH; see Waypost receiver setup.", file=sys.stderr)
        return 127
    if len(args) == 1:
        print("Specify bridge arguments explicitly; no bridge was started.", file=sys.stderr)
        return 2
    os.execv(executable, [executable, *args[1:]])


if __name__ == "__main__":
    sys.exit(main())
