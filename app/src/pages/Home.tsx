import React, { useState } from "react";
import { useNavigate } from "react-router-dom";
import Card from "../components/Card";
import TextInput from "../components/TextInput";
import Button from "../components/Button";
import { PublicKey } from "@solana/web3.js";

export const Home: React.FC = () => {
  const [pdaInput, setPdaInput] = useState("");
  const [error, setError] = useState("");
  const navigate = useNavigate();

  const handleGo = (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    const trimmed = pdaInput.trim();
    if (!trimmed) {
      setError("Please enter a Campaign PDA address.");
      return;
    }
    try {
      new PublicKey(trimmed);
      navigate(`/c/${encodeURIComponent(trimmed)}`);
    } catch (err) {
      setError("Invalid Solana public key / Campaign PDA address.");
    }
  };

  return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "60vh" }}>
      <div style={{ width: "100%", maxWidth: "500px" }}>
        <Card title="Bening Campaign Explorer">
          <form onSubmit={handleGo} style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
            <TextInput
              label="Campaign PDA"
              placeholder="Enter campaign PDA address..."
              value={pdaInput}
              onChange={(e) => setPdaInput(e.target.value)}
            />
            {error && (
              <div style={{ color: "var(--pink)", fontWeight: 700, fontSize: "0.875rem" }}>
                {error}
              </div>
            )}
            <Button type="submit" variant="primary">
              Go
            </Button>
          </form>
        </Card>
      </div>
    </div>
  );
};
