#!/usr/bin/env python3
"""
============================================================================
test_lsp_filtering.py - Server-side vs Client-side Filtering Analysis
============================================================================

Purpose:
    Analyzes whether bkmr's built-in LSP implements server-side or client-side 
    filtering by testing incremental typing patterns and monitoring bkmr query 
    execution.

Test Method:
    Simulates typing sequence in a document and counts bkmr database queries
    to determine filtering behavior.

Expected Behaviors:
    • Client-side filtering (problematic): 1 bkmr query total
      - Only initial trigger executes bkmr search
      - Subsequent typing filters cached results on client
      - May miss relevant completions
    
    • Server-side filtering (correct): Multiple bkmr queries
      - Each keystroke triggers new bkmr search
      - Filters become progressively more specific
      - Shows all relevant completions

Usage:
    python3 scripts/lsp/test_lsp_filtering.py [options]

Examples:
    python3 scripts/lsp/test_lsp_filtering.py
    python3 scripts/lsp/test_lsp_filtering.py --db-path ../db/bkmr.db
    python3 scripts/lsp/test_lsp_filtering.py --debug

Output:
    - Real-time monitoring of bkmr command executions
    - Analysis of filtering behavior (server vs client-side)
    - Filter progression showing query refinement
    - Clear pass/fail determination

Diagnostics:
    This test helps diagnose completion issues where not all expected
    results appear in LSP clients (particularly in Neovim/Vim).

Environment Variables:
    BKMR_DB_URL: Database path
    RUST_LOG: Logging level for debugging
============================================================================
"""

import json
import subprocess
import sys
import threading
import time
import re
import argparse
import os
from typing import List, Dict, Any


class BkmrQueryMonitor:
    """Monitors and analyzes bkmr command executions during LSP completion testing."""
    
    def __init__(self):
        self.queries = []
        self.completion_responses = []
        self.document_changes = []
    
    def add_query(self, query_info: str, timestamp: float = None):
        """Store query information from server logs"""
        if timestamp is None:
            timestamp = time.time()
        
        self.queries.append({
            'query': query_info,
            'timestamp': timestamp
        })
    
    def add_document_change(self, content: str):
        """Track document content changes"""
        self.document_changes.append({
            'content': content,
            'timestamp': time.time()
        })
    
    def add_completion_response(self, response: Dict[str, Any]):
        """Store completion response for analysis"""
        if 'result' in response:
            result = response['result']
            if isinstance(result, list):
                item_count = len(result)
                response_type = 'Array'
            elif isinstance(result, dict) and 'items' in result:
                item_count = len(result['items'])
                response_type = 'List'
                is_incomplete = result.get('isIncomplete', False)
            else:
                item_count = 0
                response_type = 'Unknown'
                is_incomplete = None
                
            self.completion_responses.append({
                'type': response_type,
                'item_count': item_count,
                'is_incomplete': is_incomplete if response_type == 'List' else None,
                'timestamp': time.time()
            })
    
    def analyze_results(self, test_sequence: List[str]):
        """Analyze captured data to determine server vs client-side filtering behavior."""
        print(f"\n📊 BEHAVIOR ANALYSIS RESULTS:")
        print(f"   Document changes: {len(self.document_changes)}")
        print(f"   Completion requests: {len(self.completion_responses)}")
        print(f"   Database queries detected: {len(self.queries)}")
        
        if len(self.document_changes) > 0:
            print(f"\n📝 DOCUMENT SEQUENCE:")
            for i, change in enumerate(self.document_changes):
                print(f"   {i+1}. Content: '{change['content']}'")
        
        if len(self.queries) > 0:
            print(f"\n🔍 DATABASE QUERIES:")
            for i, query in enumerate(self.queries):
                print(f"   {i+1}. {query['query']}")
        
        print(f"\n📋 COMPLETION RESPONSES:")
        for i, resp in enumerate(self.completion_responses):
            incomplete_info = f", incomplete={resp['is_incomplete']}" if resp['is_incomplete'] is not None else ""
            print(f"   {i+1}. Type: {resp['type']}, Items: {resp['item_count']}{incomplete_info}")
        
        # Determine behavior based on query patterns
        print(f"\n🎯 FILTERING BEHAVIOR DETERMINATION:")
        
        # For bkmr's built-in LSP, we analyze based on completion response patterns
        # since database queries might be internal
        if len(self.completion_responses) == 0:
            print("   ❌ No completion responses detected - test may have failed")
            return False
        
        # Check if completion item counts change with each request
        item_counts = [r['item_count'] for r in self.completion_responses]
        unique_counts = len(set(item_counts))
        
        if unique_counts > 1:
            print("   ✅ SERVER-SIDE FILTERING DETECTED (Optimal)")
            print(f"   → Different item counts across requests: {item_counts}")
            print("   → Server refines results based on document content")
            print("   → This ensures comprehensive completion coverage")
            return True
        elif all(count > 0 for count in item_counts):
            print("   ⚠️  POSSIBLE CLIENT-SIDE FILTERING")
            print(f"   → Consistent item counts: {item_counts}")
            print("   → May indicate client-side caching")
            print("   → Could miss relevant completions")
            return False
        else:
            print("   ❓ INCONCLUSIVE RESULTS")
            print(f"   → Item counts: {item_counts}")
            print("   → Need more test data or different test sequence")
            return False


