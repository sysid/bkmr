#!/usr/bin/env python3
"""
============================================================================
test_lsp_client.py - Enhanced LSP Protocol Debugging Client for bkmr
============================================================================

Purpose:
    Professional LSP client for testing and debugging bkmr's built-in LSP 
    server with comprehensive error handling, detailed logging, and protocol 
    validation.

Features:
    • Complete LSP protocol implementation
    • Detailed request/response logging with pretty-printing
    • Server stderr monitoring and filtering
    • Robust error handling and timeout management
    • Process lifecycle management
    • Real-time protocol debugging
    • Support for bkmr's built-in LSP server

Usage:
    python3 scripts/lsp/test_lsp_client.py [options]

Examples:
    python3 scripts/lsp/test_lsp_client.py
    python3 scripts/lsp/test_lsp_client.py --db-path ../db/bkmr.db
    python3 scripts/lsp/test_lsp_client.py --debug
    python3 scripts/lsp/test_lsp_client.py --no-interpolation

Output:
    - Structured LSP message logging (requests and responses)
    - Server stderr output with filtering
    - Connection status and error diagnostics
    - Completion results analysis
    - Process management status

Use Cases:
    • LSP server development and debugging
    • Protocol compliance testing
    • Communication troubleshooting
    • Performance analysis
    • Integration testing

Environment Variables:
    BKMR_DB_URL: Database path (default: ~/.config/bkmr/bkmr.db)
    RUST_LOG: Logging level (debug, info, warn, error)
============================================================================
"""

import json
import subprocess
import sys
import threading
import time
import os
import argparse
from typing import Dict, Any, Optional


