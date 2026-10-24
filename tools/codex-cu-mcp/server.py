#!/usr/bin/env python3
"""Expose installed OpenAI Chrome and desktop CUA over an independent stdio MCP.

Only supplies connection-local identity/lifecycle metadata. Vendor code,
browser authorization, elicitation, and security checks remain unchanged.
"""

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time
import uuid


FINISH_TOOL = {
    "name": "finish_turn",
    "description": (
        "Finish the current computer/browser task and run the vendor's turn-ended "
        "cleanup. Call after saving screenshots/results. Temporary tabs may close. "
        "The next js call begins a new browser turn; the JavaScript REPL persists."
    ),
    "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
}
def guidance(surfaces):
    entries = []
    if "browser" in surfaces:
        entries.append('Chrome: `var browser = await cua.getBrowser({id: "chrome"});`')
    if "computer" in surfaces:
        entries.append('Desktop app: `var app = await cua.getApp("Example App");`')
    return (
        "OpenAI computer-use bridge. Enabled surfaces: " + ", ".join(surfaces) + ". "
        "It uses independent connection/task IDs, never an existing Codex chat. "
        "On first js use execute exactly one appropriate entry call and read the "
        "returned documentation: " + "; ".join(entries) + ". "
        "Then use only the documented cua APIs. Browser backends are limited to "
        "Chrome; IAB and MCP Apps are not configured. Native app access is confirmed "
        "through MCP elicitation handled by the client, not automatically approved "
        "by this bridge. After UI actions observe fresh state; do not guess element "
        "IDs. Screenshot methods already emit images. Call finish_turn after "
        "saving results. Vendor documentation may be large: read stored tool-result "
        "files when the client truncates it."
    )


def runtime_env(config, surfaces):
    services = {}
    if "browser" in surfaces:
        services["browser"] = "@oai/browser-desktop/service"
    if "computer" in surfaces:
        services["sky"] = "@oai/sky/service"
    env = {**os.environ, **config.get("env", {})}
    env.update({
        "CUA_REPL_ENABLED_SURFACES": ",".join(surfaces),
        "BROWSER_USE_AVAILABLE_BACKENDS": "chrome",
        "NODE_REPL_TRUSTED_SERVICES": json.dumps(services),
    })
    return env


def discover_config(explicit=None):
    if explicit:
        candidates = [Path(explicit).expanduser()]
    else:
        home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
        base = home / "plugins/cache/openai-bundled/unified-computer-use"
        candidates = sorted(
            base.glob("*/.mcp.json"), key=lambda p: p.stat().st_mtime_ns, reverse=True
        )
    for path in candidates:
        config = json.loads(path.read_text())["mcpServers"]["cua_repl"]
        if Path(config["command"]).is_file() and all(
            not str(arg).startswith("/") or Path(arg).exists()
            for arg in config.get("args", [])
        ):
            return path, config
    raise RuntimeError(
        "No usable installed cua_repl manifest. Install/enable Computer Use and "
        "Chrome in the ChatGPT/Codex desktop app, or pass --manifest PATH."
    )


