#!/usr/bin/env python3
"""
============================================================================
list_snippets.py - List Snippets by Language via LSP
============================================================================

Purpose:
    Uses bkmr's LSP executeCommand to list snippets with optional language
    filtering. Demonstrates how to use the bkmr.listSnippets command programmatically.

Features:
    • Connects to bkmr LSP server
    • Executes bkmr.listSnippets command with optional language filter
    • Displays results in formatted table or JSON
    • Shows snippet details including ID, title, content preview, and tags
    • Supports any programming language filter

Usage:
    python3 scripts/lsp/list_snippets.py [options]

Examples:
    python3 scripts/lsp/list_snippets.py --language sh
    python3 scripts/lsp/list_snippets.py --language rust --json
    python3 scripts/lsp/list_snippets.py --debug
    python3 scripts/lsp/list_snippets.py --preview 100

Output:
    - Formatted table of filtered snippets
    - Snippet ID, title, content preview, and tags
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
from typing import Dict, Any, Optional, List


class SnippetLister:
    """Lists snippets by language using LSP executeCommand."""
    
    def __init__(self, debug: bool = False):
        self.debug = debug
        self.snippets = []
    
    def connect_to_server(self, server_cmd: str, env: Optional[Dict[str, str]] = None) -> 'LSPClient':
        """Connect to the LSP server."""
        return LSPClient(server_cmd, env, debug=self.debug)
    
    def list_snippets(self, client: 'LSPClient', language: Optional[str] = None) -> List[Dict[str, Any]]:
        """Execute bkmr.listSnippets command with optional language filter."""
        if language:
            print(f"🔍 Querying {language} snippets...")
        else:
            print("🔍 Querying all snippets...")
        
        # Build arguments - only include language if specified
        args = [{}]
        if language:
            args[0]["language"] = language
            
        response = client.execute_command("bkmr.listSnippets", args)
        
        if not response:
            print("❌ No response from server")
            return []
        
        if 'error' in response:
            print(f"❌ Command failed: {response['error']}")
            return []
        
        result = response.get('result', {})
        if not result:
            print("❌ Empty result from server")
            return []
        
        snippets = result.get('snippets', [])
        if not snippets:
            if language:
                print(f"ℹ️  No {language} snippets found")
            else:
                print("ℹ️  No snippets found")
            return []
        
        if language:
            print(f"✅ Found {len(snippets)} {language} snippets")
        else:
            print(f"✅ Found {len(snippets)} snippets")
        return snippets
    
    def format_snippets(self, snippets: List[Dict[str, Any]], format_type: str = 'table', preview_length: int = 50) -> None:
        """Format and display snippets."""
        if format_type == 'json':
            print(json.dumps(snippets, indent=2))
            return
        
        if not snippets:
            print("No snippets to display.")
            return
        
        # Table format
        print("\n" + "=" * 100)
        print("📋 SNIPPETS")
        print("=" * 100)
        
        # Header
        print(f"{'ID':<5} {'Title':<25} {'Preview':<40} {'Tags':<25}")
        print("-" * 100)
        
        for snippet in snippets:
            snippet_id = snippet.get('id', 'N/A')
            title = snippet.get('title', 'Untitled')
            content = snippet.get('url', snippet.get('content', ''))
            tags = snippet.get('tags', [])
            
            # Truncate long fields
            title = title[:24] + "..." if len(title) > 24 else title
            
            # Clean and preview content
            content_lines = content.split('\n')
            preview = content_lines[0] if content_lines else ''
            preview = preview.strip()
            if len(preview) > preview_length:
                preview = preview[:preview_length-3] + "..."
            
            # Format tags (exclude system tags)
            user_tags = [tag for tag in tags if not tag.startswith('_')]
            tags_str = ', '.join(user_tags[:3])  # Show first 3 tags
            if len(user_tags) > 3:
                tags_str += "..."
            tags_str = tags_str[:24]
            
            print(f"{snippet_id:<5} {title:<25} {preview:<40} {tags_str:<25}")
        
        print("-" * 100)
        print(f"Total: {len(snippets)} snippets")
        print("=" * 100)
    
    def show_detailed_snippet(self, snippets: List[Dict[str, Any]], snippet_id: int) -> None:
        """Show detailed view of a specific snippet."""
        snippet = next((s for s in snippets if s.get('id') == snippet_id), None)
        if not snippet:
            print(f"❌ Snippet with ID {snippet_id} not found")
            return
        
        print("\n" + "=" * 80)
        print(f"🔍 SNIPPET DETAILS - ID: {snippet_id}")
        print("=" * 80)
        
        print(f"Title: {snippet.get('title', 'Untitled')}")
        print(f"ID: {snippet.get('id', 'N/A')}")
        
        tags = snippet.get('tags', [])
        user_tags = [tag for tag in tags if not tag.startswith('_')]
        system_tags = [tag for tag in tags if tag.startswith('_')]
        
        if user_tags:
            print(f"Tags: {', '.join(user_tags)}")
        if system_tags:
            print(f"System Tags: {', '.join(system_tags)}")
        
        description = snippet.get('description', '')
        if description:
            print(f"Description: {description}")
        
        print("\nContent:")
        print("-" * 40)
        content = snippet.get('url', snippet.get('content', ''))
        print(content)
        print("-" * 40)
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
                "name": "shell-snippet-lister",
                "version": "0.1.0"
            },
            "capabilities": {},
            "workspaceFolders": None
        })
    
    def execute_command(self, command: str, arguments: List[Any]) -> Optional[Dict[str, Any]]:
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
        description="List snippets by language using bkmr LSP executeCommand",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
This script demonstrates how to use the bkmr.listSnippets LSP command
to retrieve snippets programmatically with optional language filtering.

The script connects to bkmr's LSP server and executes:
  bkmr.listSnippets with arguments: [{"language": "LANG"}] or [{}] for all

Examples:
  %(prog)s --language sh     # Show shell snippets in table format
  %(prog)s --language rust   # Show Rust snippets
  %(prog)s --json            # Output raw JSON response (all snippets)
  %(prog)s --debug           # Enable debug logging
  %(prog)s --preview 80      # Show longer content previews
  %(prog)s --detail-id 5     # Show detailed view of snippet ID 5

Environment Variables:
  BKMR_DB_URL    Database path override
  RUST_LOG       Server logging level
        """
    )
    
    parser.add_argument('--language', '-l', metavar='LANG',
                        help='Language filter (e.g., sh, rust, python, javascript)')
    parser.add_argument('--json', '-j', action='store_true',
                        help='Output in JSON format')
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--preview', type=int, default=50, metavar='N',
                        help='Length of content preview (default: 50)')
    parser.add_argument('--detail-id', type=int, metavar='ID',
                        help='Show detailed view of specific snippet ID')
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
    
    # Create lister
    lister = SnippetLister(debug=args.debug)
    
    try:
        # Connect to server
        client = lister.connect_to_server(server_cmd, env)
        
        # Initialize server
        response = client.initialize()
        if not response or 'error' in response:
            print("❌ Failed to initialize LSP server")
            return 1
        
        # Send initialized notification
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        # List snippets
        snippets = lister.list_snippets(client, args.language)
        
        if snippets:
            if args.detail_id:
                # Show detailed view of specific snippet
                lister.show_detailed_snippet(snippets, args.detail_id)
            else:
                # Show formatted list
                format_type = 'json' if args.json else 'table'
                lister.format_snippets(snippets, format_type, args.preview)
        
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