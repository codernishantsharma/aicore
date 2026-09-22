# AICore Architecture

AICore is a background AI daemon service designed to provide a unified local IPC interface for desktop applications, IDE extensions, and CLI tools.

## Architecture Diagram

```text
                         ┌──────────────────────┐
                         │      FluxNotes       │
                         │      VS Code         │
                         │       CLI            │
                         │   Other Applications │
                         └──────────┬───────────┘
                                    │
                              AICore SDK (@aicore/sdk)
                                    │
                                    ▼
                         ┌──────────────────────┐
                         │    Unix Domain       │
                         │       Socket         │
                         │     JSON Protocol    │
                         └──────────┬───────────┘
                                    │
                                    ▼
                    ┌────────────────────────────┐
                    │          AICore            │
                    │       Rust Service        │
                    │                            │
                    │  ┌──────────────────────┐  │
                    │  │ Request Router       │  │
                    │  ├──────────────────────┤  │
                    │  │ Session Manager      │  │
                    │  ├──────────────────────┤  │
                    │  │ Authentication       │  │
                    │  ├──────────────────────┤  │
                    │  │ Provider Manager     │  │
                    │  ├──────────────────────┤  │
                    │  │ ChatGPT Provider     │  │
                    │  ├──────────────────────┤  │
                    │  │ File Manager         │  │
                    │  ├──────────────────────┤  │
                    │  │ Streaming Manager    │  │
                    │  └──────────────────────┘  │
                    └──────────────┬─────────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    ▼                             ▼
             ChatGPT Web                    Future Providers
             Integration                  OpenAI / Gemini /
                                           Local Models / etc.
```

## Workspace Crate Breakdown

1. `aicore-protocol`: Protocol v1 serialization, request/response/stream framing, error codes.
2. `aicore-storage`: SQLite persistence layer for sessions, conversation parent message IDs, app permissions.
3. `aicore-auth`: AES-256-GCM encrypted persistence for ChatGPT credentials, machine key derivation.
4. `aicore-providers/chatgpt`: Native Rust ChatGPT Web provider (SHA3-512 PoW solver, Sentinel requirements, SSE parser, Azure Blob file uploader, sandbox downloader).
5. `aicore-core`: Business logic, request router, session manager, permission manager, provider traits.
6. `aicore-server`: Background Unix Domain Socket daemon with restricted `0700` permissions.
7. `aicore-cli`: Command-line interface tool (`aicore`).
8. `sdk/typescript`: TypeScript SDK (`@aicore/sdk`).
9. `apps/aicore-ui`: React + Tailwind management UI dashboard.
