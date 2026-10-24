#!/usr/bin/env python3
"""Session-local approval fixture for the explicitly authorized Calculator test.

Not installed with the MCP. Every other elicitation is declined.
"""

import argparse
import json
from pathlib import Path
import sys
import time


def decision(request, deny=False):
    schema = request.get("requested_schema")
    calculator = (
        request.get("hook_event_name") == "Elicitation"
        and request.get("mcp_server_name") == "codex-cu"
        and request.get("mode", "form") == "form"
        and request.get("message") in (
            'Allow Computer Use to use "Calculator"?',
            'Allow Computer Use to use "计算器"?',
        )
        and schema == {"type": "object", "properties": {}}
    )
    return "accept" if calculator and not deny else "decline"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audit", type=Path, required=True)
    parser.add_argument("--deny", action="store_true")
    args = parser.parse_args()
    request = json.load(sys.stdin)
    action = decision(request, args.deny)
    args.audit.parent.mkdir(parents=True, exist_ok=True)
    with args.audit.open("a") as log:
        log.write(json.dumps({"time": time.time(), "server": request.get("mcp_server_name"),
                              "message": request.get("message"), "mode": request.get("mode"),
                              "requested_schema": request.get("requested_schema"),
                              "action": action}, ensure_ascii=False) + "\n")
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "Elicitation", "action": action,
        **({"content": {}} if action == "accept" else {}),
    }}))


if __name__ == "__main__":
    main()
