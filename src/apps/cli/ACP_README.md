# BitFun CLI ACP (Agent Client Protocol) Support

## Overview

The `bitfun acp` command implements the Agent Client Protocol (ACP) for BitFun CLI, enabling integration with ACP-compatible editors and IDEs. ACP is a JSON-RPC 2.0 based protocol that standardizes communication between code editors and AI coding agents.

## Features

- **JSON-RPC 2.0 over stdio**: Reads requests from stdin, writes responses to stdout
- **Session Management**: Create and manage ACP sessions
- **Tool Integration**: Access to all BitFun tools through the protocol
- **Multi-mode Support**: Ask, Architect, and Code modes

## Usage

### Starting the ACP Server

```bash
bitfun acp
```

This starts an ACP server that:
- Reads JSON-RPC requests from stdin
- Writes JSON-RPC responses to stdout
- Logs debug information to stderr (if verbose mode is enabled)

### With Workspace

```bash
bitfun acp --workspace /path/to/project
```

### Verbose Mode

```bash
bitfun -v acp
```

## Protocol Methods Implemented

### Lifecycle Methods

#### `initialize`
Establishes connection and negotiates protocol capabilities.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": 1,
    "clientCapabilities": {
      "fs": {
        "readTextFile": true,
        "writeTextFile": true
      },
      "terminal": true
    },
    "clientInfo": {
      "name": "MyEditor",
      "version": "1.0.0"
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": 1,
    "agentCapabilities": {
      "loadSession": true,
      "mcpCapabilities": {
        "http": true,
        "sse": true
      },
      "promptCapabilities": {
        "audio": false,
        "embeddedContext": true,
        "image": true
      },
      "sessionCapabilities": {
        "list": true
      }
    },
    "agentInfo": {
      "name": "BitFun",
      "version": "0.2.3"
    },
    "authMethods": []
  }
}
```

### Session Methods

#### `session/new`
Creates a new ACP session.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "session/new",
  "params": {
    "cwd": "/path/to/workspace",
    "mcpServers": []
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "sessionId": "550e8400-e29b-41d4-a716-446655440000",
    "modes": {
      "availableModes": [
        {
          "id": "ask",
          "name": "Ask",
          "description": "Ask questions and get information"
        },
        {
          "id": "architect",
          "name": "Architect",
          "description": "Design and plan architecture"
        },
        {
          "id": "code",
          "name": "Code",
          "description": "Write and modify code"
        }
      ],
      "currentMode": "code"
    }
  }
}
```

#### `session/prompt`
Send a user message to the agent.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "session/prompt",
  "params": {
    "sessionId": "550e8400-e29b-41d4-a716-446655440000",
    "prompt": [
      {
        "type": "text",
        "text": "Create a new file called hello.rs with a Hello World program"
      }
    ]
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "stopReason": "complete"
  }
}
```

#### `session/cancel` (Notification)
Cancel ongoing operations.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "session/cancel",
  "params": {
    "sessionId": "550e8400-e29b-41d4-a716-446655440000"
  }
}
```

No response (notification).

#### `session/list`
List all sessions.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "session/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "sessions": [
      {
        "sessionId": "550e8400-e29b-41d4-a716-446655440000",
        "cwd": "/path/to/workspace"
      }
    ]
  }
}
```

### Tools Methods

#### `tools/list`
List all available tools.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "tools": [
      {
        "name": "LS",
        "description": "List files and directories",
        "inputSchema": {
          "type": "object",
          "properties": {
            "path": {
              "type": "string",
              "description": "Directory path to list"
            }
          }
        }
      },
      {
        "name": "Read",
        "description": "Read file contents",
        "inputSchema": {
          "type": "object",
          "properties": {
            "file_path": {
              "type": "string",
              "description": "File path to read"
            }
          },
          "required": ["file_path"]
        }
      }
      // ... more tools
    ]
  }
}
```

