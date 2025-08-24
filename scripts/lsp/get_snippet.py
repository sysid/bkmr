#!/usr/bin/env python3
"""
============================================================================
get_snippet.py - Get Individual Snippet via LSP
============================================================================

Purpose:
    Uses bkmr's LSP executeCommand to retrieve a specific snippet by ID.
    Demonstrates how to use the bkmr.getSnippet command programmatically.

Features:
    • Connects to bkmr LSP server
    • Executes bkmr.getSnippet command with snippet ID
    • Displays detailed snippet information
    • Shows full content, metadata, and tags
    • JSON output option for programmatic use

Usage:
    python3 scripts/lsp/get_snippet.py <snippet_id> [options]

Examples:
    python3 scripts/lsp/get_snippet.py 3021
    python3 scripts/lsp/get_snippet.py 3021 --json
    python3 scripts/lsp/get_snippet.py 3021 --debug

Output:
    - Detailed snippet information with full content
    - Metadata including title, tags, type, and creation info
    - Optional JSON output for programmatic use

Environment Variables:
    BKMR_DB_URL: Database path
    RUST_LOG: Logging level for debugging
============================================================================
"""

import json
import subprocess
import sys
import time
import os
import argparse
from typing import Dict, Any, Optional

class SnippetGetter:
    """Gets individual snippets using LSP executeCommand."""
    
    def __init__(self, debug: bool = False):
        self.debug = debug
    
    def connect_to_server(self, server_cmd: str, env: Optional[Dict[str, str]] = None) -> 'LSPClient':
        """Connect to the LSP server."""
        return LSPClient(server_cmd, env, debug=self.debug)
    
    def get_snippet(self, client: 'LSPClient', snippet_id: int) -> Optional[Dict[str, Any]]:
        """Execute bkmr.getSnippet command with snippet ID."""
        print(f"🔍 Retrieving snippet ID {snippet_id}...")
        
        response = client.execute_command("bkmr.getSnippet", [{
            "id": snippet_id
        }])
        
        if not response:
            print("❌ No response from server")
            return None
        
        if 'error' in response:
            print(f"❌ Command failed: {response['error']}")
            return None
        
        result = response.get('result', {})
        if not result:
            print("❌ Empty result from server")
            return None
        
        # Check if the result contains an error
        if 'error' in result and not result.get('success', True):
            error_info = result['error']
            error_msg = error_info.get('message', 'Unknown error')
            print(f"❌ {error_msg}")
            return None
        
        # The result IS the snippet directly (not wrapped in a "snippet" key)
        snippet = result
        if not snippet.get('id'):
            print(f"❌ Invalid snippet data received")
            return None
        
        print(f"✅ Retrieved snippet ID {snippet_id}")
        return snippet
    
    def format_snippet(self, snippet: Dict[str, Any], format_type: str = 'detailed') -> None:
        """Format and display snippet."""
        if format_type == 'json':
            print(json.dumps(snippet, indent=2))
            return
        
        # Detailed format
        print("\n" + "=" * 80)
        print(f"🔍 SNIPPET DETAILS - ID: {snippet.get('id', 'N/A')}")
        print("=" * 80)
        
        # Basic info
        print(f"Title: {snippet.get('title', 'Untitled')}")
        print(f"ID: {snippet.get('id', 'N/A')}")
        
        # Tags
        tags = snippet.get('tags', [])
        user_tags = [tag for tag in tags if not tag.startswith('_')]
        system_tags = [tag for tag in tags if tag.startswith('_')]
        
        if user_tags:
            print(f"Tags: {', '.join(user_tags)}")
        if system_tags:
            print(f"System Tags: {', '.join(system_tags)}")
        
        # Description
        description = snippet.get('description', '')
        if description:
            print(f"Description: {description}")
        
        # Metadata
        metadata = snippet.get('metadata', '')
        if metadata:
            print(f"Metadata: {metadata}")
        
        # File info (if available)
        file_path = snippet.get('file_path')
        if file_path:
            print(f"File Path: {file_path}")
            
        file_mtime = snippet.get('file_mtime')
        if file_mtime:
            import datetime
            mtime = datetime.datetime.fromtimestamp(file_mtime)
            print(f"Last Modified: {mtime.strftime('%Y-%m-%d %H:%M:%S')}")
        
        # Content
        print("\nContent:")
        print("-" * 60)
        content = snippet.get('url', snippet.get('content', ''))
        if content:
            print(content)
        else:
            print("(No content)")
        print("-" * 60)
        print("=" * 80)


