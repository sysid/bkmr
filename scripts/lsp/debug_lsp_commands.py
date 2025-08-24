#!/usr/bin/env python3
"""
Debug script to test LSP executeCommand functionality
"""

import json
import subprocess
import sys
import time
import os

class DebugLSPClient:
    def __init__(self):
        env = os.environ.copy()
        env['RUST_LOG'] = 'debug'
        
        self.process = subprocess.Popen(
            "bkmr lsp",
            shell=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=env,
            bufsize=0
        )
        self.request_id = 0
        
        # Start stderr reader
        import threading
        self.stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self.stderr_thread.start()
        
        time.sleep(0.5)
    
    def _read_stderr(self):
        try:
            for line in iter(self.process.stderr.readline, ''):
                if line:
                    print(f"[STDERR] {line.rstrip()}")
        except:
            pass
    
    def _read_message(self):
        """Read a single LSP message"""
        while True:
            header = self.process.stdout.readline()
            if not header:
                return None
            if header.startswith("Content-Length:"):
                content_length = int(header.split(":")[1].strip())
                break
        
        self.process.stdout.readline()  # empty line
        content = self.process.stdout.read(content_length)
        return json.loads(content)
    
    def send_request(self, method, params):
        self.request_id += 1
        request_id = self.request_id
        message = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        }
        
        content = json.dumps(message)
        lsp_message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        print(f">>> Sending: {json.dumps(message, indent=2)}")
        
        self.process.stdin.write(lsp_message)
        self.process.stdin.flush()
        
        # Read messages until we get the response
        while True:
            message = self._read_message()
            if not message:
                return None
            
            print(f"<<< Received: {json.dumps(message, indent=2)}")
            
            # Handle log messages
            if message.get("method") == "window/logMessage":
                print("    ^ This was a log message, continuing...")
                continue
            
            # Check if this is our response
            if message.get("id") == request_id:
                return message
            
            # Other message types
            print("    ^ Not our response, continuing...")
            continue
    
    def send_notification(self, method, params):
        message = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }
        
        content = json.dumps(message)
        lsp_message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        print(f">>> Notification: {json.dumps(message, indent=2)}")
        
        self.process.stdin.write(lsp_message)
        self.process.stdin.flush()
    
    def close(self):
        try:
            self.process.terminate()
            self.process.wait(timeout=2)
        except:
            self.process.kill()

def main():
    client = DebugLSPClient()
    
    try:
        # Initialize
        print("=== INITIALIZE ===")
        response = client.send_request("initialize", {
            "processId": None,
            "clientInfo": {"name": "debug-client", "version": "0.1.0"},
            "capabilities": {
                "workspace": {
                    "executeCommand": {
                        "dynamicRegistration": True
                    }
                }
            }
        })
        
        print("\n=== INITIALIZED ===")
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        print("\n=== TEST bkmr.listSnippets (no filter) ===")
        response = client.send_request("workspace/executeCommand", {
            "command": "bkmr.listSnippets",
            "arguments": [{}]
        })
        
        print("\n=== TEST bkmr.listSnippets (sh filter) ===")
        response = client.send_request("workspace/executeCommand", {
            "command": "bkmr.listSnippets", 
            "arguments": [{"language": "sh"}]
        })
        
        print("\n=== SHUTDOWN ===")
        client.send_request("shutdown", None)
        client.send_notification("exit", None)
        
    finally:
        client.close()

if __name__ == "__main__":
    main()