#### `tools/call`
Execute a tool.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "sessionId": "550e8400-e29b-41d4-a716-446655440000",
    "name": "LS",
    "arguments": {
      "path": "/path/to/workspace"
    }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "List of files and directories..."
      }
    ]
  }
}
```

## Testing the ACP Server

### Manual Testing

You can test the ACP server manually using simple JSON-RPC messages:

```bash
# Test initialization
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}' | bitfun acp

# Create a session
echo '{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/tmp/test"}}' | bitfun acp

# List tools
echo '{"jsonrpc":"2.0","id":3,"method":"tools/list"}' | bitfun acp
```

### Using with ACP-compatible Editors

ACP-compatible editors like Zed, Cursor, or others can connect to BitFun using:

```
bitfun acp
```

The editor will send JSON-RPC messages over stdio and receive responses.

### Using acpx

You can also use [acpx](https://github.com/openclaw/acpx) as a headless client:

```bash
# Install acpx
npm install -g acpx@latest

# Use BitFun as the backend agent
acpx bitfun "implement a hello world program"
```

Note: You'll need to add BitFun to acpx's agent registry first.

## Architecture

### Module Structure

```
src/apps/cli/src/acp/
├── mod.rs           - Main module, AcpServer implementation
├── protocol.rs      - JSON-RPC types and ACP protocol definitions
├── handlers.rs      - Request handlers for each ACP method
└── session.rs       - Session management (ACP <-> BitFun mapping)
```

### Key Components

1. **AcpServer**: Main server that handles JSON-RPC communication
2. **JsonRpcRequest/Response**: Protocol message types
3. **AcpSessionManager**: Maps ACP sessions to BitFun sessions
4. **Handlers**: Method-specific handlers that integrate with BitFun core

### Integration with BitFun Core

The ACP server integrates with:
- **AgenticSystem**: Conversation coordinator and execution engine
- **ToolRegistry**: Access to all BitFun tools
- **SessionManager**: Session persistence and management
- **AIClientFactory**: AI model connections

## Limitations (Current Implementation)

### What's Implemented
- ✅ `initialize` - Protocol negotiation
- ✅ `session/new` - Create new sessions
- ✅ `session/list` - List sessions
- ✅ `tools/list` - List available tools
- ✅ Basic JSON-RPC protocol handling

### What's Partially Implemented
- ⚠️ `session/prompt` - Accepts messages but doesn't execute agentic workflow
- ⚠️ `tools/call` - Placeholder response

### What's Not Yet Implemented
- ❌ `session/load` - Resume existing sessions
- ❌ `session/update` notifications - Stream progress updates
- ❌ Permission requests for tool execution
- ❌ File system operations (fs/read_text_file, fs/write_text_file)
- ❌ Terminal operations
- ❌ MCP server connections

## Future Work

To complete the implementation:

1. **Implement full `session/prompt` workflow**:
   - Create BitFun session
   - Send message to ConversationCoordinator
   - Stream events as session/update notifications
   - Handle tool calls and permission requests
   - Return proper stop reason

2. **Implement `session/load`**:
   - Resume persisted sessions
   - Restore session state

3. **Implement notifications**:
   - Send session/update notifications for streaming progress
   - Message chunks, tool calls, thought chunks

4. **Implement client methods**:
   - fs/read_text_file
   - fs/write_text_file
   - terminal/create, output, release
   - session/request_permission

5. **Implement MCP integration**:
   - Connect to MCP servers specified in session/new
   - Register MCP tools dynamically

## Resources

- [ACP Specification](https://agentclientprotocol.com)
- [ACP GitHub](https://github.com/agentclientprotocol/agent-client-protocol)
- [acpx - Headless ACP Client](https://github.com/openclaw/acpx)
- [BitFun Documentation](../../README.md)

## Contributing

To contribute to ACP support:

1. Review the ACP specification
2. Implement missing methods in `handlers.rs`
3. Add proper integration with BitFun's agentic system
4. Write tests for protocol compliance
5. Document new features in this README

## License

MIT License - See LICENSE file for details.