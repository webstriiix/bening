import React from "react";
import { useWallet } from "@solana/wallet-adapter-react";
import Button from "./Button";

export const WalletButton: React.FC = () => {
  const { publicKey, connect, disconnect, connected, select, wallets } = useWallet();

  const handleClick = async () => {
    if (connected) {
      await disconnect();
    } else {
      const phantom = wallets.find(w => w.adapter.name === "Phantom") || wallets[0];
      if (phantom) {
        select(phantom.adapter.name);
        try {
          await connect();
        } catch (e) {
          console.error(e);
        }
      }
    }
  };

  const text = connected && publicKey
    ? `${publicKey.toBase58().slice(0, 4)}...${publicKey.toBase58().slice(-4)}`
    : "Connect Wallet";

  return (
    <Button variant="primary" onClick={handleClick}>
      {text}
    </Button>
  );
};
