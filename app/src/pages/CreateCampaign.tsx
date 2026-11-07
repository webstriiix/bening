import React, { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useConnection, useAnchorWallet } from "@solana/wallet-adapter-react";
import { PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync, getMint } from "@solana/spl-token";
import BN from "bn.js";
import { getProgram } from "../utils/anchorProvider";
import { parseAnchorError } from "../utils/errors";
import Card from "../components/Card";
import Button from "../components/Button";
import TextInput from "../components/TextInput";

interface MilestoneRow {
  label: string;
  amount: string;
  vendor: string;
}

export const CreateCampaign: React.FC = () => {
  const { connection } = useConnection();
  const wallet = useAnchorWallet();
  const navigate = useNavigate();

  const [title, setTitle] = useState("");
  const [mint, setMint] = useState("");
  const [verifier, setVerifier] = useState("");
  const [metadataUri, setMetadataUri] = useState("");
  const [milestones, setMilestones] = useState<MilestoneRow[]>([
    { label: "Milestone 1", amount: "100", vendor: "" },
  ]);

  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const addMilestone = () => {
    if (milestones.length >= 5) {
      setError("Maximum 5 milestones allowed.");
      return;
    }
    setMilestones([...milestones, { label: `Milestone ${milestones.length + 1}`, amount: "100", vendor: "" }]);
  };

  const removeMilestone = (index: number) => {
    if (milestones.length <= 1) {
      setError("At least 1 milestone is required.");
      return;
    }
    setMilestones(milestones.filter((_, i) => i !== index));
  };

  const updateMilestone = (index: number, field: keyof MilestoneRow, val: string) => {
    const updated = [...milestones];
    updated[index][field] = val;
    setMilestones(updated);
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (!wallet || !wallet.publicKey) {
      setError("Please connect your wallet first.");
      return;
    }

    const organizer = wallet.publicKey;

    // 1. Title validation (1-48 bytes)
    const titleBytes = new TextEncoder().encode(title.trim());
    if (titleBytes.length === 0 || titleBytes.length > 48) {
      setError("Campaign title must be between 1 and 48 bytes.");
      return;
    }

    // 2. Mint validation
    let mintPubkey: PublicKey;
    try {
      mintPubkey = new PublicKey(mint.trim());
    } catch (e) {
      setError("Invalid mint address.");
      return;
    }

    // 3. Verifier validation
    let verifierPubkey: PublicKey;
    try {
      verifierPubkey = new PublicKey(verifier.trim());
    } catch (e) {
      setError("Invalid verifier address.");
      return;
    }

    if (verifierPubkey.equals(organizer)) {
      setError("Verifier cannot be the organizer.");
      return;
    }
    if (verifierPubkey.equals(PublicKey.default)) {
      setError("Verifier cannot be the default pubkey.");
      return;
    }

    // 4. Milestones validation (1-5)
    if (milestones.length === 0 || milestones.length > 5) {
      setError("Campaign must have between 1 and 5 milestones.");
      return;
    }

    setLoading(true);

    try {
      const mintInfo = await getMint(connection, mintPubkey);
      const decimals = mintInfo.decimals;

      const parsedMilestones = [];
      for (let i = 0; i < milestones.length; i++) {
        const m = milestones[i];
        const labelBytes = new TextEncoder().encode(m.label.trim());
        if (labelBytes.length === 0 || labelBytes.length > 32) {
          throw new Error(`Milestone #${i + 1} label must be between 1 and 32 bytes.`);
        }

        const amtNum = parseFloat(m.amount);
        if (isNaN(amtNum) || amtNum <= 0) {
          throw new Error(`Milestone #${i + 1} amount must be greater than 0.`);
        }

        let vendorPubkey: PublicKey;
        try {
          vendorPubkey = new PublicKey(m.vendor.trim());
        } catch (e) {
          throw new Error(`Milestone #${i + 1} has an invalid vendor address.`);
        }

        if (vendorPubkey.equals(organizer)) {
          throw new Error(`Milestone #${i + 1} vendor cannot be the organizer.`);
        }
        if (vendorPubkey.equals(verifierPubkey)) {
          throw new Error(`Milestone #${i + 1} vendor cannot be the verifier.`);
        }
        if (vendorPubkey.equals(PublicKey.default)) {
          throw new Error(`Milestone #${i + 1} vendor cannot be the default pubkey.`);
        }

        const baseUnits = new BN(Math.floor(amtNum * Math.pow(10, decimals)).toString());

        parsedMilestones.push({
          label: m.label.trim(),
          amount: baseUnits,
          vendor: vendorPubkey,
        });
      }

      const randomBuf = new Uint8Array(8);
      crypto.getRandomValues(randomBuf);
      const campaignIdBn = new BN(new DataView(randomBuf.buffer).getBigUint64(0, true).toString());

      const campaignIdBuffer = Buffer.alloc(8);
      campaignIdBuffer.writeBigUInt64LE(BigInt(campaignIdBn.toString()));

      const [campaignPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("campaign"), organizer.toBuffer(), campaignIdBuffer],
        new PublicKey("3uemNohfUixociV872GGK1e4Dh2PRbPiNYT2SMgKXh5y")
      );

      const vaultPda = getAssociatedTokenAddressSync(
        mintPubkey,
        campaignPda,
        true,
        TOKEN_PROGRAM_ID
      );

      const program = getProgram(wallet);

      /*
        IDL Accounts order for create_campaign:
        1. organizer (signer, writable)
        2. campaign (writable, PDA)
        3. vault (writable, ATA)
        4. mint
        5. system_program
        6. token_program
        7. associated_token_program
        8. rent
      */
      const txSig = await (program.methods as any)
        .createCampaign(
          campaignIdBn,
          title.trim(),
          metadataUri.trim(),
          verifierPubkey,
          parsedMilestones
        )
        .accounts({
          organizer: organizer,
          campaign: campaignPda,
          vault: vaultPda,
          mint: mintPubkey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      setLoading(false);
      // Auto-redirect on success to /c/<campaign_pda>, passing sig via router state
      navigate(`/c/${campaignPda.toBase58()}`, { state: { txSig } });
    } catch (err: any) {
      console.error(err);
      setError(parseAnchorError(err));
      setLoading(false);
    }
  };

  return (
    <div style={{ maxWidth: "700px", margin: "2rem auto", display: "flex", flexDirection: "column", gap: "1.5rem" }}>
      <Card title="Create Campaign">
        <form onSubmit={handleCreate} style={{ display: "flex", flexDirection: "column", gap: "1.25rem" }}>
          {error && (
            <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
              {error}
            </div>
          )}

          {!wallet || !wallet.publicKey ? (
            <div style={{ backgroundColor: "var(--yellow)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
              Please connect your wallet first to create a campaign.
            </div>
          ) : null}

          <TextInput
            label="Campaign Title (1–48 bytes)"
            placeholder="e.g. Community Rice Distribution"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
          />

          <TextInput
            label="Mint Address (Base58)"
            placeholder="SPL Token Mint pubkey..."
            value={mint}
            onChange={(e) => setMint(e.target.value)}
            required
          />

          <TextInput
            label="Verifier Address (Base58)"
            placeholder="Verifier pubkey..."
            value={verifier}
            onChange={(e) => setVerifier(e.target.value)}
            required
          />

          <TextInput
            label="Metadata URI (Optional)"
            placeholder="https://example.com/metadata.json"
            value={metadataUri}
            onChange={(e) => setMetadataUri(e.target.value)}
          />

          <div style={{ borderTop: "2px solid var(--ink)", paddingTop: "1rem" }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.75rem" }}>
              <h3 style={{ fontSize: "1rem", fontWeight: 700 }}>Milestones (1–5)</h3>
              <Button type="button" variant="primary" onClick={addMilestone}>
                + Add Milestone
              </Button>
            </div>

            {milestones.map((m, idx) => (
              <div key={idx} style={{ border: "2px solid var(--ink)", padding: "1rem", marginBottom: "1rem", backgroundColor: "var(--bg)" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.5rem" }}>
                  <strong>Milestone #{idx + 1}</strong>
                  {milestones.length > 1 && (
                    <button
                      type="button"
                      onClick={() => removeMilestone(idx)}
                      style={{ background: "var(--pink)", border: "2px solid var(--ink)", padding: "0.25rem 0.5rem", cursor: "pointer", fontWeight: 700 }}
                    >
                      Remove
                    </button>
                  )}
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                  <TextInput
                    label="Label (max 32 bytes)"
                    placeholder="e.g. Milestone 1"
                    value={m.label}
                    onChange={(e) => updateMilestone(idx, "label", e.target.value)}
                    required
                  />
                  <TextInput
                    label="Amount (whole tokens)"
                    type="number"
                    placeholder="100"
                    value={m.amount}
                    onChange={(e) => updateMilestone(idx, "amount", e.target.value)}
                    required
                  />
                  <TextInput
                    label="Vendor Address (Base58)"
                    placeholder="Vendor pubkey..."
                    value={m.vendor}
                    onChange={(e) => updateMilestone(idx, "vendor", e.target.value)}
                    required
                  />
                </div>
              </div>
            ))}
          </div>

          <Button type="submit" variant="primary" disabled={loading || !wallet || !wallet.publicKey}>
            {loading ? "Creating Campaign..." : (!wallet || !wallet.publicKey ? "Connect Wallet First" : "Create Campaign")}
          </Button>
        </form>
      </Card>
    </div>
  );
};
