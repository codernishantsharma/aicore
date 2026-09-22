export interface ClientOptions {
  clientId?: string;
  clientName?: string;
  clientVersion?: string;
  socketPath?: string;
  timeoutMs?: number;
}

export interface ClientInfo {
  id: string;
  name: string;
  version: string;
}

export interface RequestPayload {
  version: number;
  id: string;
  method: string;
  params?: Record<string, unknown>;
  client?: ClientInfo;
}

export interface ResponseResult {
  version: number;
  id: string;
  type: "result";
  result: unknown;
}

export interface ResponseDelta {
  version: number;
  id: string;
  type: "delta";
  data: {
    text?: string;
    conversationId?: string;
    messageId?: string;
    [key: string]: unknown;
  };
}

export interface ResponseDone {
  version: number;
  id: string;
  type: "done";
}

export interface ResponseError {
  version: number;
  id: string;
  type: "error";
  error: {
    code: string;
    message: string;
    details?: unknown;
  };
}

export type ResponsePacket =
  | ResponseResult
  | ResponseDelta
  | ResponseDone
  | ResponseError;

export interface ChatSendParams {
  sessionId: string;
  message: string;
  attachments?: Array<{ fileId: string }>;
}

export interface ChatSendResult {
  messageId: string;
  conversationId: string;
  text: string;
}

export interface StreamChunk {
  type: "delta" | "done" | "error";
  text?: string;
  conversationId?: string;
  messageId?: string;
  error?: string;
}