class LSPClient:
    def __init__(self, server_cmd: str, monitor: BkmrQueryMonitor, env: Optional[Dict[str, str]] = None):
        self.monitor = monitor
        
        # Set up environment
        process_env = os.environ.copy()
        if env:
            process_env.update(env)
        
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
        
        # Start stderr monitoring thread
        self.stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self.stderr_thread.start()

    def _read_stderr(self):
        """Monitor stderr for query information"""
        try:
            while True:
                line = self.process.stderr.readline()
                if not line:
                    break
                
                # Look for query patterns in logs
                if any(pattern in line for pattern in ["search", "query", "filter", "snippet", "SELECT"]):
                    self.monitor.add_query(line.strip())
                
                # Show important logs
                if any(keyword in line for keyword in ["ERROR", "WARN", "DEBUG"]):
                    print(f"[SERVER] {line.strip()}")
                    
        except Exception as e:
            pass

    def send_request(self, method: str, params: dict, request_id: int = None) -> dict:
        if request_id is None:
            self.request_id += 1
            request_id = self.request_id
            
        message = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        }
        
        return self._send_message(message)

    def send_notification(self, method: str, params: dict):
        message = {
            "jsonrpc": "2.0", 
            "method": method,
            "params": params
        }
        
        self._send_message(message, expect_response=False)

    def _send_message(self, message: dict, expect_response: bool = True):
        content = json.dumps(message)
        lsp_message = f"Content-Length: {len(content)}\r\n\r\n{content}"
        
        try:
            self.process.stdin.write(lsp_message)
            self.process.stdin.flush()
        except BrokenPipeError:
            return None
            
        if expect_response:
            return self._read_response()
        return None

    def _read_response(self):
        try:
            # Read Content-Length header
            while True:
                header_line = self.process.stdout.readline()
                if not header_line:
                    return None
                if header_line.startswith("Content-Length:"):
                    content_length = int(header_line.split(":")[1].strip())
                    break
            
            # Read empty line
            self.process.stdout.readline()
            
            # Read content
            content = self.process.stdout.read(content_length)
            response = json.loads(content)
            
            return response
                
        except Exception as e:
            return None

    def close(self):
        try:
            self.process.stdin.close()
            self.process.terminate()
            self.process.wait(timeout=2)
        except:
            self.process.kill()


