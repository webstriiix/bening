import React, { useCallback, useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { useConnection, useAnchorWallet } from "@solana/wallet-adapter-react";
import { PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY } from "@solana/web3.js";
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountIdempotentInstruction,
  getAccount,
  getMint,
} from "@solana/spl-token";
import BN from "bn.js";
import { getProgram } from "../utils/anchorProvider";
import { parseAnchorError } from "../utils/errors";
import { sendAndConfirmPolling } from "../utils/sendAndConfirmPolling";
import Card from "../components/Card";
import Button from "../components/Button";
import Badge from "../components/Badge";
import Mono from "../components/Mono";
import TextInput from "../components/TextInput";
import ProgressBar from "../components/ProgressBar";

/*
 * NOTE: The UI is unverified; only the human tests in the browser.
 */

type BadgeStatus = "pending" | "proof_submitted" | "approved" | "released";

interface MilestoneView {
  index: number;
  label: string;
  amount: BN;
  vendor: string;
  status: BadgeStatus;
  proofUri: string;
  proofSubmittedAt: number;
  approvedAt: number;
  releasedAt: number;
}

interface DonationView {
  donor: string;
  amount: BN;
  timestamp: number;
  index: number;
}

interface CampaignView {
  address: string;
  organizer: string;
  verifier: string;
  mint: string;
  campaignId: BN;
  title: string;
  status: "active" | "completed";
  targetAmount: BN;
  totalRaised: BN;
  donationCount: number;
  milestones: MilestoneView[];
}

const PROGRAM_ID = new PublicKey("3uemNohfUixociV872GGK1e4Dh2PRbPiNYT2SMgKXh5y");
const MAX_URI_BYTES = 128;

/** Convert a MilestoneStatus enum (Anchor object or string) to a badge key. */
function normalizeStatus(raw: unknown): BadgeStatus {
  let key = "Pending";
  if (typeof raw === "string") {
    key = raw;
  } else if (raw && typeof raw === "object") {
    const keys = Object.keys(raw as Record<string, unknown>);
    if (keys.length > 0) key = keys[0];
  }
  switch (key.toLowerCase()) {
    case "proofsubmitted":
      return "proof_submitted";
    case "approved":
      return "approved";
    case "released":
      return "released";
    default:
      return "pending";
  }
}

/**
 * Parse a decimal token string into base units without any floating point.
 */
export function parseTokenAmount(input: string, mintDecimals: number): BN {
  const cleaned = input.replace(/\s+/g, "");
  if (!cleaned) throw new Error("Enter an amount in whole tokens.");
  if (!/^\d*(\.\d*)?$/.test(cleaned) || cleaned === ".") {
    throw new Error("Enter a valid number, e.g. 12.5");
  }

  const [wholePart = "", fractionPart = ""] = cleaned.split(".");
  if (fractionPart.length > mintDecimals) {
    throw new Error(
      `Too many decimals: this token supports at most ${mintDecimals} decimal place(s).`
    );
  }

  const whole = wholePart === "" ? "0" : wholePart;
  const fraction = fractionPart.padEnd(mintDecimals, "0");
  const baseUnits = BigInt(`${whole}${fraction}`.replace(/^0+(?=\d)/, ""));

  if (baseUnits <= BigInt(0)) {
    throw new Error("Amount must be greater than 0.");
  }

  return new BN(baseUnits.toString());
}

/** Format base units as a whole-token string for display. */
function formatBaseUnits(value: BN, decimals: number): string {
  const str = value.toString().padStart(decimals + 1, "0");
  const whole = str.slice(0, str.length - decimals);
  const fraction = str.slice(str.length - decimals).replace(/0+$/, "");
  return fraction ? `${whole}.${fraction}` : whole;
}