class LSPClient:
    """Enhanced LSP client with comprehensive debugging and error handling."""
    
    def __init__(self, server_cmd: str, env: Optional[Dict[str, str]] = None):
        print(f"🚀 Starting LSP server: {server_cmd}")
        
        # Set up environment
        process_env = os.environ.copy()
        if env:
            process_env.update(env)
        
        # Ensure RUST_LOG is set for debugging
        if 'RUST_LOG' not in process_env:
            process_env['RUST_LOG'] = 'info'
        
        print(f"📋 Environment: RUST_LOG={process_env.get('RUST_LOG')}, BKMR_DB_URL={process_env.get('BKMR_DB_URL', 'default')}")

        self.process = subprocess.Popen(
            server_cmd,
            shell=True,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=process_env,
            bufsize=0  # Unbuffered
        )
        self.request_id = 0

        # Start stderr reader thread
        self.stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self.stderr_thread.start()

        # Give server time to start
        time.sleep(0.5)

        # Validate server started successfully
        if self.process.poll() is not None:
            raise RuntimeError(f"❌ Server process exited immediately with code {self.process.returncode}")

    def _read_stderr(self):
        """Monitor and filter server stderr output in background thread."""
        try:
            for line in iter(self.process.stderr.readline, ''):
                if line:
                    # Filter important server messages
                    line = line.rstrip()
                    if any(keyword in line for keyword in ['ERROR', 'WARN', 'Successfully', 'Executing', 'snippets found']):
                        print(f"🔍 [SERVER] {line}")
                    elif 'DEBUG' in line and any(keyword in line for keyword in ['completion', 'LSP', 'snippet']):
                        print(f"📊 [DEBUG] {line}")
                    elif 'INFO' in line:
                        print(f"ℹ️  [INFO] {line}")
        except Exception as e:
            # Silent failure for stderr monitoring
            pass

    def send_message(self, message: Dict[str, Any]) -> None:
        """Send a JSON-RPC message to the LSP server"""
        json_str = json.dumps(message)
        content = f"Content-Length: {len(json_str)}\r\n\r\n{json_str}"

        print(f"📤 >>> SENDING LSP MESSAGE:")
        print(f"    Content-Length: {len(json_str)}")
        print(f"    Method: {message.get('method', 'N/A')}")
        print(f"    ID: {message.get('id', 'N/A')}")
        if 'params' in message and message['params']:
            print(f"    Params: {json.dumps(message['params'], indent=2)[:200]}...")
        print()

        try:
            self.process.stdin.write(content)
            self.process.stdin.flush()
        except BrokenPipeError:
            raise RuntimeError("❌ Server stdin pipe broken - server may have crashed")

    def read_message(self, timeout: float = 5.0) -> Optional[Dict[str, Any]]:
        """Read a JSON-RPC message from the LSP server with timeout"""
        start_time = time.time()

        try:
            # Read Content-Length header with timeout
            while True:
                if time.time() - start_time > timeout:
                    print(f"⏰ TIMEOUT: No response after {timeout} seconds")
                    return None

                # Check if process died
                if self.process.poll() is not None:
                    print(f"❌ ERROR: Server process died with exit code {self.process.returncode}")
                    return None

                line = self.process.stdout.readline()
                if not line:
                    time.sleep(0.1)
                    continue

                if line.startswith("Content-Length:"):
                    content_length = int(line.split(":")[1].strip())
                    break

            # Skip empty line
            self.process.stdout.readline()

            # Read the JSON content
            content = self.process.stdout.read(content_length)
            message = json.loads(content)

            print(f"📥 <<< RECEIVED LSP MESSAGE:")
            print(f"    Content-Length: {content_length}")
            print(f"    Method: {message.get('method', 'N/A')}")
            print(f"    ID: {message.get('id', 'N/A')}")
            if 'error' in message:
                print(f"    ❌ ERROR: {message.get('error', {})}")
            elif 'result' in message:
                result = message['result']
                if isinstance(result, dict) and 'capabilities' in result:
                    print(f"    ✅ Server capabilities: {list(result['capabilities'].keys())}")
                elif isinstance(result, list) or (isinstance(result, dict) and 'items' in result):
                    item_count = len(result) if isinstance(result, list) else len(result.get('items', []))
                    print(f"    ✅ Completion items: {item_count}")
                else:
                    print(f"    Result: {json.dumps(result)[:200]}...")
            print()

            return message

        except json.JSONDecodeError as e:
            print(f"❌ JSON ERROR: Failed to decode server response: {e}")
            print(f"    Raw content: {repr(content)}")
            return None
        except Exception as e:
            print(f"❌ COMMUNICATION ERROR: {e}")
            return None

    def next_id(self) -> int:
        self.request_id += 1
        return self.request_id

    def initialize(self) -> Optional[Dict[str, Any]]:
        """Send initialize request"""
        message = {
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "processId": None,
                "clientInfo": {
                    "name": "bkmr-test-client",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "textDocument": {
                        "completion": {
                            "completionItem": {
                                "snippetSupport": True,
                                "insertReplaceSupport": True,
                                "deprecatedSupport": True
                            },
                            "contextSupport": True
                        }
                    }
                },
                "workspaceFolders": None
            }
        }

        self.send_message(message)
        return self.read_message()

    def initialized(self) -> None:
        """Send initialized notification"""
        message = {
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        }
        self.send_message(message)

    def did_open(self, uri: str, language_id: str, text: str) -> None:
        """Send textDocument/didOpen notification"""
        message = {
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }
        }
        self.send_message(message)

    def completion(self, uri: str = "file:///tmp/test.txt", line: int = 0, character: int = 0) -> Optional[Dict[str, Any]]:
        """Request completion"""
        message = {
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "textDocument/completion",
            "params": {
                "textDocument": {
                    "uri": uri
                },
                "position": {
                    "line": line,
                    "character": character
                },
                "context": {
                    "triggerKind": 1  # Invoked
                }
            }
        }

        self.send_message(message)
        return self.read_message()

    def shutdown(self) -> Optional[Dict[str, Any]]:
        """Send shutdown request"""
        message = {
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "shutdown",
            "params": None
        }

        self.send_message(message)
        return self.read_message()

    def exit(self) -> None:
        """Send exit notification"""
        message = {
            "jsonrpc": "2.0",
            "method": "exit",
            "params": None
        }
        self.send_message(message)

    def close(self):
        """Close the LSP client"""
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()


