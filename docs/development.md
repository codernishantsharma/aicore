# AICore Development Guide

## Prerequisites
- Rust 1.75+
- Node.js 18+ and npm
- Linux or macOS environment

## Workspace Crate Commands

Build all Rust crates:
```bash
cargo build
```

Run all unit and integration tests:
```bash
cargo test
```

Start the background daemon server:
```bash
cargo run --bin aicore-server
```

Run the CLI tool:
```bash
cargo run --bin aicore -- status
```

Build the TypeScript SDK:
```bash
cd sdk/typescript
npm install
npm run build
```

Build the Management UI:
```bash
cd apps/aicore-ui
npm install
npm run build
```
