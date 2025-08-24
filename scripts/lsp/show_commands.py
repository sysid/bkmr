#!/usr/bin/env python3
"""
============================================================================
show_commands.py - Display Available LSP Commands for bkmr
============================================================================

Purpose:
    Queries bkmr's built-in LSP server to discover and display all available
    commands that can be executed via the executeCommand LSP protocol.

Features:
    • Connects to LSP server and retrieves server capabilities
    • Extracts and displays available commands
    • Shows command descriptions and parameters if available
    • Provides examples of how to use commands
    • Useful for LSP client developers and debugging

Usage:
    python3 scripts/lsp/show_commands.py [options]

Examples:
    python3 scripts/lsp/show_commands.py
    python3 scripts/lsp/show_commands.py --debug
    python3 scripts/lsp/show_commands.py --json
    python3 scripts/lsp/show_commands.py --test-command bkmr.listSnippets

Output:
    - List of available LSP commands
    - Command descriptions and parameters
    - Usage examples for each command
    - Optional JSON output for programmatic use

Environment Variables:
    BKMR_DB_URL: Database path (default: ~/.config/bkmr/bkmr.db)
    RUST_LOG: Logging level (debug, info, warn, error)
============================================================================
"""

import json
import subprocess
import sys
import time
import os
import argparse
from typing import Dict, Any, Optional, List


class LSPCommandExplorer:
    """Explores and displays available LSP commands."""
    
    # Known bkmr LSP commands with descriptions and examples
    KNOWN_COMMANDS = {
        "bkmr.createSnippet": {
            "description": "Create a new snippet in the database",
            "parameters": {
                "url": "Snippet content/code",
                "title": "Snippet title",
                "description": "Optional description",
                "tags": "List of tags"
            },
            "example": {
                "url": "console.log('Hello, World!');",
                "title": "JavaScript Hello World",
                "description": "Simple console log example",
                "tags": ["javascript", "example"]
            }
        },
        "bkmr.updateSnippet": {
            "description": "Update an existing snippet",
            "parameters": {
                "id": "Snippet ID",
                "url": "Updated content",
                "title": "Updated title",
                "tags": "Updated tags"
            },
            "example": {
                "id": 1,
                "url": "console.log('Updated!');",
                "title": "Updated JavaScript",
                "tags": ["javascript", "updated"]
            }
        },
        "bkmr.deleteSnippet": {
            "description": "Delete a snippet from the database",
            "parameters": {
                "id": "Snippet ID to delete"
            },
            "example": {
                "id": 1
            }
        },
        "bkmr.getSnippet": {
            "description": "Retrieve a specific snippet by ID",
            "parameters": {
                "id": "Snippet ID to retrieve"
            },
            "example": {
                "id": 1
            }
        },
        "bkmr.listSnippets": {
            "description": "List snippets with optional language filtering",
            "parameters": {
                "language": "Optional language filter (e.g., 'rust', 'python')"
            },
            "example": {
                "language": "rust"
            }
        },
        "bkmr.searchSnippets": {
            "description": "Search snippets by query",
            "parameters": {
                "query": "Search query string"
            },
            "example": {
                "query": "async"
            }
        },
        "bkmr.insertFilepathComment": {
            "description": "Insert a comment with the current file path",
            "parameters": {
                "uri": "File URI"
            },
            "example": {
                "uri": "file:///path/to/file.rs"
            }
        }
    }
    
    def __init__(self, debug: bool = False):
        self.debug = debug
        self.server_capabilities = {}
        self.available_commands = []
    
    def connect_to_server(self, server_cmd: str, env: Optional[Dict[str, str]] = None) -> 'LSPClient':
        """Connect to the LSP server."""
        return LSPClient(server_cmd, env)
    
    def extract_commands(self, capabilities: Dict[str, Any]) -> List[str]:
        """Extract available commands from server capabilities."""
        commands = []
        
        # Check executeCommandProvider capability
        if 'executeCommandProvider' in capabilities:
            provider = capabilities['executeCommandProvider']
            if isinstance(provider, dict) and 'commands' in provider:
                commands = provider['commands']
            elif isinstance(provider, bool) and provider:
                # Server supports commands but doesn't list them
                commands = list(self.KNOWN_COMMANDS.keys())
        
        return commands
    
    def display_commands(self, commands: List[str], format: str = 'text'):
        """Display commands in the requested format."""
        if format == 'json':
            output = {
                "available_commands": commands,
                "command_details": {}
            }
            for cmd in commands:
                if cmd in self.KNOWN_COMMANDS:
                    output["command_details"][cmd] = self.KNOWN_COMMANDS[cmd]
            print(json.dumps(output, indent=2))
        else:
            print("=" * 80)
            print("📋 AVAILABLE LSP COMMANDS")
            print("=" * 80)
            
            if not commands:
                print("⚠️  No commands reported by server")
                print("    Showing known bkmr commands:")
                commands = list(self.KNOWN_COMMANDS.keys())
            
            for i, cmd in enumerate(commands, 1):
                print(f"\n{i}. {cmd}")
                
                if cmd in self.KNOWN_COMMANDS:
                    info = self.KNOWN_COMMANDS[cmd]
                    print(f"   📄 {info['description']}")
                    
                    if 'parameters' in info:
                        print("   📝 Parameters:")
                        for param, desc in info['parameters'].items():
                            print(f"      • {param}: {desc}")
                    
                    if 'example' in info:
                        print("   💡 Example:")
                        print(f"      {json.dumps(info['example'], indent=6)}")
                else:
                    print("   ❓ Unknown command (not in documentation)")
            
            print("\n" + "=" * 80)
            print(f"Total: {len(commands)} commands available")
            print("=" * 80)
    
    def test_command(self, client: 'LSPClient', command: str) -> bool:
        """Test executing a specific command."""
        print(f"\n🧪 Testing command: {command}")
        
        # Prepare test arguments based on command
        if command in self.KNOWN_COMMANDS:
            example = self.KNOWN_COMMANDS[command].get('example', {})
            args = [example]
        else:
            args = []
        
        # Send execute command request
        response = client.execute_command(command, args)
        
        if response:
            if 'error' in response:
                print(f"❌ Command failed: {response['error']}")
                return False
            else:
                print(f"✅ Command succeeded")
                if 'result' in response:
                    print(f"   Result: {json.dumps(response['result'], indent=3)[:500]}...")
                return True
        else:
            print("❌ No response received")
            return False


