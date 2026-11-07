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
 * NOTE: This UI is UNVERIFIED. Only the human tests it in the browser.
 * Nothing here has been validated by an automated browser or E2E run.
 */

type BadgeStatus = "pending" | "proof_submitted" | "approved" | "released";

interface MilestoneView {
  index: number;
  label: string;
  amount: BN;
  vendor: string;
  status: BadgeStatus;
  proofUri: string;
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
 *
 * Rules:
 * - whitespace is stripped, empty input is rejected
 * - only digits and a single "." are allowed
 * - fractional digits must not exceed mintDecimals
 * - the value must be strictly positive
 *
 * Returns the base-unit amount as a BN.
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

  const walletAddress = wallet?.publicKey ? wallet.publicKey.toBase58() : null;

  /* ------------------------------------------------------------------ */
  /* Campaign loading                                                    */
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
      // Read-only program: account fetches do not need a connected wallet
      const program = getProgram(wallet ?? ({
        publicKey: PublicKey.default,
        signTransaction: async (tx: any) => tx,
        signAllTransactions: async (txs: any) => txs,
      } as any));

      const account = await (program.account as any).campaign.fetch(campaignKey);

      const milestones: MilestoneView[] = (account.milestones as any[]).map((m, index) => ({
        index,
        label: m.label as string,
        amount: new BN((m.amount as any).toString()),
        vendor: (m.vendor as PublicKey).toBase58(),
        status: normalizeStatus(m.status),
        proofUri: (m.proofUri as string) || "",
      }));

      const statusKey =
        typeof account.status === "string"
          ? account.status
          : Object.keys(account.status || {})[0] || "Active";

      setCampaign({
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
      });
      setLoadError("");

      const mintInfo = await getMint(connection, account.mint as PublicKey);
      setMintDecimals(mintInfo.decimals);
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

  // Read the donor's current balance for this mint (for the "no tokens" hint)
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
        // No token account yet means a zero balance
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
  /* Role checks: strict base58 string equality, never object identity   */
  /* ------------------------------------------------------------------ */

  const isConnected = walletAddress !== null;
  const isOrganizer = isConnected && campaign !== null && walletAddress === campaign.organizer;
  const isVerifier = isConnected && campaign !== null && walletAddress === campaign.verifier;
  const isCampaignActive = campaign?.status === "active";

  /* ------------------------------------------------------------------ */
  /* Transaction helper: build tx, sign with the wallet, poll for status */
  /* ------------------------------------------------------------------ */

  /**
   * Send a pre-built transaction with the connected wallet.
   *
   * How the signature is obtained: `wallet.signTransaction(tx)` asks the
   * wallet adapter to sign locally, then `connection.sendRawTransaction`
   * returns the transaction signature which we poll with
   * sendAndConfirmPolling (no WebSocket confirmTransaction).
   */
  const sendWithWallet = async (tx: any): Promise<string> => {
    if (!wallet?.publicKey) throw new Error("Please connect your wallet first.");
    // Anchor's .transaction() does not set these; the wallet needs them to sign.
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

      /*
       * Donation PDA, derived fresh from the LIVE campaign account right
       * before sending. Verified against programs/bening/src/instructions/
       * donate.rs and state.rs (Donation::index is u32):
       *   seeds = [b"donation", campaign.key().as_ref(), campaign.donation_count.to_le_bytes()]
       * donation_count is u32, so to_le_bytes() = exactly 4 bytes little-endian.
       */
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
        setDonateError("Could not read the latest campaign state; retrying once...");
        donationPda = await deriveDonationPda();
        setDonateError("");
      }

      // Does the donor already own a token account for this mint?
      const donorAtaExists = await connection
        .getAccountInfo(donorAta, "confirmed")
        .then((info) => info !== null)
        .catch(() => false);

      /*
       * donate instruction, copied from the IDL in bening.json.
       *
       * args:
       *   amount: u64
       *
       * accounts (name / writable / signer):
       *   donor                    mut  signer
       *   campaign                 mut
       *   donation                 mut   (PDA, init: seeds ["donation", campaign, donation_count u32 LE])
       *   donorAta                 mut   (ATA of donor for mint)
       *   vault                    mut   (ATA of campaign PDA for mint)
       *   mint                     mut
       *   systemProgram
       *   tokenProgram
       *   associatedTokenProgram
       *   clock                    (Sysvar<Clock>, required by the handler)
       *
       * Every PDA/sysvar above is passed explicitly so Anchor does not have
       * to resolve anything itself.
       */
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

      // Create the donor ATA idempotently in the SAME transaction when missing.
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

      // Refetch campaign and donor balance after confirmation
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

      /*
       * submit_proof instruction, copied from the IDL in bening.json.
       *
       * args:
       *   milestoneIndex: u8
       *   proofHash:      [u8; 32]
       *   proofUri:       string
       *
       * accounts:
       *   organizer  signer (must equal campaign.organizer)
       *   campaign   mut (PDA, seeds ["campaign", organizer, campaign_id u64 LE])
       */
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
  /* C.3 Approve milestone (verifier only), one per milestone           */
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

      /*
       * approve_milestone instruction, copied from the IDL in bening.json.
       *
       * args:
       *   milestoneIndex: u8
       *
       * accounts:
       *   verifier  signer (has_one = campaign.verifier)
       *   campaign  mut (PDA, seeds ["campaign", organizer, campaign_id u64 LE])
       */
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
      {/* Campaign header */}
      <Card title={campaign.title || "Untitled campaign"}>
        <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
          <div>
            <strong>Status: </strong>
            {campaign.status === "completed" ? "Completed" : "Active"}
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

      {/* Milestones overview */}
      <Card title="Milestones">
        <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
          {campaign.milestones.map((m) => (
            <div key={m.index} style={{ border: "2px solid var(--ink)", padding: "0.75rem" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: "0.5rem" }}>
                <strong>
                  {m.index + 1}. {m.label}
                </strong>
                <Badge status={m.status}>{m.status.replace("_", " ")}</Badge>
              </div>
              <div style={{ fontSize: "0.875rem", marginTop: "0.25rem" }}>
                Amount: {formatBaseUnits(m.amount, decimals)} tokens
              </div>
              <Mono text={`Vendor: ${m.vendor}`} />
              {m.proofUri && (
                <div style={{ fontSize: "0.875rem", wordBreak: "break-all" }}>
                  Proof:{" "}
                  <a href={m.proofUri} target="_blank" rel="noopener noreferrer">
                    {m.proofUri}
                  </a>
                </div>
              )}
            </div>
          ))}
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
    </div>
  );
};
