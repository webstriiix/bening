import React, { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { useConnection } from "@solana/wallet-adapter-react";
import { PublicKey } from "@solana/web3.js";
import { getMint } from "@solana/spl-token";
import BN from "bn.js";
import { getReadOnlyProgram } from "../utils/anchorProvider";
import Card from "../components/Card";
import TextInput from "../components/TextInput";
import Button from "../components/Button";
import ProgressBar from "../components/ProgressBar";
import Badge from "../components/Badge";

/*
 * NOTE: The UI is unverified; only the human tests in the browser.
 */

const MAX_CAMPAIGNS = 12;

export const Home: React.FC = () => {
  const { connection } = useConnection();
  const navigate = useNavigate();

  // State for PDA search
  const [pdaInput, setPdaInput] = useState("");
  const [pdaError, setPdaError] = useState("");

  // State for campaign grid
  const [campaigns, setCampaigns] = useState<any[]>([]);
  const [gridLoading, setGridLoading] = useState(true);
  const [gridError, setGridError] = useState("");
  const [refreshLoading, setRefreshLoading] = useState(false);

  // Mint decimals cache: mint -> decimals number
  const mintDecimalsCache = new Map<string, number>();

  const fetchCampaigns = async () => {
    setGridLoading(true);
    setGridError("");
    try {
      const program = getReadOnlyProgram();
      const all = await (program.account as any).campaign.all();

      // Sort by total_raised desc, capped at 12 max
      const sorted = all
        .sort((a: any, b: any) => {
          const ra = (b.account.totalRaised as BN).toNumber();
          const rb = (a.account.totalRaised as BN).toNumber();
          return ra - rb;
        })
        .slice(0, MAX_CAMPAIGNS);

      const result: any[] = [];
      for (const d of sorted) {
        const acct = d.account as any;
        const mint = acct.mint as PublicKey;
        const mintStr = mint.toBase58();
        if (!mintDecimalsCache.has(mintStr)) {
          try {
            const mintInfo = await getMint(connection, mint);
            mintDecimalsCache.set(mintStr, mintInfo.decimals);
          } catch {
            mintDecimalsCache.set(mintStr, 6);
          }
        }
        const decs = mintDecimalsCache.get(mintStr) || 6;
        result.push({
          address: d.publicKey.toBase58(),
          title: acct.title as string,
          totalRaised: new BN((acct.totalRaised as any).toString()),
          targetAmount: new BN((acct.targetAmount as any).toString()),
          milestoneCount: (acct.milestones as any[]).length,
          metadata_uri: (acct.metadataUri as string) || (acct.metadata_uri as string) || "",
          decimals: decs,
        });
      }
      setCampaigns(result);
    } catch (err: any) {
      console.error("Failed to fetch campaigns:", err);
      setGridError(
        `Could not load campaigns. Public RPC may be rate-limited. ${
          err instanceof Error ? err.message : String(err)
        }`
      );
    } finally {
      setGridLoading(false);
    }
  };

  useEffect(() => {
    void fetchCampaigns();
  }, [connection]);

  const handlePdaGo = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = pdaInput.trim();
    if (!trimmed) {
      setPdaError("Please enter a Campaign PDA address.");
      return;
    }
    try {
      new PublicKey(trimmed);
      navigate(`/c/${encodeURIComponent(trimmed)}`);
    } catch {
      setPdaError("Invalid Solana public key / Campaign PDA address.");
    }
  };

  const handleRefresh = async () => {
    setRefreshLoading(true);
    await fetchCampaigns();
    setRefreshLoading(false);
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "2rem", paddingBottom: "2rem" }}>
      {/* PDA Search Section */}
      <Card title="Bening Campaign Explorer">
        <form onSubmit={handlePdaGo} style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          <TextInput
            label="Campaign PDA"
            placeholder="Enter campaign PDA address..."
            value={pdaInput}
            onChange={(e) => setPdaInput(e.target.value)}
          />
          {pdaError && (
            <div style={{ fontWeight: 700, color: "var(--pink)" }}>{pdaError}</div>
          )}
          <Button type="submit" variant="primary">
            Go
          </Button>
        </form>
      </Card>

      {/* Task 5 — Onboarding Section ("How it works") */}
      <Card title="How it works">
        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem", fontSize: "0.95rem" }}>
          <div>
            <strong>1. Create:</strong> The organizer sets up a campaign with milestones, target amounts, and a designated verifier.
          </div>
          <div>
            <strong>2. Donate:</strong> Donors contribute SPL tokens to fund campaign milestones.
          </div>
          <div>
            <strong>3. Proof & Approve:</strong> The organizer submits proof for completed milestones, and the verifier reviews and approves them.
          </div>
          <div>
            <strong>4. Release:</strong> Anyone can trigger the release of funds from the vault to the vendor once a milestone is approved.
          </div>
        </div>
      </Card>

      {/* Campaigns Section */}
      <div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem" }}>
          <h2 style={{ margin: 0, fontSize: "1.25rem" }}>Campaigns</h2>
          <Button variant="primary" onClick={handleRefresh} disabled={refreshLoading}>
            {refreshLoading ? "Refreshing…" : "Refresh"}
          </Button>
        </div>

        {/* State: Loading */}
        {gridLoading && (
          <div style={{ textAlign: "center", padding: "2rem", fontWeight: 700 }}>
            Loading campaigns…
          </div>
        )}

        {/* State: Empty */}
        {!gridLoading && gridError === "" && campaigns.length === 0 && (
          <div style={{ textAlign: "center", padding: "2rem" }}>
            <p style={{ fontWeight: 700, marginBottom: "1rem" }}>No campaigns yet</p>
            <Button
              variant="primary"
              onClick={() => navigate("/create")}
            >
              Create Campaign
            </Button>
          </div>
        )}

        {/* State: Error */}
        {!gridLoading && gridError !== "" && (
          <div style={{ backgroundColor: "var(--pink)", color: "var(--ink)", padding: "1rem", border: "2px solid var(--ink)", marginBottom: "1rem" }}>
            <p style={{ fontWeight: 700, marginBottom: "0.75rem" }}>{gridError}</p>
            <Button variant="primary" onClick={handleRefresh} disabled={refreshLoading}>
              {refreshLoading ? "Retrying…" : "Retry"}
            </Button>
          </div>
        )}

        {/* Campaign Grid (Responsive: 1 col on mobile, 2-3 on desktop) */}
        {!gridLoading && campaigns.length > 0 && (
          <div
            style={{
              display: "grid",
              gap: "1.5rem",
              gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))",
            }}
          >
            {campaigns.map((c, idx) => {
              const decs = c.decimals || 6;
              const raisedF = formatBaseUnits(c.totalRaised, decs);
              const targetF = formatBaseUnits(c.targetAmount, decs);

              const meta = c.metadata_uri || "";
              const isImageUrl = meta.startsWith("https://") && new TextEncoder().encode(meta).length <= 128;
              const firstLetter = (c.title || "C")[0].toUpperCase();

              return (
                <div key={idx} className="card" style={{ border: "3px solid var(--ink)", backgroundColor: "var(--white)", boxShadow: "6px 6px 0 var(--ink)", display: "flex", flexDirection: "column" }}>
                  <a
                    href={`/c/${c.address}`}
                    style={{ textDecoration: "none", color: "inherit", display: "flex", flexDirection: "column", height: "100%" }}
                  >
                    {isImageUrl ? (
                      <img
                        src={meta}
                        loading="lazy"
                        referrerPolicy="no-referrer"
                        alt={c.title || ""}
                        style={{ width: "100%", height: "160px", objectFit: "cover", borderBottom: "3px solid var(--ink)" }}
                        onError={(e) => {
                          (e.target as HTMLElement).style.display = "none";
                        }}
                      />
                    ) : (
                      <div
                        style={{
                          width: "100%",
                          height: "160px",
                          backgroundColor: "var(--ink)",
                          color: "var(--bg)",
                          display: "flex",
                          alignItems: "center",
                          justifyContent: "center",
                          fontWeight: 800,
                          fontSize: "3rem",
                          borderBottom: "3px solid var(--ink)",
                        }}
                      >
                        {firstLetter}
                      </div>
                    )}
                    <div style={{ padding: "1rem", display: "flex", flexDirection: "column", gap: "0.75rem", flex: 1, justifyContent: "space-between" }}>
                      <div>
                        <h3 style={{ margin: "0 0 0.5rem 0", fontSize: "1.1rem", fontWeight: 800, wordBreak: "break-word" }}>
                          {c.title || "Untitled"}
                        </h3>
                        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center", marginBottom: "0.5rem" }}>
                          <Badge status="pending">Active</Badge>
                          <span style={{ fontSize: "0.75rem", fontWeight: 700 }}>{c.milestoneCount} milestones</span>
                        </div>
                      </div>
                      <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
                        <ProgressBar
                          target={Number(c.targetAmount.toString()) || 1}
                          current={Number(c.totalRaised.toString()) || 0}
                        />
                        <div style={{ fontSize: "0.8125rem", fontWeight: 700, opacity: 0.9 }}>
                          {raisedF} / {targetF} tokens
                        </div>
                      </div>
                    </div>
                  </a>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
};

function formatBaseUnits(value: BN, decimals: number): string {
  const num = Number(value.toString());
  if (!Number.isFinite(num)) return "0";
  const str = num.toString().padStart(decimals + 1, "0");
  const whole = str.slice(0, str.length - decimals);
  const fraction = str.slice(str.length - decimals).replace(/0+$/, "");
  return fraction ? `${whole}.${fraction}` : whole;
}