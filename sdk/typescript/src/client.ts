import { SocketConnection } from "./socket";
import {
  ClientOptions,
  ChatSendParams,
  ChatSendResult,
  StreamChunk,
  ResponsePacket,
  RequestPayload,
} from "./types";

export class AICore {
  private conn: SocketConnection;
  private options: ClientOptions;

  private constructor(options: ClientOptions, conn: SocketConnection) {
    this.options = options;
    this.conn = conn;
  }

  static async connect(options: ClientOptions = {}): Promise<AICore> {
    const conn = new SocketConnection();
    await conn.connect(options.socketPath);
    return new AICore(options, conn);
  }

  get chat() {
    return {
      send: async (params: ChatSendParams): Promise<ChatSendResult> => {
        const res = await this.request("chat.send", params as unknown as Record<string, unknown>);
        return res as ChatSendResult;
      },
      stream: (params: ChatSendParams): AsyncIterable<StreamChunk> => {
        return this.streamRequest("chat.stream", params as unknown as Record<string, unknown>);
      },
    };
  }

  get auth() {
    return {
      status: async () => this.request("auth.status"),
      login: async () => this.request("auth.login"),
      logout: async () => this.request("auth.logout"),
    };
  }

  get conversation() {
    return {
      create: async () => this.request("conversation.create"),
      get: async (sessionId: string) => this.request("conversation.get", { sessionId }),
      reset: async (sessionId: string) => this.request("conversation.reset", { sessionId }),
    };
  }

  get file() {
    return {
      upload: async (params: { path: string; name?: string }) => this.request("file.upload", params as unknown as Record<string, unknown>),
      download: async (params: { fileId: string }) => this.request("file.download", params as unknown as Record<string, unknown>),
    };
  }

  get image() {
    return {
      download: async (params: { imageId: string }) => this.request("image.download", params as unknown as Record<string, unknown>),
    };
  }

  async close(): Promise<void> {
    this.conn.close();
  }

  private async request(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    const reqId = `req_${Math.random().toString(36).substring(2, 11)}`;
    const payload: RequestPayload = {
      version: 1,
      id: reqId,
      method,
      params,
      client: {
        id: this.options.clientId || "default",
        name: this.options.clientName || "SDK Client",
        version: this.options.clientVersion || "1.0.0",
      },
    };

    return new Promise((resolve, reject) => {
      const timeoutMs = this.options.timeoutMs || 30000;
      const timer = setTimeout(() => {
        this.conn.unregisterListener(reqId);
        reject(new Error(`Request ${reqId} timed out`));
      }, timeoutMs);

      this.conn.registerListener(reqId, (packet: ResponsePacket) => {
        if (packet.type === "result") {
          clearTimeout(timer);
          this.conn.unregisterListener(reqId);
          resolve(packet.result);
        } else if (packet.type === "error") {
          clearTimeout(timer);
          this.conn.unregisterListener(reqId);
          reject(new Error(`[${packet.error.code}] ${packet.error.message}`));
        }
      });

      this.conn.send(payload);
    });
  }

  private streamRequest(
    method: string,
    params: Record<string, unknown> = {}
  ): AsyncIterable<StreamChunk> {
    const reqId = `req_${Math.random().toString(36).substring(2, 11)}`;
    const conn = this.conn;
    const payload: RequestPayload = {
      version: 1,
      id: reqId,
      method,
      params,
      client: {
        id: this.options.clientId || "default",
        name: this.options.clientName || "SDK Client",
        version: this.options.clientVersion || "1.0.0",
      },
    };

    const queue: StreamChunk[] = [];
    let resolveNext: (() => void) | null = null;
    let isDone = false;

    conn.registerListener(reqId, (packet: ResponsePacket) => {
      if (packet.type === "delta") {
        queue.push({
          type: "delta",
          text: packet.data.text,
          conversationId: packet.data.conversationId,
          messageId: packet.data.messageId,
        });
      } else if (packet.type === "done") {
        queue.push({ type: "done" });
        isDone = true;
      } else if (packet.type === "error") {
        queue.push({ type: "error", error: packet.error.message });
        isDone = true;
      }

      if (resolveNext) {
        resolveNext();
        resolveNext = null;
      }
    });

    conn.send(payload);

    return {
      [Symbol.asyncIterator]() {
        return {
          async next(): Promise<IteratorResult<StreamChunk>> {
            while (queue.length === 0 && !isDone) {
              await new Promise<void>((res) => {
                resolveNext = res;
              });
            }

            if (queue.length > 0) {
              const item = queue.shift()!;
              if (item.type === "done") {
                conn.unregisterListener(reqId);
                return { done: true, value: undefined };
              }
              return { done: false, value: item };
            }

            conn.unregisterListener(reqId);
            return { done: true, value: undefined };
          },
        };
      },
    };
  }
}
