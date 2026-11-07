import { Buffer } from "buffer/";

declare global {
  interface Window {
    Buffer: typeof Buffer;
  }
}

if (!globalThis.Buffer) {
  globalThis.Buffer = Buffer as any;
}
