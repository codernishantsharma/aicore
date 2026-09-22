# AICore Security Model

## Socket Permissions & Local IPC
- The Unix Domain Socket is created dynamically at `$XDG_RUNTIME_DIR/aicore.sock` (or `~/.ai-core/aicore.sock`).
- File permissions are set to `0700` (`S_IRWXU`), restricting socket read/write access strictly to the owner user account.

## Credential Encryption at Rest
- Sensitive tokens and cookies are encrypted using AES-256-GCM.
- Cryptographic keys are derived from machine seeds (`/etc/machine-id` or DBus machine ID) combined with domain salt via SHA-256 key derivation.
- Credentials are never exposed through the public client API.

## File System Validation & Path Traversal Defense
- All file paths supplied by clients are validated to prevent directory traversal attacks (`..`).
- File operations enforce directory boundaries and size limits.

## Log Hygiene
- Structured logging using `tracing` sanitizes logs to ensure access tokens, cookies, auth headers, and upload URLs are never recorded.
