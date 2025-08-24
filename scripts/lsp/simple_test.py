#!/usr/bin/env python3
"""Simple LSP test to debug communication issues."""

import json
import subprocess
import sys
import time
import os

def main():
    # Start LSP server
    env = os.environ.copy()
    env['RUST_LOG'] = 'error'
    
    process = subprocess.Popen(
        "./target/debug/bkmr lsp",
        shell=True,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
        cwd="/Users/Q187392/dev/s/public/bkmr/bkmr"
    )
    
    time.sleep(0.5)
    
    try:
        # Initialize
        init_msg = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": None,
                "clientInfo": {"name": "test-client", "version": "0.1.0"},
                "capabilities": {}
            }
        }
        
        content = json.dumps(init_msg)
        message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        print(f"Sending initialize...")
        process.stdin.write(message)
        process.stdin.flush()
        
        # Read response
        print("Reading response...")
        header = process.stdout.readline()
        print(f"Header: {repr(header)}")
        
        if header.startswith("Content-Length:"):
            content_length = int(header.split(":")[1].strip())
            process.stdout.readline()  # empty line
            response = process.stdout.read(content_length)
            print(f"Response: {response}")
        
    finally:
        process.terminate()
        process.wait()

if __name__ == "__main__":
    main()