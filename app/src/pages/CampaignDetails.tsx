import React, { useEffect, useState } from "react";
import { useParams, Link } from "react-router-dom";
import { Connection, PublicKey } from "@solana/web3.js";
import { getProgram, parseEnumStatus } from "../utils/anchorHelper";
import { getMint } from "@solana/spl-token";
import { formatAmount, shortenAddress } from "../utils/format";
import Card from "../components/Card";
import Badge from "../components/Badge";
import ProgressBar from "../components/ProgressBar";
import Mono from "../components/Mono";
import Button from "../components/Button";

const DEVNET_RPC = "https://api.devnet.solana.com";

export const CampaignDetails: React.FC = () => {
  const { campaignPda } = useParams<{ campaignPda: string }>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [campaignData, setCampaignData] = useState<any>(null);
  const [mintDecimals, setMintDecimals] = useState<number>(6);

  useEffect(() => {
    async function fetchCampaign() {
      if (!campaignPda) {
        setError("Campaign PDA not provided.");
        setLoading(false);
        return;
      }

      try {
        let pubkey: PublicKey;
        try {
          pubkey = new PublicKey(campaignPda);
        } catch (e) {
          setError("Invalid campaign PDA address.");
          setLoading(false);
          return;
        }

        const connection = new Connection(DEVNET_RPC, "processed");
        const program = getProgram(connection);

        let account: any;
        try {
          account = await (program.account.campaign as any).fetch(pubkey);
        } catch (e) {
          setError("Campaign not found.");
          setLoading(false);
          return;
        }

        setCampaignData(account);

        if (account.mint) {
          try {
            const mintInfo = await getMint(connection, account.mint);
            setMintDecimals(mintInfo.decimals);
          } catch (e) {
            console.error("Failed to fetch mint decimals, using default 6", e);
          }
        }

        setLoading(false);
      } catch (err: any) {
        console.error(err);
        setError(err.message || "Failed to load campaign details.");
        setLoading(false);
      }
    }

    fetchCampaign();
  }, [campaignPda]);

  if (loading) {
    return (
      <div style={{ textAlign: "center", padding: "4rem", fontFamily: "Space Grotesk", fontSize: "1.25rem", fontWeight: 700 }}>
        Loading campaign…
      </div>
    );
  }

  if (error || !campaignData) {
    return (
      <div style={{ maxWidth: "600px", margin: "2rem auto" }}>
        <Card title="Error">
          <p style={{ color: "var(--pink)", fontWeight: 700, marginBottom: "1rem" }}>
            {error || "Campaign not found."}
          </p>
          <Link to="/">
            <Button variant="primary">Back to Home</Button>
          </Link>
        </Card>
      </div>
    );
  }

  const campaignStatusStr = parseEnumStatus(campaignData.status);
  const mintStr = campaignData.mint ? campaignData.mint.toBase58() : "";
  const organizerStr = campaignData.organizer ? campaignData.organizer.toBase58() : "";
  const verifierStr = campaignData.verifier ? campaignData.verifier.toBase58() : "";

  const targetAmountNum = Number(campaignData.targetAmount ?? campaignData.target_amount ?? 0);
  const totalRaisedNum = Number(campaignData.totalRaised ?? campaignData.total_raised ?? 0);
  const totalReleasedNum = Number(campaignData.totalReleased ?? campaignData.total_released ?? 0);

  const formattedRaised = formatAmount(totalRaisedNum, mintDecimals);
  const formattedTarget = formatAmount(targetAmountNum, mintDecimals);
  const formattedReleased = formatAmount(totalReleasedNum, mintDecimals);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem", maxWidth: "800px", margin: "2rem auto", padding: "0 1rem" }}>
      <div>
        <Link to="/">
          <Button variant="primary">← Back</Button>
        </Link>
      </div>

      <Card title={campaignData.title || "Campaign Details"}>
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "0.5rem" }}>
            <div>
              <strong>Status: </strong>
              <Badge status={campaignStatusStr as any}>
                {campaignStatusStr.replace("_", " ").toUpperCase()}
              </Badge>
            </div>
            <div>
              <strong>Total Released: </strong> {totalReleasedNum} ({formattedReleased})
            </div>
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.875rem" }}>
              <span>Raised: {totalRaisedNum} ({formattedRaised})</span>
              <span>Target: {targetAmountNum} ({formattedTarget})</span>
            </div>
            <ProgressBar target={targetAmountNum} current={totalRaisedNum} />
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem", marginTop: "0.5rem" }}>
            <div>
              <span style={{ fontWeight: 700, display: "inline-block", width: "100px" }}>Mint:</span>
              <span title={mintStr}>
                <Mono text={shortenAddress(mintStr)} />
              </span>
            </div>
            <div>
              <span style={{ fontWeight: 700, display: "inline-block", width: "100px" }}>Organizer:</span>
              <span title={organizerStr}>
                <Mono text={shortenAddress(organizerStr)} />
              </span>
            </div>
            <div>
              <span style={{ fontWeight: 700, display: "inline-block", width: "100px" }}>Verifier:</span>
              <span title={verifierStr}>
                <Mono text={shortenAddress(verifierStr)} />
              </span>
            </div>
          </div>
        </div>
      </Card>

      <h2 style={{ fontFamily: "Space Grotesk", fontWeight: 750, fontSize: "1.5rem", marginTop: "1rem" }}>
        Milestones
      </h2>

      {campaignData.milestones && campaignData.milestones.length > 0 ? (
        campaignData.milestones.map((m: any, idx: number) => {
          const mStatus = parseEnumStatus(m.status);
          const mAmountNum = Number(m.amount ?? 0);
          const formattedMAmount = formatAmount(mAmountNum, mintDecimals);
          const vendorStr = m.vendor ? m.vendor.toBase58() : "";
          const proofUri = m.proofUri || m.proof_uri;

          const submittedAt = Number(m.proofSubmittedAt ?? m.proof_submitted_at ?? 0);
          const approvedAt = Number(m.approvedAt ?? m.approved_at ?? 0);
          const releasedAt = Number(m.releasedAt ?? m.released_at ?? 0);

          return (
            <Card key={idx} title={`Milestone #${idx + 1}: ${m.label || "Untitled"}`}>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", flexWrap: "wrap", gap: "0.5rem" }}>
                  <span>
                    <strong>Amount:</strong> {mAmountNum} ({formattedMAmount})
                  </span>
                  <Badge status={mStatus as any}>
                    {mStatus.replace("_", " ").toUpperCase()}
                  </Badge>
                </div>

                <div>
                  <span style={{ fontWeight: 700 }}>Vendor: </span>
                  <span title={vendorStr}>
                    <Mono text={shortenAddress(vendorStr)} />
                  </span>
                </div>

                <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "0.5rem", fontSize: "0.875rem" }}>
                  <div><strong>Proof Submitted At:</strong> {submittedAt || "-"}</div>
                  <div><strong>Approved At:</strong> {approvedAt || "-"}</div>
                  <div><strong>Released At:</strong> {releasedAt || "-"}</div>
                </div>

                {proofUri && (
                  <div>
                    <strong>Proof URI: </strong>
                    <a href={proofUri} target="_blank" rel="noopener noreferrer" style={{ color: "var(--blue)", textDecoration: "underline" }}>
                      {proofUri}
                    </a>
                  </div>
                )}
              </div>
            </Card>
          );
        })
      ) : (
        <p>No milestones found for this campaign.</p>
      )}
    </div>
  );
};
