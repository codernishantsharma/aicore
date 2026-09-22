# ChatGPT Web Integration Migration Reference

This document maps the JS function signatures and behaviors from `chatgpt.js` to their Rust implementations in AICore.

| Existing JavaScript (`chatgpt.js`) | Rust Component (`aicore-provider-chatgpt` / `aicore-auth`) | Purpose & Behavior |
| ---------------------------------- | --------------------------------------------------------- | ------------------ |
| `_getToken()` / `/api/auth/session` | `AuthRetryManager::refresh_session_from_endpoint()` | Requests `/api/auth/session` using stored cookies to retrieve updated access token and session metadata. |
| `_getScriptsAndDpl()`              | `ChatGPTRequirements` | Fetches script requirements from ChatGPT frontend. |
| `_solvePOW()`                      | `PowSolver::solve()` | Solves SHA3-512 Proof-of-Work nonces for ChatGPT sentinel challenge (`seed`, `difficulty`, `user_agent`). |
| `_getRequirementsAndPOW()`         | `ChatGPTRequirements::fetch_and_solve()` | Calls `POST /backend-api/sentinel/chat-requirements` and solves PoW challenge to acquire `openai-sentinel-proof-token` and `openai-sentinel-chat-requirements-token`. |
| `_parseSSEStream()`                | `SseParser::parse_line()` | Parses asynchronous Server-Sent Events (`data: ...`), tracks message deltas, and emits normalized `Response::Delta` / `Response::Done` frames. |
| `uploadFileToChatGPT()`            | `ChatGPTFileClient::upload_file()` | Executes file upload flow: `POST /backend-api/files` -> Azure Blob `PUT` (`x-ms-blob-type: BlockBlob`) -> `POST /backend-api/files/{id}/uploaded`. |
| `send()`                           | `ChatGPTProvider::send()` / `ChatGPTProvider::stream()` | Constructs payload for `POST /backend-api/conversation` with parent message ID tracking, sentinel headers, and SSE streaming. |
| `downloadSandboxImage()`           | `ChatGPTFileClient::download_sandbox_file()` | Downloads interpreter files via `/backend-api/conversation/{id}/interpreter/download` and asset pointers (`sediment://`). |