class LSPClient:
    """Simple LSP client for executing commands."""
    
    def __init__(self, server_cmd: str, env: Optional[Dict[str, str]] = None, debug: bool = False):
        self.debug = debug
        
        process_env = os.environ.copy()
        if env:
            process_env.update(env)
        
        if 'RUST_LOG' not in process_env:
            process_env['RUST_LOG'] = 'debug' if debug else 'error'
        
        if debug:
            print(f"🚀 Starting LSP server: {server_cmd}")
            print(f"Environment: RUST_LOG={process_env.get('RUST_LOG')}, BKMR_DB_URL={process_env.get('BKMR_DB_URL', 'default')}")
        
        self.process = subprocess.Popen(
            server_cmd,
            shell=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=process_env,
            bufsize=0
        )
        self.request_id = 0
        
        # Start stderr reader
        import threading
        self.stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self.stderr_thread.start()
        
        time.sleep(0.5)
        
        if self.process.poll() is not None:
            raise RuntimeError(f"Server exited immediately with code {self.process.returncode}")
    
    def _read_stderr(self):
        """Read stderr with filtering."""
        try:
            for line in iter(self.process.stderr.readline, ''):
                if line:
                    line = line.rstrip()
                    if self.debug:
                        print(f"[SERVER] {line}")
                    elif 'ERROR' in line or 'WARN' in line:
                        print(f"[SERVER] {line}")
        except:
            pass
    
    def send_request(self, method: str, params: Any) -> Optional[Dict[str, Any]]:
        """Send request and get response."""
        self.request_id += 1
        message = {
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": method,
            "params": params
        }
        
        content = json.dumps(message)
        lsp_message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        if self.debug:
            print(f"📤 Sending: {method}")
        
        try:
            self.process.stdin.write(lsp_message)
            self.process.stdin.flush()
            
            # Read messages until we get the response to our request
            while True:
                # Read message header
                while True:
                    header = self.process.stdout.readline()
                    if not header:
                        return None
                    if header.startswith("Content-Length:"):
                        content_length = int(header.split(":")[1].strip())
                        break
                
                self.process.stdout.readline()  # empty line
                content = self.process.stdout.read(content_length)
                response = json.loads(content)
                
                if self.debug:
                    print(f"📥 Received: {response}")
                
                # Handle log messages - continue reading for actual response
                if response.get("method") == "window/logMessage":
                    if self.debug:
                        print("    ^ This was a log message, continuing...")
                    continue
                
                # Check if this is our response
                if response.get("id") == self.request_id:
                    return response
                
                # Other message types - continue reading
                if self.debug:
                    print("    ^ Not our response, continuing...")
                continue
        except Exception as e:
            if self.debug:
                print(f"❌ Request failed: {e}")
            return None
    
    def send_notification(self, method: str, params: Any):
        """Send notification without expecting response."""
        message = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }
        
        content = json.dumps(message)
        lsp_message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        try:
            self.process.stdin.write(lsp_message)
            self.process.stdin.flush()
        except:
            pass
    
    def initialize(self) -> Optional[Dict[str, Any]]:
        """Initialize the LSP server."""
        return self.send_request("initialize", {
            "processId": None,
            "clientInfo": {
                "name": "snippet-getter",
                "version": "0.1.0"
            },
            "capabilities": {},
            "workspaceFolders": None
        })
    
    def execute_command(self, command: str, arguments: list) -> Optional[Dict[str, Any]]:
        """Execute a command."""
        return self.send_request("workspace/executeCommand", {
            "command": command,
            "arguments": arguments
        })
    
    def shutdown(self):
        """Shutdown the server."""
        self.send_request("shutdown", None)
        self.send_notification("exit", None)
    
    def close(self):
        """Close the client."""
        try:
            self.process.terminate()
            self.process.wait(timeout=2)
        except:
            self.process.kill()


def main():
    parser = argparse.ArgumentParser(
        description="Get specific snippet by ID using bkmr LSP executeCommand",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
This script demonstrates how to use the bkmr.getSnippet LSP command
to retrieve a specific snippet by ID.

The script connects to bkmr's LSP server and executes:
  bkmr.getSnippet with arguments: [{"id": SNIPPET_ID}]

Examples:
  %(prog)s 3021              # Show detailed snippet view
  %(prog)s 3021 --json       # Output raw JSON response
  %(prog)s 3021 --debug      # Enable debug logging

Environment Variables:
  BKMR_DB_URL    Database path override
  RUST_LOG       Server logging level
        """
    )
    
    parser.add_argument('snippet_id', type=int, metavar='ID',
                        help='Snippet ID to retrieve')
    parser.add_argument('--json', '-j', action='store_true',
                        help='Output in JSON format')
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--no-interpolation', action='store_true',
                        help='Disable template interpolation')
    
    args = parser.parse_args()
    
    # Build server command - use relative path to bkmr binary
    server_cmd = "./bkmr/target/debug/bkmr lsp"
    if args.no_interpolation:
        server_cmd += " --no-interpolation"
    
    # Set up environment
    env = {}
    if args.db_path:
        env['BKMR_DB_URL'] = args.db_path
    if args.debug:
        env['RUST_LOG'] = 'debug'
    
    # Create getter
    getter = SnippetGetter(debug=args.debug)
    
    try:
        # Connect to server
        client = getter.connect_to_server(server_cmd, env)
        
        # Initialize server
        response = client.initialize()
        if not response or 'error' in response:
            print("❌ Failed to initialize LSP server")
            return 1
        
        # Send initialized notification
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        # Get snippet
        snippet = getter.get_snippet(client, args.snippet_id)
        
        if snippet:
            format_type = 'json' if args.json else 'detailed'
            getter.format_snippet(snippet, format_type)
        else:
            return 1
        
        # Shutdown
        client.shutdown()
        return 0
        
    except Exception as e:
        print(f"❌ Error: {e}")
        if args.debug:
            import traceback
            traceback.print_exc()
        return 1
    finally:
        if 'client' in locals():
            client.close()


if __name__ == "__main__":
    sys.exit(main())