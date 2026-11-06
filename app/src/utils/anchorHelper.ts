import { Connection, PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider, Idl } from "@coral-xyz/anchor";
import idl from "../idl/bening.json";

export function getProgram(connection: Connection): Program {
  const dummyWallet = {
    publicKey: PublicKey.default,
    signTransaction: async (tx: any) => tx,
    signAllTransactions: async (txs: any) => txs,
  };
  const provider = new AnchorProvider(connection, dummyWallet as any, { commitment: "processed" });
  return new Program(idl as unknown as Idl, provider);
}

export function parseEnumStatus(statusObj: any): string {
  if (typeof statusObj === "string") return statusObj.toLowerCase();
  if (!statusObj || typeof statusObj !== "object") return "unknown";
  const keys = Object.keys(statusObj);
  if (keys.length === 0) return "unknown";
  const key = keys[0];
  return key.replace(/([A-Z])/g, "_$1").toLowerCase();
}
