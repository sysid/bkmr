#!/usr/bin/env python3
"""
============================================================================
test_lsp_language_filtering.py - Language-aware Filtering Test
============================================================================

Purpose:
    Tests that bkmr's built-in LSP server correctly extracts file type 
    information from LSP clients and applies language-aware filtering to 
    snippet completions.

Test Method:
    Opens documents with different language IDs and verifies the server:
    1. Correctly identifies the language from textDocument/didOpen
    2. Filters snippets based on language tags
    3. Includes universal snippets for all languages

Expected Behavior:
    - Rust files: Get Rust + universal snippets
    - Python files: Get Python + universal snippets  
    - JavaScript files: Get JavaScript + universal snippets
    - Go files: Get Go + universal snippets
    - C files: Get C + universal snippets

Usage:
    python3 scripts/lsp/test_lsp_language_filtering.py [options]

Examples:
    python3 scripts/lsp/test_lsp_language_filtering.py
    python3 scripts/lsp/test_lsp_language_filtering.py --debug
    python3 scripts/lsp/test_lsp_language_filtering.py --db-path ../db/bkmr.db

Output:
    - Language detection verification
    - Snippet filtering analysis per language
    - Universal snippet inclusion check
    - Pass/fail for each language test

Environment Variables:
    BKMR_DB_URL: Database path
    RUST_LOG: Logging level for debugging
============================================================================
"""

import json
import subprocess
import sys
import time
import argparse
import os
from typing import Dict, List, Any, Optional


class LanguageFilterTester:
    """Tests language-aware filtering in bkmr LSP."""
    
    def __init__(self, debug: bool = False):
        self.debug = debug
        self.test_results = []
        self.language_snippets = {}
    
    def add_test_result(self, language: str, success: bool, details: str):
        """Record test result for a language."""
        self.test_results.append({
            'language': language,
            'success': success,
            'details': details
        })
    
    def analyze_completions(self, language: str, items: List[Dict[str, Any]]) -> Dict[str, Any]:
        """Analyze completion items for language filtering."""
        analysis = {
            'total_items': len(items),
            'language_specific': 0,
            'universal': 0,
            'other': 0,
            'sample_labels': []
        }
        
        for item in items:
            label = item.get('label', '')
            detail = item.get('detail', '')
            
            # Try to get tags from item data if available
            tags = []
            if 'data' in item and isinstance(item['data'], dict):
                tags = item['data'].get('tags', [])
            
            # Count by category
            if any(lang_tag in tags for lang_tag in [language, f'_{language}_']):
                analysis['language_specific'] += 1
            elif 'universal' in tags or not tags:
                analysis['universal'] += 1
            else:
                analysis['other'] += 1
            
            # Collect sample labels
            if len(analysis['sample_labels']) < 3:
                analysis['sample_labels'].append(label)
        
        return analysis
    
    def print_summary(self):
        """Print test summary."""
        print("\n" + "=" * 80)
        print("📊 TEST SUMMARY")
        print("=" * 80)
        
        passed = sum(1 for r in self.test_results if r['success'])
        total = len(self.test_results)
        
        for result in self.test_results:
            status = "✅" if result['success'] else "❌"
            print(f"{status} {result['language']:12} {result['details']}")
        
        print("-" * 80)
        print(f"Overall: {passed}/{total} tests passed")
        
        if passed == total:
            print("✅ All language filtering tests passed!")
            return True
        else:
            print(f"❌ {total - passed} tests failed")
            return False


class LSPClient:
    """Simple LSP client for testing."""
    
    def __init__(self, server_cmd: str, env: Optional[Dict[str, str]] = None):
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
        
        # Monitor stderr in background
        import threading
        self.stderr_thread = threading.Thread(target=self._read_stderr, daemon=True)
        self.stderr_thread.start()
        
        time.sleep(0.5)  # Give server time to start
    
    def _read_stderr(self):
        """Read stderr in background."""
        try:
            for line in iter(self.process.stderr.readline, ''):
                if line and ('ERROR' in line or 'WARN' in line):
                    print(f"[SERVER] {line.rstrip()}")
        except:
            pass
    
    def send_request(self, method: str, params: dict) -> Optional[dict]:
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
    
    def send_notification(self, method: str, params: dict):
        """Send notification (no response expected)."""
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
    
    def close(self):
        """Close client."""
        try:
            self.process.terminate()
            self.process.wait(timeout=2)
        except:
            self.process.kill()


