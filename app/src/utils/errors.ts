import idl from "../idl/bening.json";

export function parseAnchorError(err: any): string {
  if (!err) return "An unknown error occurred.";
  const msg = err.message || String(err);
  
  const match = msg.match(/(?:custom program error: 0x([0-9a-fA-F]+))|(?:Error Number: (\d+))|(?:code: (\d+))/i);
  let code: number | null = null;
  if (match) {
    if (match[1]) code = parseInt(match[1], 16);
    else if (match[2]) code = parseInt(match[2], 10);
    else if (match[3]) code = parseInt(match[3], 10);
  }

  if (code !== null) {
    const errorEntry = idl.errors?.find((e: any) => e.code === code);
    if (errorEntry) {
      return errorEntry.msg;
    }
  }

  return msg;
}
