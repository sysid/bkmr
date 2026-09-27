#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Talk to `bkmr lsp` over stdio for manual debugging.

Automated LSP tests live in bkmr/tests/lsp (cargo test). This tool answers ad-hoc
questions against any database, e.g. "what does a javascript buffer get?".

Examples:
    lsp_probe.py capabilities
    lsp_probe.py complete rust javascript shellscript
    lsp_probe.py complete rust --prefix deri
    lsp_probe.py list javascript
    lsp_probe.py get 42

Uses BKMR_DB_URL from the environment; --bin selects the binary (default: bkmr on PATH).
"""

import argparse
import json
import subprocess
import sys


class LspClient:
    def __init__(self, cmd: list[str], verbose: bool):
        self.verbose = verbose
        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=None if verbose else subprocess.DEVNULL,
        )
        assert self.proc.stdin and self.proc.stdout
        self.stdin, self.stdout = self.proc.stdin, self.proc.stdout
        self.next_id = 1

    def _send(self, msg: dict) -> None:
        body = json.dumps(msg).encode()
        self.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.stdin.flush()

    def _read(self) -> dict:
        length = None
        while True:
            line = self.stdout.readline()
            if not line:
                sys.exit("server closed stdout")
            line = line.strip()
            if not line:
                break
            name, _, value = line.decode().partition(":")
            if name.lower() == "content-length":
                length = int(value)
        if length is None:
            sys.exit("frame without Content-Length")
        msg = json.loads(self.stdout.read(length))
        if self.verbose:
            print(f"<<< {json.dumps(msg)[:300]}", file=sys.stderr)
        return msg

    def request(self, method: str, params):
        req_id = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": req_id, "method": method, "params": params})
        while True:
            msg = self._read()
            if msg.get("id") == req_id:  # skip notifications such as window/logMessage
                if "error" in msg:
                    sys.exit(f"{method} failed: {msg['error']}")
                return msg.get("result")

    def notify(self, method: str, params) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def initialize(self):
        caps = {"textDocument": {"completion": {"completionItem": {"snippetSupport": True}}}}
        result = self.request("initialize", {"processId": None, "capabilities": caps})
        self.notify("initialized", {})
        return result

    def close(self) -> None:
        self.request("shutdown", None)
        self.notify("exit", None)
        self.proc.wait(timeout=5)


def complete(client: LspClient, languages: list[str], prefix: str) -> None:
    for lang in languages:
        uri = f"file:///tmp/probe.{lang}"
        client.notify(
            "textDocument/didOpen",
            {"textDocument": {"uri": uri, "languageId": lang, "version": 1, "text": prefix}},
        )
        result = client.request(
            "textDocument/completion",
            {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": len(prefix)},
                "context": {"triggerKind": 1},
            },
        )
        labels = sorted(item["label"] for item in (result or {}).get("items", []))
        print(f"{lang:16} {labels}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--bin", default="bkmr", help="bkmr binary (default: bkmr on PATH)")
    parser.add_argument("-v", "--verbose", action="store_true", help="show server stderr and raw responses")
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("capabilities", help="print server capabilities")
    p_complete = sub.add_parser("complete", help="completion labels per languageId")
    p_complete.add_argument("languages", nargs="+")
    p_complete.add_argument("--prefix", default="", help="text typed before the cursor")
    p_list = sub.add_parser("list", help="bkmr.listSnippets")
    p_list.add_argument("language", nargs="?")
    p_get = sub.add_parser("get", help="bkmr.getSnippet")
    p_get.add_argument("id", type=int)
    args = parser.parse_args()

    server = [args.bin] + (["-dd"] if args.verbose else []) + ["lsp"]
    client = LspClient(server, args.verbose)
    init = client.initialize()
    if args.cmd == "capabilities":
        print(json.dumps(init, indent=2))
    elif args.cmd == "complete":
        complete(client, args.languages, args.prefix)
    elif args.cmd == "list":
        arguments = [{"language": args.language}] if args.language else [{}]
        result = client.request("workspace/executeCommand", {"command": "bkmr.listSnippets", "arguments": arguments})
        for s in (result or {}).get("snippets", []):
            print(f"{s['id']:>5}  {s['title']:<40} {','.join(s['tags'])}")
    elif args.cmd == "get":
        result = client.request("workspace/executeCommand", {"command": "bkmr.getSnippet", "arguments": [{"id": args.id}]})
        print(json.dumps(result, indent=2))
    client.close()


if __name__ == "__main__":
    main()
