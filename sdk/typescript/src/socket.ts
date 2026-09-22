import * as net from "net";
import * as os from "os";
import * as path from "path";
import { RequestPayload, ResponsePacket } from "./types";

export function getDefaultSocketPath(): string {
  if (process.env.XDG_RUNTIME_DIR) {
    return path.join(process.env.XDG_RUNTIME_DIR, "aicore.sock");
  }
  return path.join(os.homedir(), ".ai-core", "aicore.sock");
}

export class SocketConnection {
  private socket: net.Socket | null = null;
  private buffer = "";
  private listeners: Map<string, (packet: ResponsePacket) => void> = new Map();

  async connect(socketPath?: string): Promise<void> {
    const targetPath = socketPath || getDefaultSocketPath();
    return new Promise((resolve, reject) => {
      this.socket = net.createConnection(targetPath, () => {
        resolve();
      });

      this.socket.on("data", (chunk: Buffer) => {
        this.buffer += chunk.toString("utf-8");
        this.processBuffer();
      });

      this.socket.on("error", (err: Error) => {
        reject(err);
      });
    });
  }

  registerListener(reqId: string, callback: (packet: ResponsePacket) => void): void {
    this.listeners.set(reqId, callback);
  }

  unregisterListener(reqId: string): void {
    this.listeners.delete(reqId);
  }

  send(payload: RequestPayload): void {
    if (!this.socket) {
      throw new Error("Socket not connected");
    }
    const json = JSON.stringify(payload) + "\n";
    this.socket.write(json);
  }

  close(): void {
    if (this.socket) {
      this.socket.end();
      this.socket = null;
    }
  }

  private processBuffer(): void {
    const lines = this.buffer.split("\n");
    this.buffer = lines.pop() || "";

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      try {
        const packet: ResponsePacket = JSON.parse(trimmed);
        const listener = this.listeners.get(packet.id);
        if (listener) {
          listener(packet);
        }
      } catch (err) {
        console.error("Failed to parse response line:", trimmed, err);
      }
    }
  }
}
