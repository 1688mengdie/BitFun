# Quick Start: BitFun ACP Integration

## For Developers

### Building the CLI

```bash
cd /Users/harryfan/code/bitfun
cargo build --package bitfun-cli
```

The binary will be at: `target/debug/bitfun-cli` or `target/release/bitfun-cli`

### Running ACP Server

```bash
# Development build
./target/debug/bitfun-cli acp

# Release build
./target/release/bitfun-cli acp

# With workspace
./target/debug/bitfun-cli acp --workspace /path/to/project

# Verbose mode (debug logs)
./target/debug/bitfun-cli -v acp
```

### Testing the Implementation

#### Test 1: Initialize Protocol
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":true},"terminal":true}}}' | ./target/debug/bitfun-cli acp
```

Expected response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": 1,
    "agentCapabilities": {...},
    "agentInfo": {
      "name": "BitFun",
      "version": "0.2.3"
    }
  }
}
```

#### Test 2: Create Session
```bash
echo '{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp/test"}}' | ./target/debug/bitfun-cli acp
```

Expected response:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "sessionId": "...",
    "modes": {...}
  }
}
```

#### Test 3: List Tools
```bash
echo '{"jsonrpc":"2.0","id":3,"method":"tools/list"}' | ./target/debug/bitfun-cli acp
```

Expected response:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "tools": [
      {"name": "LS", ...},
      {"name": "Read", ...},
      {"name": "Write", ...},
      // 40+ more tools
    ]
  }
}
```

### Integration with Your Editor

To integrate BitFun with your editor/IDE:

1. **Spawn the process**: `bitfun-cli acp`
2. **Communicate via stdio**: 
   - Write JSON-RPC requests to stdin
   - Read JSON-RPC responses from stdout
3. **Follow ACP protocol**: Use standard ACP method names and parameters

Example pseudo-code:
```typescript
// Spawn BitFun ACP server
const bitfun = spawn('bitfun-cli', ['acp']);

// Send initialize request
bitfun.stdin.write(JSON.stringify({
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: { protocolVersion: 1 }
}) + '\n');

// Read response
bitfun.stdout.on('data', (data) => {
  const response = JSON.parse(data.toString());
  // Handle response...
});
```

### What Works Now

- ✅ Protocol initialization
- ✅ Session creation
- ✅ Tool listing
- ✅ Session listing
- ✅ JSON-RPC message handling

### What's Coming Next

- 🚧 Full message execution (session/prompt)
- 🚧 Tool execution (tools/call)
- 🚧 Progress streaming (session/update)
- 🚧 Permission handling
- 🚧 Session persistence

### Architecture Overview

```
Your Editor
    ↓ (JSON-RPC over stdio)
BitFun ACP Server
    ↓ (calls handlers)
ACP Handlers
    ↓ (integrates with)
BitFun Core (Agentic System)
    ↓ (executes)
AI Model + Tools
```

### Key Files to Understand

1. **src/apps/cli/src/acp/mod.rs**: 
   - Server implementation
   - Stdio handling

2. **src/apps/cli/src/acp/handlers.rs**:
   - Method implementations
   - Integration points

3. **src/apps/cli/src/acp/protocol.rs**:
   - Message types
   - JSON structures

4. **src/apps/cli/src/acp/session.rs**:
   - Session state
   - ID mapping

### Extending the Implementation

To add a new ACP method:

1. **Define types in protocol.rs**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YourMethodParams {
    pub param1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YourMethodResult {
    pub result1: String,
}
```

2. **Add handler in handlers.rs**:
```rust
fn handle_your_method(request: &JsonRpcRequest) -> Result<serde_json::Value> {
    let params: YourMethodParams = request
        .params
        .as_ref()
        .ok_or_else(|| anyhow!("Missing params"))?
        .clone()
        .try_into()?;
    
    // Your implementation
    
    let result = YourMethodResult {
        result1: "value".to_string(),
    };
    
    Ok(serde_json::to_value(result)?)
}
```

3. **Register in handle_method**:
```rust
"your/method" => handle_your_method(&request)?,
```

### Testing Framework

Use the provided test scripts:

```bash
# Bash tests
bash scripts/test-acp.sh

# Node.js tests
node scripts/test-acp.js
```

Or create your own tests following the JSON-RPC format.

### Debugging Tips

1. **Enable verbose logging**:
```bash
bitfun-cli -v acp
```

2. **Check logs** (TUI mode logs to file):
```bash
tail -f ~/.bitfun-cli/logs/bitfun-cli.log
```

3. **Validate JSON-RPC**:
   - Ensure proper "jsonrpc": "2.0"
   - Include "id" for requests expecting responses
   - Use correct parameter names (camelCase)

4. **Test incrementally**:
   - Start with `initialize`
   - Then `session/new`
   - Then `tools/list`

### Common Issues

**Issue**: No response received
- Check JSON format
- Ensure "id" is present
- Check stderr for errors

**Issue**: Method not found
- Verify method name spelling
- Check if method is implemented

**Issue**: Parse error
- Validate JSON syntax
- Check parameter structure

### Contributing

When contributing to ACP support:

1. Follow ACP specification
2. Test with multiple clients
3. Document new methods
4. Update ACP_README.md
5. Add test cases

### Resources

- [ACP_README.md](src/apps/cli/ACP_README.md) - Full documentation
- [ACP_IMPLEMENTATION_SUMMARY.md](ACP_IMPLEMENTATION_SUMMARY.md) - Implementation details
- [ACP Specification](https://agentclientprotocol.com)

---

**Quick Reference**:

```bash
# Build
cargo build --package bitfun-cli

# Run
./target/debug/bitfun-cli acp

# Test
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}' | ./target/debug/bitfun-cli acp
```

Happy coding! 🚀