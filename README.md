# Bening

> **Donations you can see through.**

*Bening* is Indonesian for "clear" or "transparent". Bening is a donation platform on Solana (devnet) where every donation is held in on-chain escrow and released milestone by milestone, directly to vendors, only after the organizer submits proof and a designated verifier approves it.

| | |
|---|---|
| Live demo | `https://bening-roan.vercel.app/` |
| Demo video | `https://youtu.be/wRqfLwSvop0?si=RO1nJOUd0Hp_YR78` |
| Slides | `https://canva.link/9nidgtdetrv4nd9` |
| Program ID (devnet) | `3uemNohfUixociV872GGK1e4Dh2PRbPiNYT2SMgKXh5y` |

Built for the Encode Club x Solana Foundation Hackathon 2026, track **Best Consumer App on Solana**.

---

## The problem

Donors send money to an organizer's account and hope for the best. Spending is reported after the fact, if at all, and nobody can verify that the rice was bought or the cement delivered. Trust is the whole product, and it is easy to break.

## The solution

Bening removes the need to trust the organizer by making the money programmable.

1. **Create a campaign.** The organizer defines 1 to 5 milestones, each with a label, an amount and a vendor wallet, plus a verifier wallet.
2. **Donate.** Donors send tokens into a vault controlled by the program, not by any person.
3. **Submit proof.** The organizer submits a proof URL. The app computes its SHA-256 hash and stores hash and URL on-chain.
4. **Approve.** The verifier reviews the proof and approves the milestone.
5. **Release.** Anyone can trigger the payout. The milestone amount goes from the vault **directly to the vendor**.
6. **Audit.** Every donation, proof, approval and payout appears on a public timeline.

If a milestone is not documented and approved, its money does not move.

## How Solana powers Bening

- **Escrow without a custodian.** The vault is the associated token account of the Campaign PDA. No private key controls it; only the program can sign transfers out of it (CPI `transfer_checked` signed with the PDA seeds).
- **On-chain milestone state machine.** `Pending -> ProofSubmitted -> Approved -> Released`, one step at a time, never backward. A payout without proof is impossible by construction.
- **Role enforcement in the program.** Only the organizer can submit proof, only the verifier can approve (`has_one` constraints on the Campaign PDA).
- **Tamper-evident proof.** A SHA-256 hash of the proof is stored on-chain with its URI and timestamps.
- **Direct-to-vendor payments.** Funds never pass through the organizer.
- **Public, auditable ledger.** The timeline in the UI is a view of real on-chain data, not a database we control.
- **Fast and cheap.** Low fees make small donations practical.

## Architecture

### Accounts

| Account | Purpose |
|---|---|
| `Campaign` (PDA: `"campaign"`, organizer, campaign_id) | Organizer, verifier, mint, title, metadata URI (used as image URL), target, total raised, total released, donation count, status, and up to 5 milestones (label, amount, vendor, status, proof hash, proof URI, timestamps) |
| `Donation` (PDA: `"donation"`, campaign, index) | One record per donation: donor, amount, timestamp |
| Vault | Associated token account of the Campaign PDA for the campaign's mint |

### Instructions

| Instruction | Caller | Effect |
|---|---|---|
| `create_campaign` | Organizer | Creates the campaign, its milestones and the vault |
| `donate` | Anyone | Transfers tokens into the vault and records the donation |
| `submit_proof` | Organizer | `Pending -> ProofSubmitted` (hash, URI, timestamp) |
| `approve_milestone` | Verifier | `ProofSubmitted -> Approved` |
| `release_funds` | Anyone | `Approved -> Released`; pays the vendor from the vault |

The vendor's token account is created by the frontend (idempotent create-ATA in the same transaction); the program only validates it.

### Security notes

- Campaign PDA seeds and bump are checked on every instruction.
- The token program is pinned to the original SPL Token program on `release_funds` and `donate`.
- The vault must be the ATA of the Campaign PDA for the campaign mint.
- Release requires status `Approved`, so a milestone can be paid only once, and never more than the vault balance.
- State is updated before the token transfer CPI.
- Negative tests assert exact error codes (wrong role, wrong status, zero hash, URI too long, wrong mint, fake vault, wrong token program, and others).

