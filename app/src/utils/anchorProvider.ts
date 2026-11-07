import { Connection, PublicKey } from "@solana/web3.js";
import { Program, AnchorProvider, Idl } from "@coral-xyz/anchor";
import { AnchorWallet } from "@solana/wallet-adapter-react";
import idl from "../idl/bening.json";

const DEVNET_RPC = "https://api.devnet.solana.com";

export function getConnection(): Connection {
  return new Connection(DEVNET_RPC, "confirmed");
}

export function getProgram(wallet: AnchorWallet): Program {
  const connection = getConnection();
  const provider = new AnchorProvider(connection, wallet, { commitment: "confirmed" });
  return new Program(idl as unknown as Idl, provider);
}

export function getReadOnlyProgram(): Program {
  const connection = getConnection();
  const dummyWallet = {
    publicKey: PublicKey.default,
    signTransaction: async (tx: any) => tx,
    signAllTransactions: async (txs: any) => txs,
  };
  const provider = new AnchorProvider(connection, dummyWallet as any, { commitment: "confirmed" });
  return new Program(idl as unknown as Idl, provider);
}