class Bridge:
    def __init__(self, manifest=None, surfaces=("browser", "computer")):
        self.manifest, config = discover_config(manifest)
        self.surfaces = surfaces
        self.guidance = guidance(surfaces)
        self.session_id = str(uuid.uuid4())
        self.turn_id = str(uuid.uuid4())
        self.lock = threading.RLock()
        self.write_lock = threading.Lock()
        self.output_lock = threading.Lock()
        self.trace_lock = threading.Lock()
        self.pending_finishes = set()
        self.cleanup_id = "bridge-cleanup-" + str(uuid.uuid4())
        self.cleanup_done = threading.Event()
        self.reader_done = threading.Event()
        self.trace = None
        trace_path = os.environ.get("CODEX_CU_MCP_TRACE") or os.environ.get("CODEX_CHROME_MCP_TRACE")
        if trace_path:
            path = Path(trace_path).expanduser()
            path.parent.mkdir(parents=True, exist_ok=True)
            fd = os.open(path, os.O_CREAT | os.O_APPEND | os.O_WRONLY, 0o600)
            self.trace = os.fdopen(fd, "a", buffering=1)
        env = runtime_env(config, surfaces)
        self.child = subprocess.Popen(
            [config["command"], *config.get("args", [])],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=None, env=env,
        )
        self.reader = threading.Thread(target=self.read_child, daemon=True)
        self.reader.start()

    def record(self, direction, message):
        if self.trace:
            with self.trace_lock:
                self.trace.write(json.dumps({
                    "time": time.time(), "direction": direction, "message": message
                }, ensure_ascii=False) + "\n")

    def to_child(self, message):
        self.record("bridge_to_vendor", message)
        data = (json.dumps(message, ensure_ascii=False) + "\n").encode()
        with self.write_lock:
            self.child.stdin.write(data)
            self.child.stdin.flush()

    def to_client(self, message):
        self.record("bridge_to_client", message)
        with self.output_lock:
            sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
            sys.stdout.flush()

    def metadata(self, params):
        params = dict(params)
        meta = dict(params.get("_meta") or {})
        # IDs name THIS connection/task. Never impersonate an existing Codex chat.
        meta["x-codex-turn-metadata"] = {
            "session_id": self.session_id, "turn_id": self.turn_id
        }
        params["_meta"] = meta
        return params

    def finish_params(self):
        return {
            "name": "turn_ended",
            "arguments": {
                "session_id": self.session_id,
                "turn_id": self.turn_id,
                "hook_event_name": "Stop",
            },
        }

    def from_client(self, message):
        self.record("client_to_bridge", message)
        # Responses (including elicitation) and notifications pass through intact.
        if message.get("method") == "tools/call":
            params = dict(message.get("params") or {})
            name = params.get("name")
            with self.lock:
                if name == "finish_turn":
                    params = {**params, **self.finish_params()}
                    self.pending_finishes.add(message["id"])
                elif name not in ("js", "js_reset"):
                    self.to_client({"jsonrpc": "2.0", "id": message.get("id"),
                                    "error": {"code": -32602, "message": "Unknown tool"}})
                    return
                message = {**message, "params": self.metadata(params)}
        self.to_child(message)

    def read_child(self):
        try:
            for line in self.child.stdout:
                message = json.loads(line)
                self.record("vendor_to_bridge", message)
                # IDs are direction-local. Do not mistake a server request for
                # a response to a client request with the same numeric ID.
                if "method" not in message:
                    if message.get("id") == self.cleanup_id:
                        self.cleanup_done.set()
                        continue
                    result = message.get("result")
                    if isinstance(result, dict):
                        if "tools" in result:
                            result["tools"] = [
                                t for t in result["tools"] if t["name"] in ("js", "js_reset")
                            ] + [FINISH_TOOL]
                            for tool in result["tools"]:
                                if tool["name"] == "js":
                                    tool["description"] = self.guidance + "\n\n" + tool["description"]
                                    # UI actions mutate state. Do not let a client
                                    # parallelize scripts as read-only operations.
                                    tool.setdefault("annotations", {})["readOnlyHint"] = False
                        if "serverInfo" in result:
                            result["serverInfo"] = {"name": "codex-cu", "version": "0.2.0"}
                            result["instructions"] = self.guidance + "\n\n" + result.get("instructions", "")
                    with self.lock:
                        if message.get("id") in self.pending_finishes:
                            self.pending_finishes.remove(message["id"])
                            if "error" not in message and not (result or {}).get("isError"):
                                self.turn_id = str(uuid.uuid4())
                self.to_client(message)
        except (BrokenPipeError, OSError):
            pass
        except Exception as exc:
            print(f"codex-cu bridge: {exc}", file=sys.stderr)
        finally:
            self.reader_done.set()

    def close(self):
        if self.child.poll() is None and not self.reader_done.is_set():
            try:
                with self.lock:
                    params = self.metadata(self.finish_params())
                self.to_child({"jsonrpc": "2.0", "id": self.cleanup_id,
                               "method": "tools/call", "params": params})
                self.cleanup_done.wait(5)
            except (OSError, BrokenPipeError):
                pass
        self.child.terminate()
        try:
            self.child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.child.kill()
            self.child.wait(timeout=3)
        self.reader.join(timeout=1)
        if self.trace:
            self.trace.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", help="Override the installed cua_repl .mcp.json path")
    parser.add_argument("--surfaces", choices=("all", "browser", "computer"), default="all",
                        help="Enable both surfaces (default), Chrome only, or desktop only")
    args = parser.parse_args()
    surfaces = ("browser", "computer") if args.surfaces == "all" else (args.surfaces,)
    bridge = Bridge(args.manifest, surfaces)

    def stop(_signum, _frame):
        raise SystemExit(0)

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        for line in sys.stdin:
            if line.strip():
                bridge.from_client(json.loads(line))
    finally:
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        signal.signal(signal.SIGINT, signal.SIG_IGN)
        bridge.close()


if __name__ == "__main__":
    main()