class LSPClient:
    """Simple LSP client for command discovery."""
    
    def __init__(self, server_cmd: str, env: Optional[Dict[str, str]] = None):
        process_env = os.environ.copy()
        if env:
            process_env.update(env)
        
        if 'RUST_LOG' not in process_env:
            process_env['RUST_LOG'] = 'warn'  # Quieter by default
        
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
        """Read stderr quietly unless there are errors."""
        try:
            for line in iter(self.process.stderr.readline, ''):
                if line and 'ERROR' in line:
                    print(f"[SERVER ERROR] {line.rstrip()}")
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
        
        try:
            self.process.stdin.write(lsp_message)
            self.process.stdin.flush()
            
            # Read response
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
        except:
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
                "name": "bkmr-command-explorer",
                "version": "0.1.0"
            },
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {"snippetSupport": True}
                    }
                }
            },
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
        description="Discover and display available LSP commands in bkmr",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
This tool connects to bkmr's LSP server and queries available commands
that can be executed via the LSP executeCommand protocol.

Examples:
  %(prog)s                        # Show all available commands
  %(prog)s --json                # Output in JSON format
  %(prog)s --test-command bkmr.listSnippets  # Test a specific command
  %(prog)s --debug               # Enable debug logging

Known Commands:
  • bkmr.createSnippet    - Create a new snippet
  • bkmr.updateSnippet    - Update existing snippet
  • bkmr.deleteSnippet    - Delete a snippet
  • bkmr.getSnippet       - Get snippet by ID
  • bkmr.listSnippets     - List all snippets
  • bkmr.searchSnippets   - Search snippets
  • bkmr.insertFilepathComment - Insert file path comment
        """
    )
    
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging')
    parser.add_argument('--json', '-j', action='store_true',
                        help='Output in JSON format')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--test-command', metavar='CMD',
                        help='Test executing a specific command')
    parser.add_argument('--no-interpolation', action='store_true',
                        help='Disable template interpolation')
    
    args = parser.parse_args()
    
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
    
    # Create explorer
    explorer = LSPCommandExplorer(debug=args.debug)
    
    print("🚀 Connecting to bkmr LSP server...")
    
    try:
        client = explorer.connect_to_server(server_cmd, env)
        
        # Initialize server
        response = client.initialize()
        if not response or 'error' in response:
            print("❌ Failed to initialize LSP server")
            return 1
        
        # Send initialized notification
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        # Extract capabilities
        capabilities = response.get('result', {}).get('capabilities', {})
        commands = explorer.extract_commands(capabilities)
        
        # If no commands found in capabilities, try known commands
        if not commands:
            commands = list(explorer.KNOWN_COMMANDS.keys())
        
        # Display or test commands
        if args.test_command:
            # Test specific command
            if args.test_command not in commands:
                print(f"⚠️  Warning: '{args.test_command}' not in known commands")
            success = explorer.test_command(client, args.test_command)
            return 0 if success else 1
        else:
            # Display all commands
            output_format = 'json' if args.json else 'text'
            explorer.display_commands(commands, output_format)
        
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