### Known limitations

- Devnet only; no mainnet deployment.
- One verifier per campaign; no donor voting.
- No refunds or expiry for failed campaigns.
- Proof is a pasted URL. The stored hash is the SHA-256 of the URL text, not of the file behind it.
- Create, donate and proof hardening for the original Token program is enforced on `donate` and `release_funds`; `create_campaign` still accepts the token interface type (tracked as technical debt).

## Tech stack

- **Program:** Rust, Anchor 1.2.1 (`anchor-lang` and `anchor-spl` pinned to `=1.2.1`), original SPL Token program
- **Tests:** Rust with LiteSVM 0.10.0 (46 tests)
- **Frontend:** Vite, React, TypeScript, Solana wallet adapter (Phantom and Solflare), Anchor client, neobrutalism styling in plain CSS
- **Network:** Solana devnet

## Toolchain

- anchor-cli 1.2.1, solana-cli 4.1.2
- rustc/cargo 1.89.0 (via `rust-toolchain.toml`)
- node v24.x, npm 11.x

## Getting started

### Prerequisites

Rust, Solana CLI, Anchor CLI, Node.js, and a Phantom or Solflare wallet set to **devnet**.

### Clone

```bash
git clone https://github.com/TODO-user/TODO-repo.git
cd bening
```

### Build and test the program

`anchor build` currently produces SBPF v3 binaries that LiteSVM 0.10.0 rejects, so the tests use the v0 build:

```bash
cd programs/bening && cargo build-sbf --arch v0 && cd ../..
cargo test
```

Expected: all tests pass (46).

### Deploy to devnet

```bash
solana config set --url devnet
solana program deploy target/deploy/bening.so --program-id target/deploy/bening-keypair.json
```

The deployed program ID is `3uemNohfUixociV872GGK1e4Dh2PRbPiNYT2SMgKXh5y`. Devnet accepted the `--arch v0` binary.

### IDL

```bash
cd programs/bening && anchor idl build -o ../../target/idl/bening.json
```

A copy lives in `app/src/idl/bening.json`.

### Run the frontend

```bash
cd app
npm install
npm run dev
```

Open `http://localhost:5173` and connect a wallet on devnet.

## Try it (demo walkthrough)

You need three wallets on devnet with a little SOL each: **organizer**, **verifier**, and a **vendor** address (the vendor does not need to connect). A donor can be the organizer for a quick test.

1. **Faucet** (`/faucet`): create a test mint, then "Mint tokens to me". These are test tokens, not USDC. SOL pays transaction fees; the test token is what gets donated.
2. **Create** (`/create`): title, the mint, the verifier address, and 1 to 5 milestones with vendor addresses. You are redirected to the campaign page.
3. **Donate** (`/c/<campaign>`): donate up to the target. Tokens go into the vault.
4. **Submit proof** (organizer): paste a proof URL. Status becomes `ProofSubmitted`.
5. **Approve** (switch to the verifier wallet): status becomes `Approved`.
6. **Release** (any wallet): the vendor receives the tokens, and the public timeline shows donation, proof, approval and payout with timestamps.

## Project status

- [x] Anchor program: campaign, milestones, donation, vault
- [x] Proof submission and verifier approval
- [x] Fund release to vendor
- [x] Tests (happy path and negative cases)
- [x] Devnet deployment
- [x] Frontend: create, donate, proof, approve, release, public timeline, campaign list
- [ ] Proof upload to decentralized storage
- [ ] Proof verification button (recompute hash and compare)

## Roadmap

- Donor voting on milestones in addition to a verifier
- Campaign expiry and refunds
- Verifier reputation
- Proof files on IPFS/Arweave with file-level hashing
- Cross-chain donations (Aurora Intents), mainnet-only so out of scope for this hackathon

## License

MIT (`TODO: add a LICENSE file`)
