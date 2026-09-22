# AICore — Background AI Core Service

**AICore** is a production-ready background AI service/daemon designed to provide a unified local IPC interface for desktop applications (such as FluxNotes), IDE extensions, and CLI tools.

## Architecture

AICore runs as a headless background process communicating over a Unix Domain Socket using a versioned JSON protocol.

```text
FluxNotes / VSCode / CLI
          │
    @aicore/sdk
          │
  Unix Domain Socket
          │
      AICore Daemon (Rust)
          │
    ChatGPT Web Provider
```

## Features

- **Modular Workspace Architecture**: Clean division across `aicore-protocol`, `aicore-storage`, `aicore-auth`, `aicore-providers/chatgpt`, `aicore-core`, `aicore-server`, `aicore-cli`.
- **ChatGPT Web Integration**: Reverse-engineered Sentinel chat-requirements, SHA3-512 Proof-of-Work solver, SSE streaming parser, Azure Blob file uploads, sandbox interpreter downloads.
- **Secure Credentials**: AES-256-GCM encrypted persistence with machine seed key derivation. Credentials remain strictly isolated within AICore.
- **Restricted Socket Permissions**: Unix socket bound at `$XDG_RUNTIME_DIR/aicore.sock` with `0700` mode.
- **TypeScript SDK (`@aicore/sdk`)**: Clean async methods and `AsyncIterable` streaming.
- **CLI (`aicore`)**: Command line control for status, auth, logout, logs, stop, restart, version.
- **Management UI Dashboard**: React + TypeScript + Tailwind UI.

## Quick Start

### Build & Run Daemon
```bash
cargo run --bin aicore-server
```

### CLI Usage
```bash
cargo run --bin aicore -- status
```

### SDK Usage
```typescript
import { AICore } from "@aicore/sdk";

const ai = await AICore.connect();
const response = await ai.chat.send({
  sessionId: "note_123",
  message: "Explain photosynthesis"
});
console.log(response.text);
```

## Documentation

- [Architecture Guide](docs/architecture.md)
- [Protocol Specification](docs/protocol.md)
- [ChatGPT JS Migration Reference](docs/chatgpt-migration.md)
- [Security Model](docs/security.md)
- [Development Setup](docs/development.md)
