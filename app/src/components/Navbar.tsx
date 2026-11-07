import React from "react";
import { Link } from "react-router-dom";
import { WalletButton } from "./WalletButton";

export const Navbar: React.FC = () => {
  return (
    <header style={{
      borderBottom: "3px solid var(--ink)",
      backgroundColor: "var(--white)",
      padding: "1rem",
      marginBottom: "2rem"
    }}>
      <div className="container" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "1rem" }}>
        <div style={{ display: "flex", alignItems: "center", gap: "1.5rem" }}>
          <Link to="/" style={{ textDecoration: "none", color: "var(--ink)" }}>
            <h1 style={{ fontFamily: "Space Grotesk", fontWeight: 750, fontSize: "1.5rem", margin: 0 }}>
              bening.
            </h1>
          </Link>
          <nav style={{ display: "flex", gap: "1rem", fontWeight: 600, fontSize: "0.875rem" }}>
            <Link to="/" style={{ color: "var(--ink)", textDecoration: "none" }}>Explore</Link>
            <Link to="/create" style={{ color: "var(--ink)", textDecoration: "none" }}>Create Campaign</Link>
            <Link to="/faucet" style={{ color: "var(--ink)", textDecoration: "none" }}>Faucet</Link>
          </nav>
        </div>
        <WalletButton />
      </div>
    </header>
  );
};
