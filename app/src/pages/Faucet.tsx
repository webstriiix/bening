import React, { useState } from "react";
import { useConnection, useWallet } from "@solana/wallet-adapter-react";
import { sendAndConfirmPolling } from "../utils/sendAndConfirmPolling";

/*
 * NOTE: The UI is unverified. Only human tests in the browser can confirm
 * the correctness of transactions, signatures, and on-chain state.
 * This is a frontend-only implementation and should be validated by users.
 */
import {
  TOKEN_PROGRAM_ID,
  MINT_SIZE,
  createInitializeMint2Instruction,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createMintToInstruction,
  getAccount,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
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
  const [tokenBalance, setTokenBalance] = useState<string | null>(null);
  const [createdMint, setCreatedMint] = useState<string | null>(null);

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text);
    setMessage("Copied to clipboard!");
    setTimeout(() => setMessage(""), 2000);
  };

  const fetchTokenBalance = async (mintPubkey: PublicKey) => {
    if (!wallet.publicKey) return;
    try {
      const ata = await getAssociatedTokenAddress(mintPubkey, wallet.publicKey, true, TOKEN_PROGRAM_ID);
      const accountInfo = await connection.getAccountInfo(ata);
      if (accountInfo) {
        const account = await getAccount(connection, ata);
        setTokenBalance(account.amount.toString());
      } else {
        setTokenBalance("0");
      }
    } catch (err) {
      console.error("Failed to fetch token balance:", err);
      setTokenBalance("0");
    }
  };

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

      // Signing order matters: the new mint account must sign BEFORE the wallet signs
      // and BEFORE serialize(). Otherwise serialize() throws 'Missing signature'.
      // Note: the method is tx.partialSign(keypair), not keypair.partialSign(tx).
      tx.partialSign(mintKeypair);
      const signed = await wallet.signTransaction(tx);
      const sig = await connection.sendRawTransaction(signed.serialize());

      // Send transaction via wallet signature; poll for confirmation only (no WebSocket confirmTransaction)
      await sendAndConfirmPolling(connection, sig);

      const mintStr = mintKeypair.publicKey.toBase58();
      setCreatedMint(mintStr);
      setMintAddress(mintStr); // Prefill the mint tokens input
      setMessage(`Test mint created successfully! Mint: ${mintStr} (Tx: ${sig})`);
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

      // Send transaction via wallet signature; poll for confirmation only (no WebSocket confirmTransaction)
      await sendAndConfirmPolling(connection, sig);

      setMessage(`Minted ${amountVal} tokens to your ATA! (Tx: ${sig})`);
      
      // Fetch and show token balance
      await fetchTokenBalance(mintPubkey);
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
            
            {createdMint && (
              <div style={{ marginTop: "1rem", display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
                <span style={{ fontWeight: 700 }}>Created Mint:</span>
                <TextInput
                  value={createdMint}
                  readOnly
                  style={{ flex: 1, minWidth: "200px" }}
                />
                <button className="btn btn-ok" onClick={() => copyToClipboard(createdMint)} style={{ padding: "0.25rem 0.5rem" }}>
                  Copy
                </button>
              </div>
            )}
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

            {tokenBalance !== null && (
              <div style={{ marginTop: "0.75rem", padding: "0.75rem", backgroundColor: "var(--bg)", border: "2px solid var(--ink)" }}>
                <strong>Your Token Balance: </strong>
                <span style={{ fontFamily: "monospace" }}>{tokenBalance} base units</span>
                {mintAddress && (
                  <span style={{ marginLeft: "1rem" }}>({Number(tokenBalance) / 1_000_000} whole tokens)</span>
                )}
              </div>
            )}
          </div>
        </div>
      </Card>
    </div>
  );
};