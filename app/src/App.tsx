import React from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { ConnectionProvider, WalletProvider } from "@solana/wallet-adapter-react";
import { PhantomWalletAdapter } from "@solana/wallet-adapter-phantom";
import { SolflareWalletAdapter } from "@solana/wallet-adapter-solflare";
import { Home } from "./pages/Home";
import { CampaignDetails } from "./pages/CampaignDetails";
import { Navbar } from "./components/Navbar";

const endpoint = "https://api.devnet.solana.com";
const wallets = [new PhantomWalletAdapter(), new SolflareWalletAdapter()];

const App: React.FC = () => {
  return (
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} autoConnect>
        <BrowserRouter>
          <div style={{ minHeight: "100vh", backgroundColor: "var(--bg)", color: "var(--ink)" }}>
            <Navbar />
            <main className="container">
              <Routes>
                <Route path="/" element={<Home />} />
                <Route path="/c/:campaignPda" element={<CampaignDetails />} />
              </Routes>
            </main>
          </div>
        </BrowserRouter>
      </WalletProvider>
    </ConnectionProvider>
  );
};

export default App;