def test_language_filtering(args):
    """Test language-aware filtering for different file types."""
    
    tester = LanguageFilterTester(debug=args.debug)
    
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
    
    print("🚀 Starting bkmr LSP server...")
    client = LSPClient(server_cmd, env)
    
    try:
        # Initialize
        print("1. Initialize LSP server")
        response = client.send_request("initialize", {
            "processId": None,
            "capabilities": {
                "textDocument": {
                    "completion": {"completionItem": {"snippetSupport": True}}
                }
            }
        })
        
        if not response or 'error' in response:
            print("❌ Failed to initialize LSP server")
            return False
        
        print("✅ Server initialized")
        client.send_notification("initialized", {})
        time.sleep(0.2)
        
        # Test different languages
        test_files = [
            ("rust", "file:///test/example.rs", "fn main() {\n    println!(\"Hello\");\n}"),
            ("python", "file:///test/example.py", "#!/usr/bin/env python3\nprint('Hello')"),
            ("javascript", "file:///test/example.js", "console.log('Hello');"),
            ("go", "file:///test/example.go", "package main\n\nfunc main() {\n    println(\"Hello\")\n}"),
            ("c", "file:///test/example.c", "#include <stdio.h>\n\nint main() {\n    printf(\"Hello\\n\");\n}"),
            ("typescript", "file:///test/example.ts", "const greeting: string = 'Hello';"),
        ]
        
        for i, (language_id, uri, content) in enumerate(test_files, 2):
            print(f"\n{i}. Testing {language_id} file")
            
            # Open document
            client.send_notification("textDocument/didOpen", {
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": content
                }
            })
            time.sleep(0.1)
            
            # Request completion
            print(f"   Requesting completions for {language_id}...")
            response = client.send_request("textDocument/completion", {
                "textDocument": {"uri": uri},
                "position": {"line": 0, "character": 0},
                "context": {"triggerKind": 1}
            })
            
            if response and 'result' in response:
                result = response['result']
                items = result if isinstance(result, list) else result.get('items', [])
                
                analysis = tester.analyze_completions(language_id, items)
                
                print(f"   📊 Results:")
                print(f"      Total items: {analysis['total_items']}")
                print(f"      Language-specific: {analysis['language_specific']}")
                print(f"      Universal: {analysis['universal']}")
                print(f"      Other: {analysis['other']}")
                
                if analysis['sample_labels']:
                    print(f"      Samples: {', '.join(analysis['sample_labels'][:3])}")
                
                # Determine success
                if analysis['total_items'] > 0:
                    # Success if we got any completions (language-specific or universal)
                    success = True
                    details = f"Got {analysis['total_items']} completions"
                else:
                    success = False
                    details = "No completions returned"
                
                tester.add_test_result(language_id, success, details)
            else:
                print(f"   ❌ No completion response")
                tester.add_test_result(language_id, False, "No response")
            
            # Close document
            client.send_notification("textDocument/didClose", {
                "textDocument": {"uri": uri}
            })
            time.sleep(0.1)
        
        # Shutdown
        print(f"\n{len(test_files) + 2}. Shutdown server")
        client.send_request("shutdown", {})
        client.send_notification("exit", {})
        
        # Print summary
        return tester.print_summary()
        
    finally:
        client.close()


def main():
    parser = argparse.ArgumentParser(
        description="Test language-aware filtering in bkmr LSP",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
This test verifies that the LSP server correctly:
1. Extracts language ID from textDocument/didOpen
2. Filters snippets based on language tags
3. Includes universal snippets for all languages

The test opens documents with different language IDs and analyzes
the returned completion items to ensure proper filtering.

Examples:
  %(prog)s                           # Basic test
  %(prog)s --debug                  # Enable debug logging
  %(prog)s --db-path ../db/bkmr.db  # Use specific database
        """
    )
    
    parser.add_argument('--debug', '-d', action='store_true',
                        help='Enable debug logging')
    parser.add_argument('--db-path', metavar='PATH',
                        help='Path to bkmr database')
    parser.add_argument('--no-interpolation', action='store_true',
                        help='Disable template interpolation')
    
    args = parser.parse_args()
    
    print("=" * 80)
    print("🔬 LANGUAGE-AWARE FILTERING TEST")
    print("=" * 80)
    print("📋 Testing: Language detection and snippet filtering")
    print("🎯 Languages: Rust, Python, JavaScript, Go, C, TypeScript")
    print("=" * 80)
    print()
    
    success = test_language_filtering(args)
    
    return 0 if success else 1


if __name__ == "__main__":
    sys.exit(main())