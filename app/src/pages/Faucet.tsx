import React, { useState } from "react";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createInitializeMint2Instruction,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
} from "@solana/spl-token";
import Card from "../components/Card";
import Button from "../components/Button";
import TextInput from "../components/TextInput";
import { parseAnchorError } from "../utils/errors";

export const Faucet: React.FC = () => {
  const { connection } = useConnection();
  const wallet = useWallet();
  const [mintAddress, setMintAddress] = useState("");
  const [mintAmount, setMintAmount] = useState("1000000");
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");

  const handleCreateMint = async () => {
    if (!wallet.publicKey || !wallet.signTransaction) {
      setError("Please connect your wallet first.");
      return;
    }
    setError("");
    setMessage("");
    setLoading(true);

    try {
      const mintKeypair = Keypair.generate();
      const lamports = await connection.getMinimumBalanceForRentExemption(MINT_SIZE);

      const tx = new Transaction().add(
        SystemProgram.createAccount({
          fromPubkey: wallet.publicKey,
          newAccountPubkey: mintKeypair.publicKey,
          space: MINT_SIZE,
          lamports,
          programId: TOKEN_PROGRAM_ID,
        }),
        createInitializeMint2Instruction(
          mintKeypair.publicKey,
          6,
          wallet.publicKey,
          wallet.publicKey,
          TOKEN_PROGRAM_ID
        )
      );

      const { blockhash } = await connection.getLatestBlockhash("processed");
      tx.recentBlockhash = blockhash;
      tx.feePayer = wallet.publicKey;

      tx.partialSign(mintKeypair);
      const signed = await wallet.signTransaction(tx);
      const sig = await connection.sendRawTransaction(signed.serialize());
      await connection.confirmTransaction(sig, "processed");

      setMintAddress(mintKeypair.publicKey.toBase58());
      setMessage(`Test mint created successfully! Mint: ${mintKeypair.publicKey.toBase58()} (Tx: ${sig})`);
    } catch (err: any) {
      console.error(err);
      setError(parseAnchorError(err));
    } finally {
      setLoading(false);
    }
  };

  const handleMintTokens = async () => {
    if (!wallet.publicKey || !wallet.signTransaction) {
      setError("Please connect your wallet first.");
      return;
    }
    if (!mintAddress.trim()) {
      setError("Please enter or create a mint address first.");
      return;
    }
    setError("");
    setMessage("");
    setLoading(true);

    try {
      const mintPubkey = new PublicKey(mintAddress.trim());
      const ata = await getAssociatedTokenAddress(mintPubkey, wallet.publicKey, true, TOKEN_PROGRAM_ID);

      const tx = new Transaction();
      const accountInfo = await connection.getAccountInfo(ata);
      if (!accountInfo) {
        tx.add(
          createAssociatedTokenAccountInstruction(
            wallet.publicKey,
            ata,
            wallet.publicKey,
            mintPubkey,
            TOKEN_PROGRAM_ID
          )
        );
      }

      const amountVal = parseFloat(mintAmount) || 1000000;
      const baseUnits = BigInt(Math.floor(amountVal * 1_000_000)); // 6 decimals

      tx.add(
        createMintToInstruction(
          mintPubkey,
          ata,
          wallet.publicKey,
          baseUnits,
          [],
          TOKEN_PROGRAM_ID
        )
      );

      const { blockhash } = await connection.getLatestBlockhash("processed");
      tx.recentBlockhash = blockhash;
      tx.feePayer = wallet.publicKey;

      const signed = await wallet.signTransaction(tx);
      const sig = await connection.sendRawTransaction(signed.serialize());
      await connection.confirmTransaction(sig, "processed");

      setMessage(`Minted ${amountVal} tokens to your ATA! (Tx: ${sig})`);
    } catch (err: any) {
      console.error(err);
      setError(parseAnchorError(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={{ maxWidth: "600px", margin: "2rem auto", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <Card title="Devnet Faucet">
        <p style={{ marginBottom: "1rem", fontSize: "0.875rem" }}>
          Use this faucet to create test SPL token mints and mint tokens to your wallet on Solana <strong>Devnet</strong>.
        </p>

        {error && (
          <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", marginBottom: "1rem", fontWeight: 700 }}>
            {error}
          </div>
        )}

        {message && (
          <div style={{ backgroundColor: "var(--green)", padding: "0.75rem", border: "2px solid var(--ink)", marginBottom: "1rem", fontWeight: 700, wordBreak: "break-all" }}>
            {message}
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
          <div>
            <h3 style={{ fontSize: "1rem", fontWeight: 700, marginBottom: "0.5rem" }}>1. Create Test Mint (6 Decimals)</h3>
            <Button variant="primary" onClick={handleCreateMint} disabled={loading || !wallet.connected}>
              {loading ? "Processing..." : "Create test mint"}
            </Button>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: "1rem", borderTop: "2px solid var(--ink)", paddingTop: "1rem" }}>
            <h3 style={{ fontSize: "1rem", fontWeight: 700 }}>2. Mint Tokens to Me</h3>
            <TextInput
              label="Mint Address"
              placeholder="Paste mint pubkey here..."
              value={mintAddress}
              onChange={(e) => setMintAddress(e.target.value)}
            />
            <TextInput
              label="Amount (whole tokens)"
              type="number"
              value={mintAmount}
              onChange={(e) => setMintAmount(e.target.value)}
            />
            <Button variant="ok" onClick={handleMintTokens} disabled={loading || !wallet.connected}>
              {loading ? "Processing..." : "Mint tokens to me"}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
};