def test_lsp_server(args):
    """Execute comprehensive LSP server testing sequence."""
    print("=" * 80)
    print("🔧 bkmr LSP Server Debug Session")
    print("=" * 80)
    print(f"💻 Server: bkmr lsp {' '.join(args.lsp_args)}")
    print(f"📄 Protocol: Language Server Protocol (LSP) 3.17")
    print(f"🗄️  Database: {args.db_path or 'default'}")
    print(f"🐛 Debug: {'Enabled' if args.debug else 'Standard'}")
    print(f"🔄 Interpolation: {'Disabled' if args.no_interpolation else 'Enabled'}")
    print("=" * 80)
    print()

    # Build server command
    server_cmd = "bkmr lsp"
    if args.no_interpolation:
        server_cmd += " --no-interpolation"
    
    # Set up environment
    env = {}
    if args.db_path:
        env['BKMR_DB_URL'] = args.db_path
    if args.debug:
        env['RUST_LOG'] = 'debug'
    elif args.verbose:
        env['RUST_LOG'] = 'info'

    try:
        client = LSPClient(server_cmd, env)
    except RuntimeError as e:
        print(f"Failed to start server: {e}")
        return 1

    try:
        # Step 1: Initialize
        print("=== 1. INITIALIZE ===")
        init_response = client.initialize()
        if not init_response:
            print("❌ FAILED: No initialize response received")
            return 1
        elif 'error' in init_response:
            print(f"❌ FAILED: Initialize error: {init_response['error']}")
            return 1
        else:
            print("✅ SUCCESS: LSP server initialized")
            capabilities = init_response.get('result', {}).get('capabilities', {})
            if capabilities:
                print(f"    🛠️  Server capabilities:")
                for cap in capabilities:
                    print(f"        • {cap}")

        # Step 2: Initialized notification
        print("\n=== 2. INITIALIZED NOTIFICATION ===")
        client.initialized()
        print("✅ Initialized notification sent")
        time.sleep(0.5)  # Give server time to process

        # Step 3: Open a document (to set language context)
        print("\n=== 3. OPEN DOCUMENT ===")
        test_uri = "file:///tmp/test.rs"
        client.did_open(test_uri, "rust", "// Test file\n")
        print(f"✅ Opened document: {test_uri}")
        time.sleep(0.2)

        # Step 4: Test completion at various positions
        print("\n=== 4. COMPLETION REQUESTS ===")
        
        # Test cases for different positions
        test_cases = [
            ("Empty position", 0, 0),
            ("After comment", 0, 12),
            ("New line", 1, 0),
        ]
        
        for description, line, char in test_cases:
            print(f"\n📍 Testing: {description} (line {line}, char {char})")
            completion_response = client.completion(test_uri, line, char)
            
            if completion_response:
                if 'error' in completion_response:
                    print(f"❌ COMPLETION ERROR: {completion_response['error']}")
                else:
                    result = completion_response.get("result")
                    if result:
                        items = result if isinstance(result, list) else result.get('items', [])
                        item_count = len(items)
                        print(f"✅ SUCCESS: Received {item_count} completion items")
                        
                        # Show first few items with details
                        for i, item in enumerate(items[:3]):
                            label = item.get('label', 'No label')
                            kind = item.get('kind', 'Unknown')
                            detail = item.get('detail', '')
                            tags = item.get('data', {}).get('tags', []) if isinstance(item.get('data'), dict) else []
                            print(f"    {i + 1}. {label}")
                            print(f"       Kind: {kind}, Detail: {detail}")
                            if tags:
                                print(f"       Tags: {', '.join(tags)}")
                        
                        if len(items) > 3:
                            print(f"    ... and {len(items) - 3} more items")
                    else:
                        print("⚠️  Empty completion result")
            else:
                print("❌ FAILED: No completion response received")

        # Step 5: Shutdown
        print("\n=== 5. SHUTDOWN SEQUENCE ===")
        shutdown_response = client.shutdown()
        if shutdown_response:
            print("✅ Shutdown request acknowledged")
        else:
            print("⚠️  No shutdown response")
        
        client.exit()
        print("✅ Exit notification sent")
        
        print("\n" + "=" * 80)
        print("✅ LSP debug session completed successfully")
        print("=" * 80)
        return 0

    except KeyboardInterrupt:
        print("\n❌ Test interrupted by user")
        return 1
    except Exception as e:
        print(f"❌ Test failed with error: {e}")
        import traceback
        traceback.print_exc()
        return 1
    finally:
        client.close()


def main():
    parser = argparse.ArgumentParser(
        description="Test and debug bkmr's built-in LSP server",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                           # Basic test
  %(prog)s --debug                  # Enable debug logging
  %(prog)s --db-path ../db/bkmr.db  # Use specific database
  %(prog)s --no-interpolation       # Disable template interpolation
  %(prog)s --verbose                # Enable verbose output

Environment Variables:
  BKMR_DB_URL    Database path (overrides --db-path)
  RUST_LOG       Logging level (overrides --debug/--verbose)
        """
    )
    
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging (RUST_LOG=debug)')
    parser.add_argument('--verbose', '-v', action='store_true',
                        help='Enable verbose output (RUST_LOG=info)')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--no-interpolation', action='store_true',
                        help='Disable template interpolation')
    parser.add_argument('lsp_args', nargs=argparse.REMAINDER,
                        help='Additional arguments to pass to bkmr lsp')
    
    args = parser.parse_args()
    
    # Validate that bkmr is available
    import shutil
    if not shutil.which('bkmr'):
        print("❌ ERROR: bkmr command not found")
        print("")
        print("🔧 Please ensure bkmr is installed and in PATH:")
        print("  cd bkmr && cargo build --release")
        print("  cargo install --path bkmr")
        sys.exit(1)
    
    sys.exit(test_lsp_server(args))


if __name__ == "__main__":
    main()