def test_completion_behavior(args):
    """Test completion behavior using incremental typing sequence"""
    
    monitor = BkmrQueryMonitor()
    
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
    
    client = LSPClient(server_cmd, monitor, env)
    
    try:
        print("🧪 Testing completion filtering behavior...")
        print("   Goal: Determine if filtering is server-side or client-side")
        print()
        
        # Initialize LSP
        response = client.send_request("initialize", {
            "processId": None,
            "clientInfo": {"name": "test-filtering", "version": "0.1.0"},
            "capabilities": {
                "textDocument": {
                    "completion": {"completionItem": {"snippetSupport": True}}
                }
            },
            "workspaceFolders": None
        })
        
        if not response or "error" in response:
            print("❌ Initialize failed")
            return False
            
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        # Test sequence - simulate typing in a Rust file
        test_cases = [
            {"content": "", "description": "Empty document", "position": 0},
            {"content": "// ", "description": "After comment start", "position": 3},
            {"content": "// test", "description": "After word", "position": 7},
            {"content": "fn main() {\n    ", "description": "Inside function", "position": 16},
        ]
        
        uri = "file:///tmp/test-filtering.rs"
        
        # Initial document open
        client.send_notification("textDocument/didOpen", {
            "textDocument": {
                "uri": uri,
                "languageId": "rust",
                "version": 1,
                "text": ""
            }
        })
        
        for i, test_case in enumerate(test_cases):
            print(f"\n📝 Step {i+1}: {test_case['description']}")
            print(f"   Document: '{test_case['content']}'")
            
            # Track document change
            monitor.add_document_change(test_case['content'])
            
            # Update document content
            if i > 0:  # Skip first as document is already opened empty
                client.send_notification("textDocument/didChange", {
                    "textDocument": {
                        "uri": uri,
                        "version": i + 1
                    },
                    "contentChanges": [{
                        "text": test_case['content']
                    }]
                })
            
            # Small delay to let document sync
            time.sleep(0.1)
            
            # Calculate line and character position
            lines = test_case['content'].split('\n')
            line = len(lines) - 1
            character = len(lines[-1])
            
            # Request completion
            response = client.send_request("textDocument/completion", {
                "textDocument": {"uri": uri},
                "position": {"line": line, "character": character},
                "context": {
                    "triggerKind": 1,  # Manual invocation
                }
            })
            
            if response:
                monitor.add_completion_response(response)
                if 'result' in response:
                    result = response['result']
                    if isinstance(result, list):
                        print(f"   → Got {len(result)} completion items")
                    elif isinstance(result, dict) and 'items' in result:
                        print(f"   → Got {len(result['items'])} completion items")
                    else:
                        print(f"   → Got unexpected response format")
                else:
                    print(f"   → No completion results")
            else:
                print(f"   → No response received")
            
            # Delay between requests
            time.sleep(0.2)
        
        # Analyze results and determine filtering behavior
        success = monitor.analyze_results([tc['content'] for tc in test_cases])
        
        # Cleanup
        client.send_request("shutdown", {})
        client.send_notification("exit", {})
        
        return success
        
    finally:
        client.close()


def main():
    parser = argparse.ArgumentParser(
        description="Test bkmr LSP server-side vs client-side filtering behavior",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
This test determines whether the LSP server implements server-side or 
client-side filtering by monitoring completion behavior during incremental typing.

Server-side filtering (optimal):
  - Each keystroke triggers new database queries
  - Results are refined based on current document content
  - Ensures all relevant completions are available

Client-side filtering (problematic):
  - Initial query caches all results
  - Client filters the cached results locally
  - May miss relevant completions

Examples:
  %(prog)s                           # Basic filtering test
  %(prog)s --debug                  # Enable debug logging
  %(prog)s --db-path ../db/bkmr.db  # Use specific database
        """
    )
    
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging (RUST_LOG=debug)')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--no-interpolation', action='store_true',
                        help='Disable template interpolation')
    
    args = parser.parse_args()
    
    print("=" * 80)
    print("🔬 COMPLETION FILTERING BEHAVIOR ANALYSIS")
    print("=" * 80)
    print("📋 Testing: Server-side vs Client-side filtering behavior")
    print("🎯 Method: Incremental typing with query monitoring")
    print("=" * 80)
    
    success = test_completion_behavior(args)
    
    print("\n" + "=" * 80)
    if success:
        print("✅ RESULT: Server-side filtering is working optimally")
        print("   🔄 Dynamic query refinement based on document content")
        print("   🎯 This ensures comprehensive completion coverage")
        print("   💡 LSP clients should receive all relevant completions")
    else:
        print("⚠️  RESULT: Filtering behavior may need optimization")
        print("   📱 Consider implementing server-side filtering")
        print("   🔄 Each request should trigger fresh database queries")
        print("   🎯 This prevents missing relevant completions")
    print("=" * 80)
    
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())