function formatTimestamp(ts: number): string {
  if (!ts || ts <= 0) return "-";
  return new Date(ts * 1000).toLocaleString();
}

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export const CampaignDetails: React.FC = () => {
  const { connection } = useConnection();
  const wallet = useAnchorWallet();
  const { campaignPda } = useParams<{ campaignPda: string }>();

  // Campaign state, loaded from chain
  const [campaign, setCampaign] = useState<CampaignView | null>(null);
  const [loadError, setLoadError] = useState("");
  const [loading, setLoading] = useState(true);
  const [mintDecimals, setMintDecimals] = useState<number | null>(null);
  const [vaultBalance, setVaultBalance] = useState<BN>(new BN(0));
  const [donations, setDonations] = useState<DonationView[]>([]);

  // Donate panel
  const [donateAmount, setDonateAmount] = useState("");
  const [donateLoading, setDonateLoading] = useState(false);
  const [donateError, setDonateError] = useState("");
  const [donateSig, setDonateSig] = useState<string | null>(null);
  const [donorBalance, setDonorBalance] = useState<BN | null>(null);

  // Submit proof panel
  const [proofMilestone, setProofMilestone] = useState<number | null>(null);
  const [proofUrl, setProofUrl] = useState("");
  const [proofLoading, setProofLoading] = useState(false);
  const [proofError, setProofError] = useState("");
  const [proofHashHex, setProofHashHex] = useState<string | null>(null);

  // Approve panel: one in-flight milestone at a time
  const [approvingIndex, setApprovingIndex] = useState<number | null>(null);
  const [approveError, setApproveError] = useState("");

  // Release panel
  const [releasingIndex, setReleasingIndex] = useState<number | null>(null);
  const [releaseError, setReleaseError] = useState("");
  const [releaseSig, setReleaseSig] = useState<string | null>(null);

  const walletAddress = wallet?.publicKey ? wallet.publicKey.toBase58() : null;

  /* ------------------------------------------------------------------ */
  /* Campaign loading & Donations                                      */
  /* ------------------------------------------------------------------ */

  const fetchCampaign = useCallback(async () => {
    if (!campaignPda) {
      setLoadError("No campaign address in the URL.");
      setLoading(false);
      return;
    }

    let campaignKey: PublicKey;
    try {
      campaignKey = new PublicKey(campaignPda);
    } catch {
      setLoadError("That campaign address is not a valid Solana public key.");
      setLoading(false);
      return;
    }

    try {
      const readProgram = getProgram(
        wallet ??
          ({
            publicKey: PublicKey.default,
            signTransaction: async (tx: any) => tx,
            signAllTransactions: async (txs: any) => txs,
          } as any)
      );

      const account = await (readProgram.account as any).campaign.fetch(campaignKey);

      const milestones: MilestoneView[] = (account.milestones as any[]).map((m, index) => ({
        index,
        label: m.label as string,
        amount: new BN((m.amount as any).toString()),
        vendor: (m.vendor as PublicKey).toBase58(),
        status: normalizeStatus(m.status),
        proofUri: (m.proofUri as string) || "",
        proofSubmittedAt: Number(m.proofSubmittedAt || 0),
        approvedAt: Number(m.approvedAt || 0),
        releasedAt: Number(m.releasedAt || 0),
      }));

      const statusKey =
        typeof account.status === "string"
          ? account.status
          : Object.keys(account.status || {})[0] || "Active";

      const campaignView: CampaignView = {
        address: campaignKey.toBase58(),
        organizer: (account.organizer as PublicKey).toBase58(),
        verifier: (account.verifier as PublicKey).toBase58(),
        mint: (account.mint as PublicKey).toBase58(),
        campaignId: new BN((account.campaignId as any).toString()),
        title: account.title as string,
        status: statusKey.toLowerCase() === "completed" ? "completed" : "active",
        targetAmount: new BN((account.targetAmount as any).toString()),
        totalRaised: new BN((account.totalRaised as any).toString()),
        donationCount: Number(account.donationCount),
        milestones,
      };

      setCampaign(campaignView);
      setLoadError("");

      const mintPubkey = account.mint as PublicKey;
      const mintInfo = await getMint(connection, mintPubkey);
      setMintDecimals(mintInfo.decimals);

      // Fetch vault balance
      try {
        const vaultAta = await getAssociatedTokenAddress(
          mintPubkey,
          campaignKey,
          true,
          TOKEN_PROGRAM_ID
        );
        const vaultAcc = await getAccount(connection, vaultAta, "confirmed", TOKEN_PROGRAM_ID);
        setVaultBalance(new BN(vaultAcc.amount.toString()));
      } catch {
        setVaultBalance(new BN(0));
      }

      // Fetch donations for this campaign (capped at 50 newest)
      try {
        const allDonations = await (readProgram.account as any).donation.all();
        const filtered: DonationView[] = allDonations
          .filter((d: any) => (d.account.campaign as PublicKey).equals(campaignKey))
          .map((d: any) => ({
            donor: (d.account.donor as PublicKey).toBase58(),
            amount: new BN((d.account.amount as any).toString()),
            timestamp: Number(d.account.timestamp || 0),
            index: Number(d.account.index || 0),
          }))
          .sort((a: DonationView, b: DonationView) => b.index - a.index)
          .slice(0, 50);
        setDonations(filtered);
      } catch (err) {
        console.error("Failed to fetch donations:", err);
        setDonations([]);
      }
    } catch (err: any) {
      console.error(err);
      setLoadError(
        `Could not load this campaign from devnet. ${parseAnchorError(err)}`
      );
    } finally {
      setLoading(false);
    }
  }, [campaignPda, connection, wallet]);

  useEffect(() => {
    setLoading(true);
    void fetchCampaign();
  }, [fetchCampaign]);

  const fetchDonorBalance = useCallback(async () => {
    if (!wallet?.publicKey || !campaign) {
      setDonorBalance(null);
      return;
    }
    try {
      const ata = await getAssociatedTokenAddress(
        new PublicKey(campaign.mint),
        wallet.publicKey,
        true,
        TOKEN_PROGRAM_ID
      );
      try {
        const account = await getAccount(connection, ata, "confirmed", TOKEN_PROGRAM_ID);
        setDonorBalance(new BN(account.amount.toString()));
      } catch {
        setDonorBalance(new BN(0));
      }
    } catch (err) {
      console.error(err);
      setDonorBalance(null);
    }
  }, [wallet, campaign, connection]);

  useEffect(() => {
    void fetchDonorBalance();
  }, [fetchDonorBalance]);

  /* ------------------------------------------------------------------ */
  /* Role checks & Status                                              */
  /* ------------------------------------------------------------------ */

  const isConnected = walletAddress !== null;
  const isOrganizer = isConnected && campaign !== null && walletAddress === campaign.organizer;
  const isVerifier = isConnected && campaign !== null && walletAddress === campaign.verifier;
  const isCampaignActive = campaign?.status === "active";
  const allMilestonesReleased =
    campaign !== null &&
    campaign.milestones.length > 0 &&
    campaign.milestones.every((m) => m.status === "released");

  /* ------------------------------------------------------------------ */
  /* Transaction sender helper                                         */
  /* ------------------------------------------------------------------ */

  const sendWithWallet = async (tx: any): Promise<string> => {
    if (!wallet?.publicKey) throw new Error("Please connect your wallet first.");
    const { blockhash } = await connection.getLatestBlockhash("confirmed");
    tx.recentBlockhash = blockhash;
    tx.feePayer = wallet.publicKey;
    const signed = await wallet.signTransaction(tx);
    const signature = await connection.sendRawTransaction(signed.serialize(), {
      skipPreflight: false,
    });
    await sendAndConfirmPolling(connection, signature);
    return signature;
  };

  /* ------------------------------------------------------------------ */
  /* C.1 Donate                                                          */
  /* ------------------------------------------------------------------ */

  const handleDonate = async () => {
    if (!wallet?.publicKey || !wallet.signTransaction) {
      setDonateError("Please connect your wallet first.");
      return;
    }
    if (!campaign || mintDecimals === null) {
      setDonateError("Campaign is still loading. Try again in a moment.");
      return;
    }

    let baseUnits: BN;
    try {
      baseUnits = parseTokenAmount(donateAmount, mintDecimals);
    } catch (err: any) {
      setDonateError(err.message ?? "Enter a valid amount.");
      return;
    }

    setDonateLoading(true);
    setDonateError("");
    setDonateSig(null);

    try {
      const campaignKey = new PublicKey(campaign.address);
      const mintKey = new PublicKey(campaign.mint);
      const program = getProgram(wallet);

      const donorAta = await getAssociatedTokenAddress(
        mintKey,
        wallet.publicKey,
        false,
        TOKEN_PROGRAM_ID
      );
      const vault = await getAssociatedTokenAddress(
        mintKey,
        campaignKey,
        true,
        TOKEN_PROGRAM_ID
      );

      const deriveDonationPda = async (): Promise<PublicKey> => {
        const live = await (program.account as any).campaign.fetch(campaignKey);
        const countBytes = new Uint8Array(4);
        new DataView(countBytes.buffer).setUint32(0, Number(live.donationCount), true);
        const [pda] = PublicKey.findProgramAddressSync(
          [Buffer.from("donation"), campaignKey.toBuffer(), Buffer.from(countBytes)],
          PROGRAM_ID
        );
        return pda;
      };

      let donationPda: PublicKey;
      try {
        donationPda = await deriveDonationPda();
      } catch (firstErr) {
        console.error("Donation PDA derivation failed, retrying once:", firstErr);
        donationPda = await deriveDonationPda();
      }

      const donorAtaExists = await connection
        .getAccountInfo(donorAta, "confirmed")
        .then((info) => info !== null)
        .catch(() => false);

      const tx = await (program.methods as any)
        .donate(baseUnits)
        .accounts({
          donor: wallet.publicKey,
          campaign: campaignKey,
          donation: donationPda,
          donorAta,
          vault,
          mint: mintKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          clock: SYSVAR_CLOCK_PUBKEY,
        })
        .transaction();

      if (!donorAtaExists) {
        const createAtaIx = createAssociatedTokenAccountIdempotentInstruction(
          wallet.publicKey,
          donorAta,
          wallet.publicKey,
          mintKey,
          TOKEN_PROGRAM_ID
        );
        tx.instructions.unshift(createAtaIx);
      }

      const signature = await sendWithWallet(tx);
      setDonateSig(signature);
      setDonateAmount("");

      await fetchCampaign();
      await fetchDonorBalance();
    } catch (err: any) {
      console.error(err);
      setDonateError(
        `Donation failed. ${parseAnchorError(err)} You can retry — nothing was silently dropped.`
      );
    } finally {
      setDonateLoading(false);
    }
  };

  /* ------------------------------------------------------------------ */
  /* C.2 Submit proof (organizer only)                                   */
  /* ------------------------------------------------------------------ */

  const handleSubmitProof = async () => {
    if (!isOrganizer) {
      setProofError("Only the campaign organizer can submit proof.");
      return;
    }
    if (!campaign) return;
    if (proofMilestone === null) {
      setProofError("Select a pending milestone first.");
      return;
    }

    const trimmedUrl = proofUrl.trim();
    if (!trimmedUrl) {
      setProofError("Enter a proof URL.");
      return;
    }
    const urlBytes = new TextEncoder().encode(trimmedUrl);
    if (urlBytes.length > MAX_URI_BYTES) {
      setProofError(
        `Proof URL is ${urlBytes.length} bytes; the limit is ${MAX_URI_BYTES} bytes (UTF-8).`
      );
      return;
    }

    setProofLoading(true);
    setProofError("");
    setProofHashHex(null);

    try {
      const hashBuffer = await crypto.subtle.digest("SHA-256", urlBytes);
      const hashArr = new Uint8Array(hashBuffer);
      setProofHashHex(toHex(hashArr));

      const campaignKey = new PublicKey(campaign.address);
      const program = getProgram(wallet!);

      const tx = await (program.methods as any)
        .submitProof(proofMilestone, Array.from(hashArr), trimmedUrl)
        .accounts({
          organizer: wallet!.publicKey,
          campaign: campaignKey,
        })
        .transaction();

      await sendWithWallet(tx);

      setProofUrl("");
      setProofMilestone(null);
      await fetchCampaign();
    } catch (err: any) {
      console.error(err);
      setProofError(`Submit proof failed. ${parseAnchorError(err)}`);
    } finally {
      setProofLoading(false);
    }
  };

  /* ------------------------------------------------------------------ */
  /* C.3 Approve milestone (verifier only)                              */
  /* ------------------------------------------------------------------ */

  const handleApprove = async (milestoneIndex: number) => {
    if (!isVerifier) {
      setApproveError("Only the campaign verifier can approve milestones.");
      return;
    }
    if (!campaign) return;

    setApprovingIndex(milestoneIndex);
    setApproveError("");

    try {
      const campaignKey = new PublicKey(campaign.address);
      const program = getProgram(wallet!);

      const tx = await (program.methods as any)
        .approveMilestone(milestoneIndex)
        .accounts({
          verifier: wallet!.publicKey,
          campaign: campaignKey,
        })
        .transaction();

      await sendWithWallet(tx);
      await fetchCampaign();
    } catch (err: any) {
      console.error(err);
      setApproveError(`Approve failed. ${parseAnchorError(err)}`);
    } finally {
      setApprovingIndex(null);
    }
  };

  /* ------------------------------------------------------------------ */
  /* Task 1 — Release Funds (any connected wallet)                      */
  /* ------------------------------------------------------------------ */

  const handleRelease = async (milestoneIndex: number) => {
    if (!isConnected) {
      setReleaseError("Please connect your wallet first.");
      return;
    }
    if (!campaign) return;

    const milestone = campaign.milestones[milestoneIndex];
    if (!milestone) return;

    if (vaultBalance.lt(milestone.amount)) {
      setReleaseError("Vault does not hold enough tokens yet; donate first.");
      return;
    }

    setReleasingIndex(milestoneIndex);
    setReleaseError("");
    setReleaseSig(null);

    try {
      const campaignKey = new PublicKey(campaign.address);
      const mintKey = new PublicKey(campaign.mint);
      const vendorPubkey = new PublicKey(milestone.vendor);
      const program = getProgram(wallet!);

      const vault = await getAssociatedTokenAddress(
        mintKey,
        campaignKey,
        true,
        TOKEN_PROGRAM_ID
      );
      const vendorTokenAccount = await getAssociatedTokenAddress(
        mintKey,
        vendorPubkey,
        false,
        TOKEN_PROGRAM_ID
      );

      /*
       * release_funds instruction, copied from the IDL in bening.json.
       *
       * args:
       *   milestoneIndex: u8
       *
       * accounts:
       *   caller             signer (any connected wallet)
       *   campaign           mut (PDA, seeds ["campaign", organizer, campaign_id u64 LE])
       *   mint               (campaign mint)
       *   vault              mut (ATA of campaign PDA for mint)
       *   vendorTokenAccount mut (ATA of milestone.vendor for mint)
       *   tokenProgram       (TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA)
       */
      const tx = await (program.methods as any)
        .releaseFunds(milestoneIndex)
        .accounts({
          caller: wallet!.publicKey,
          campaign: campaignKey,
          mint: mintKey,
          vault,
          vendorTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .transaction();

      // Idempotent ATA creation for the vendor ATA in the SAME transaction
      const createVendorAtaIx = createAssociatedTokenAccountIdempotentInstruction(
        wallet!.publicKey,
        vendorTokenAccount,
        vendorPubkey,
        mintKey,
        TOKEN_PROGRAM_ID
      );
      tx.instructions.unshift(createVendorAtaIx);

      const signature = await sendWithWallet(tx);
      setReleaseSig(signature);
      await fetchCampaign();
    } catch (err: any) {
      console.error(err);
      const msg = parseAnchorError(err);
      if (msg.includes("InsufficientVaultBalance") || err.message?.includes("InsufficientVaultBalance")) {
        setReleaseError("Vault does not hold enough tokens yet; donate first.");
      } else {
        setReleaseError(`Release failed. ${msg}`);
      }
    } finally {
      setReleasingIndex(null);
    }
  };

  /* ------------------------------------------------------------------ */
  /* Render                                                              */
  /* ------------------------------------------------------------------ */

  if (loading) {
    return <p>Loading campaign from devnet…</p>;
  }

  if (loadError || !campaign) {
    return (
      <Card title="Campaign">
        <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
          {loadError || "Campaign not found."}
        </div>
      </Card>
    );
  }

  const pendingMilestones = campaign.milestones.filter((m) => m.status === "pending");
  const submittedMilestones = campaign.milestones.filter((m) => m.status === "proof_submitted");
  const approvedMilestones = campaign.milestones.filter((m) => m.status === "approved");
  const decimals = mintDecimals ?? 0;
  const raised = campaign.totalRaised.toNumber();
  const target = campaign.targetAmount.toNumber();
  const hasNoTokens = donorBalance !== null && donorBalance.isZero();

  const roleHint = !isConnected
    ? null
    : !isOrganizer && !isVerifier
      ? "Your wallet is neither the organizer nor the verifier of this campaign."
      : null;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem", paddingBottom: "2rem" }}>
      {/* Campaign Completed Banner */}
      {(campaign.status === "completed" || allMilestonesReleased) && (
        <div style={{ backgroundColor: "var(--green)", color: "var(--ink)", padding: "1.25rem", border: "3px solid var(--ink)", boxShadow: "6px 6px 0 var(--ink)", fontWeight: 800, fontSize: "1.25rem", textAlign: "center" }}>
          Campaign completed
        </div>
      )}

      {/* Campaign header */}
      <Card title={campaign.title || "Untitled campaign"}>
        <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
          <div>
            <strong>Status: </strong>
            {campaign.status === "completed" || allMilestonesReleased ? "Completed" : "Active"}
          </div>
          <Mono text={`Campaign PDA: ${campaign.address}`} />
          <Mono text={`Mint: ${campaign.mint}`} />
          <Mono text={`Organizer: ${campaign.organizer}`} />
          <Mono text={`Verifier: ${campaign.verifier}`} />
          <div style={{ marginTop: "0.75rem" }}>
            <strong>Raised: </strong>
            {formatBaseUnits(campaign.totalRaised, decimals)} /{" "}
            {formatBaseUnits(campaign.targetAmount, decimals)} tokens
          </div>
          <ProgressBar target={target} current={raised} />
          {roleHint && (
            <div style={{ marginTop: "0.75rem", backgroundColor: "var(--yellow)", padding: "0.75rem", border: "2px solid var(--ink)" }}>
              <div style={{ fontWeight: 700 }}>{roleHint}</div>
              <Mono text={`Connected:          ${walletAddress}`} />
              <Mono text={`Expected Organizer: ${campaign.organizer}`} />
              <Mono text={`Expected Verifier:  ${campaign.verifier}`} />
            </div>
          )}
        </div>
      </Card>

      {/* Task 2.1 — Public Milestone Timeline (Read-Only) */}
      <Card title="Milestone Timeline">
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          {campaign.milestones.map((m) => {
            // Compute vendor ATA for explorer link when released
            // (We can display a note or link if released)
            return (
              <div key={m.index} style={{ border: "2px solid var(--ink)", padding: "1rem", backgroundColor: "var(--bg)" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "0.5rem", marginBottom: "0.5rem" }}>
                  <strong>
                    {m.index + 1}. {m.label}
                  </strong>
                  <Badge status={m.status}>{m.status.replace("_", " ")}</Badge>
                </div>
                <div style={{ fontSize: "0.875rem", marginBottom: "0.25rem" }}>
                  Amount: {formatBaseUnits(m.amount, decimals)} tokens
                </div>
                <div style={{ fontSize: "0.8125rem", marginBottom: "0.25rem" }}>
                  Vendor:{" "}
                  <a
                    href={`https://explorer.solana.com/address/${m.vendor}?cluster=devnet`}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    <Mono text={m.vendor} style={{ display: "inline" }} /> ↗
                  </a>
                </div>
                {m.proofUri && m.proofUri.startsWith("https://") && (
                  <div style={{ fontSize: "0.875rem", wordBreak: "break-all", marginBottom: "0.25rem" }}>
                    Proof URL:{" "}
                    <a href={m.proofUri} target="_blank" rel="noopener noreferrer">
                      {m.proofUri} ↗
                    </a>
                  </div>
                )}
                <div style={{ fontSize: "0.75rem", marginTop: "0.5rem", display: "flex", flexDirection: "column", gap: "0.15rem", opacity: 0.85 }}>
                  <div>Proof Submitted: {formatTimestamp(m.proofSubmittedAt)}</div>
                  <div>Approved: {formatTimestamp(m.approvedAt)}</div>
                  <div>Released: {formatTimestamp(m.releasedAt)}</div>
                </div>
              </div>
            );
          })}
        </div>
      </Card>

      {/* Task 2.2 — Donations List (Read-Only) */}
      <Card title="Donations">
        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          {donations.length === 0 ? (
            <div style={{ fontWeight: 700 }}>No donations yet.</div>
          ) : (
            donations.map((d, idx) => (
              <div key={idx} style={{ display: "flex", justifyContent: "space-between", alignItems: "center", borderBottom: "1px solid var(--ink)", paddingBottom: "0.5rem", gap: "1rem", flexWrap: "wrap" }}>
                <div>
                  <span style={{ fontSize: "0.75rem", fontWeight: 700, marginRight: "0.5rem" }}>#{d.index + 1}</span>
                  <Mono text={d.donor} style={{ display: "inline" }} />
                </div>
                <div style={{ textAlign: "right" }}>
                  <div style={{ fontWeight: 700 }}>{formatBaseUnits(d.amount, decimals)} tokens</div>
                  <div style={{ fontSize: "0.75rem", opacity: 0.75 }}>{formatTimestamp(d.timestamp)}</div>
                </div>
              </div>
            ))
          )}
        </div>
      </Card>

      {/* C.1 Donate */}
      <Card title="Donate">
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          <TextInput
            label={`Amount (whole tokens, up to ${decimals} decimals)`}
            value={donateAmount}
            onChange={(e) => setDonateAmount(e.target.value)}
            placeholder="10"
            disabled={!isConnected || !isCampaignActive}
          />
          <Button
            variant="primary"
            onClick={handleDonate}
            disabled={donateLoading || !isConnected || !isCampaignActive}
          >
            {donateLoading ? "Sending…" : "Donate"}
          </Button>
          {!isConnected && (
            <div style={{ fontWeight: 700 }}>Connect your wallet to donate.</div>
          )}
          {isConnected && !isCampaignActive && (
            <div style={{ fontWeight: 700 }}>This campaign is no longer accepting donations.</div>
          )}
          {hasNoTokens && (
            <div style={{ backgroundColor: "var(--yellow)", padding: "0.75rem", border: "2px solid var(--ink)" }}>
              You hold no tokens for this campaign's mint. Get some from the <a href="/faucet">faucet</a> first.
            </div>
          )}
          {donateError && (
            <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
              {donateError}
            </div>
          )}
          {donateSig && (
            <div style={{ backgroundColor: "var(--green)", padding: "0.75rem", border: "2px solid var(--ink)", wordBreak: "break-all" }}>
              <div style={{ fontWeight: 700 }}>Donation confirmed.</div>
              <a href={`https://explorer.solana.com/tx/${donateSig}?cluster=devnet`} target="_blank" rel="noopener noreferrer">
                View on Explorer ↗
              </a>
            </div>
          )}
        </div>
      </Card>

      {/* C.2 Submit proof — organizer only, pending milestones */}
      <Card title="Submit Proof (organizer only)">
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
          {pendingMilestones.length === 0 ? (
            <div style={{ fontWeight: 700 }}>No milestones are pending proof submission.</div>
          ) : (
            <>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
                {pendingMilestones.map((m) => (
                  <label key={m.index} style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}>
                    <input
                      type="radio"
                      name="proof-milestone"
                      checked={proofMilestone === m.index}
                      onChange={() => setProofMilestone(m.index)}
                      disabled={!isOrganizer}
                    />
                    Milestone {m.index + 1}: {m.label}
                  </label>
                ))}
              </div>
              <TextInput
                label="Proof URL (max 128 bytes, UTF-8)"
                value={proofUrl}
                onChange={(e) => setProofUrl(e.target.value)}
                placeholder="https://example.com/proof"
                disabled={!isOrganizer}
              />
              <Button
                variant="ok"
                onClick={handleSubmitProof}
                disabled={proofLoading || !isOrganizer || proofMilestone === null || !proofUrl.trim()}
              >
                {proofLoading
                  ? "Submitting…"
                  : proofMilestone !== null
                    ? `Submit proof for milestone ${proofMilestone + 1}`
                    : "Submit proof"}
              </Button>
              {!isConnected && <div style={{ fontWeight: 700 }}>Connect your wallet to submit proof.</div>}
              {isConnected && !isOrganizer && (
                <div style={{ fontWeight: 700 }}>Only the organizer can submit proof.</div>
              )}
              {proofError && (
                <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
                  {proofError}
                </div>
              )}
              {proofHashHex && (
                <div>
                  <div style={{ fontWeight: 700 }}>SHA-256 of the proof URL:</div>
                  <Mono text={proofHashHex} style={{ wordBreak: "break-all", whiteSpace: "pre-wrap" }} />
                </div>
              )}
            </>
          )}
        </div>
      </Card>

      {/* C.3 Approve — verifier only, one button per submitted milestone */}
      <Card title="Approve Milestones (verifier only)">
        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          {submittedMilestones.length === 0 ? (
            <div style={{ fontWeight: 700 }}>No milestones have submitted proofs to approve.</div>
          ) : (
            submittedMilestones.map((m) => (
              <div key={m.index} style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
                <Button
                  variant="primary"
                  onClick={() => handleApprove(m.index)}
                  disabled={approvingIndex !== null || !isVerifier}
                >
                  {approvingIndex === m.index ? "Approving…" : `Approve Milestone ${m.index + 1}`}
                </Button>
                <div style={{ fontSize: "0.875rem" }}>
                  {m.label} — {formatBaseUnits(m.amount, decimals)} tokens
                </div>
                {m.proofUri && (
                  <a href={m.proofUri} target="_blank" rel="noopener noreferrer" style={{ fontSize: "0.8125rem", wordBreak: "break-all" }}>
                    {m.proofUri}
                  </a>
                )}
              </div>
            ))
          )}
          {!isConnected && <div style={{ fontWeight: 700 }}>Connect your wallet to approve milestones.</div>}
          {isConnected && !isVerifier && (
            <div style={{ fontWeight: 700 }}>
              Only the verifier can approve milestones. Connected: {walletAddress}; expected verifier:{" "}
              {campaign.verifier}.
            </div>
          )}
          {approveError && (
            <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
              {approveError}
            </div>
          )}
        </div>
      </Card>

      {/* Task 1 — Release Funds Panel (any connected wallet) */}
      <Card title="Release Funds (anyone can release approved milestones)">
        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          <div style={{ fontSize: "0.875rem", marginBottom: "0.25rem" }}>
            Vault token balance: <strong>{formatBaseUnits(vaultBalance, decimals)}</strong> tokens
          </div>
          {approvedMilestones.length === 0 ? (
            <div style={{ fontWeight: 700 }}>No approved milestones ready for release.</div>
          ) : (
            approvedMilestones.map((m) => {
              const insufficientVault = vaultBalance.lt(m.amount);
              return (
                <div key={m.index} style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
                  <Button
                    variant="ok"
                    onClick={() => handleRelease(m.index)}
                    disabled={releasingIndex !== null || !isConnected || insufficientVault}
                  >
                    {releasingIndex === m.index
                      ? "Releasing…"
                      : insufficientVault
                        ? "Vault does not hold enough tokens yet; donate first"
                        : `Release Milestone ${m.index + 1}`}
                  </Button>
                  <div style={{ fontSize: "0.875rem" }}>
                    {m.label} — {formatBaseUnits(m.amount, decimals)} tokens (Vendor: <Mono text={m.vendor} style={{ display: "inline" }} />)
                  </div>
                </div>
              );
            })
          )}
          {!isConnected && <div style={{ fontWeight: 700 }}>Connect your wallet to release milestone funds.</div>}
          {releaseError && (
            <div style={{ backgroundColor: "var(--pink)", padding: "0.75rem", border: "2px solid var(--ink)", fontWeight: 700 }}>
              {releaseError}
            </div>
          )}
          {releaseSig && (
            <div style={{ backgroundColor: "var(--green)", padding: "0.75rem", border: "2px solid var(--ink)", wordBreak: "break-all" }}>
              <div style={{ fontWeight: 700 }}>Milestone funds released successfully.</div>
              <a href={`https://explorer.solana.com/tx/${releaseSig}?cluster=devnet`} target="_blank" rel="noopener noreferrer">
                View on Explorer ↗
              </a>
            </div>
          )}
        </div>
      </Card>
    </div>
  );
};
