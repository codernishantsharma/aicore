# AICore Protocol v1 Specification

AICore communicates over a local Unix Domain Socket using newline-delimited JSON messages.

## Request Framing

```json
{
  "version": 1,
  "id": "req_123",
  "method": "chat.send",
  "params": {
    "sessionId": "fluxnotes-note-123",
    "message": "Explain quantum computing"
  },
  "client": {
    "id": "fluxnotes",
    "name": "FluxNotes",
    "version": "1.0.0"
  }
}
```

## Response Types

### Result Frame
```json
{
  "version": 1,
  "id": "req_123",
  "type": "result",
  "result": {
    "messageId": "msg_456",
    "conversationId": "conv_789",
    "text": "Quantum computing is..."
  }
}
```

### Delta Frame (Streaming)
```json
{
  "version": 1,
  "id": "req_123",
  "type": "delta",
  "data": {
    "text": "Quantum "
  }
}
```

### Done Frame (Stream Completion)
```json
{
  "version": 1,
  "id": "req_123",
  "type": "done"
}
```

### Error Frame
```json
{
  "version": 1,
  "id": "req_123",
  "type": "error",
  "error": {
    "code": "AUTH_REQUIRED",
    "message": "AICore is not authenticated with ChatGPT"
  }
}
```

## Supported Methods

- `system.ping`
- `core.info`
- `auth.status`
- `auth.login`
- `auth.logout`
- `chat.send`
- `chat.stream`
- `conversation.create`
- `conversation.get`
- `conversation.reset`
- `file.upload`
- `file.download`
- `image.download`
