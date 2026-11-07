import React, { useEffect, useState } from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import Button from "./Button";

const WALLETTED_OUT_HINT =
  "Unable to select wallet. Please try opening your wallet extension manually.";

export const WalletButton: React.FC = () => {
  const { publicKey, wallet, connecting, connect, disconnect, connected, select, wallets } =
    useWallet();
  const [hint, setHint] = useState("");
  // Set when the user clicked Connect but no adapter was selected yet;
  // the effect below performs the actual connect() once the context has
  // updated after select().
  const [pendingConnect, setPendingConnect] = useState(false);

  const handleClick = async () => {
    setHint("");
    if (connected) {
      setPendingConnect(false);
      await disconnect();
      return;
    }

    const phantom = wallets.find((w) => w.adapter.name === "Phantom") ?? wallets[0];
    if (!phantom) {
      setHint("No Solana wallet was found. Install Phantom or Solflare and reload the page.");
      return;
    }

    if (!wallet) {
      // Nothing is selected in the wallet context yet. select() updates the
      // context asynchronously, so do NOT call connect() here — the effect
      // below will connect reactively once `wallet` becomes non-null.
      try {
        select(phantom.adapter.name);
      } catch {
        console.error("select() failed");
        setHint(WALLETTED_OUT_HINT);
        return;
      }
      setPendingConnect(true);
      return;
    }

    // An adapter is already selected in the context: connect immediately.
    try {
      await connect();
    } catch (e: any) {
      if (e?.name === "WalletNotSelectedError") {
        setHint(WALLETTED_OUT_HINT);
      } else {
        console.error(e);
        setHint("Could not connect the wallet. Unlock it, approve the request, and try again.");
      }
    }
  };

  // Reactive connect: fires once select() has propagated a non-null adapter
  // into the context, guaranteeing connect() is never called unselected.
  useEffect(() => {
    if (!pendingConnect || !wallet || connected) return;
    (async () => {
      try {
        await connect();
      } catch (e: any) {
        if (e?.name === "WalletNotSelectedError") {
          setHint(WALLETTED_OUT_HINT);
        } else {
          console.error(e);
          setHint("Could not connect the wallet. Unlock it, approve the request, and try again.");
        }
      } finally {
        setPendingConnect(false);
      }
    })();
  }, [pendingConnect, wallet, connected, connect]);

  const text =
    connected && publicKey
      ? `${publicKey.toBase58().slice(0, 4)}...${publicKey.toBase58().slice(-4)}`
      : connecting
        ? "Connecting…"
        : "Connect Wallet";

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: "0.25rem" }}>
      <Button variant="primary" onClick={handleClick} disabled={connecting}>
        {text}
      </Button>
      {hint && <div style={{ fontSize: "0.75rem", fontWeight: 700 }}>{hint}</div>}
    </div>
  );
};
