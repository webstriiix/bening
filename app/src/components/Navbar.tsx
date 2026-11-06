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
      <div className="container" style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <Link to="/" style={{ textDecoration: "none", color: "var(--ink)" }}>
          <h1 style={{ fontFamily: "Space Grotesk", fontWeight: 750, fontSize: "1.5rem", margin: 0 }}>
            bening.
          </h1>
        </Link>
        <WalletButton />
      </div>
    </header>
  );
};
