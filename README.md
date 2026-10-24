# Bening

> **Donations you can see through.**

*Bening* is Indonesian for "clear" or "transparent". Bening is a donation platform on Solana where every donation is held in on-chain escrow and released in milestones, directly to vendors, only after organizers document how the money will be spent.

**Quick links**

| | |
|---|---|
| Live demo | `<Coming Soon>` |
| Demo video | `<Coming Soon>` |
| Program ID (devnet) | `<Coming soon>` |
| Slides | `<Coming Soon>` |

---

## The problem

Today, donors send money to an organizer's account and hope for the best. Receipts are optional, spending is reported after the fact (if at all), and there is no way to verify that rice was actually bought or that cement actually reached the construction site. Trust is the whole product, and it is easy to break.

## The solution

Bening removes the need to trust the organizer by making the money *programmable*:

1. **Create a campaign.** The organizer defines a target and a list of milestones, each with a description (e.g. "Buy 500 kg of rice"), an amount, and the wallet of the vendor who will be paid.
2. **Donate.** Donors send USDC. Funds go into a vault controlled by the program, not by any person.
3. **Document.** After buying the goods, the organizer uploads proof (receipt, photo) to decentralized storage and submits its hash on-chain.
4. **Verify.** A designated verifier reviews the proof and approves the milestone.
5. **Release.** Only then does the program pay out that milestone's amount, **directly to the vendor's wallet**.
6. **Audit.** Every donation, proof, approval and payout appears on a public timeline anyone can check.

If the organizer does not document a milestone, the money does not move.

## How Solana powers Bening

- **Escrow without a custodian.** The campaign vault is a program-derived address (PDA). No private key controls it; only the program's logic can move funds.
- **Milestone state machine on-chain.** Each milestone moves through `Pending → ProofSubmitted → Approved → Released`. The program rejects any transition that skips a step, so a payout without proof is impossible by construction.
- **Tamper-evident proof.** The hash of each receipt or photo is stored on-chain. The file itself lives on IPFS/Arweave, and anyone can recompute the hash and compare.
- **Direct-to-vendor payments.** Funds are sent to the vendor's wallet, never through the organizer.
- **Public, auditable ledger.** All state is readable by anyone, so the timeline in the UI is a view of real on-chain data, not a database we control.
- **Fast and cheap.** Low fees and quick finality make small donations (a few dollars) practical, which is what everyday donors actually give.

## Architecture

### Accounts

| Account | Purpose |
|---|---|
| `Campaign` | Organizer, verifier, token mint, target amount, total raised, vault balance, status |
| `Milestone` | Campaign reference, description, amount, vendor wallet, status, proof hash |
| `Donation` | Donor, campaign, amount, timestamp (for the public history) |
| Vault (PDA) | Token account holding the campaign's USDC, owned by the program |

### Instructions

| Instruction | Who can call it | What it does |
|---|---|---|
| `create_campaign` | Organizer | Creates the campaign, its milestones and the vault |
| `donate` | Anyone | Transfers USDC from the donor into the vault and records the donation |
| `submit_proof` | Organizer | Attaches a proof hash to a milestone (`Pending → ProofSubmitted`) |
| `approve_milestone` | Verifier | Approves a milestone with proof (`ProofSubmitted → Approved`) |
| `release_funds` | Anyone | Pays the milestone amount from the vault to the vendor (`Approved → Released`) |

### Failure handling

- `release_funds` fails unless the milestone is `Approved`.
- `submit_proof` and `approve_milestone` can only be called by the organizer and the verifier respectively.
- A milestone can be released only once, and the amount can never exceed the vault balance.

## Tech stack

- **Program:** Rust, [Anchor](https://www.anchor-lang.com/)
- **Token:** USDC (SPL Token) on devnet
- **Frontend:** `<Coming Soon>` with Solana wallet adapter (Phantom)
- **Proof storage:** `<Coming Soon>`

## Getting started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Solana CLI](https://solana.com/docs/intro/installation)
- [Anchor](https://www.anchor-lang.com/docs/installation)
- Node.js 18+ and a package manager (npm, pnpm or yarn)
- A Solana wallet such as Phantom, set to **devnet**

### Clone and build

```bash
git clone https://github.com/webstriiix/bening.git
cd bening
anchor build
```

### Run the tests

```bash
anchor test
```

This starts a local validator, deploys the program and runs the test suite, including the failure case where a release without proof is rejected.

### Deploy to devnet

```bash
solana config set --url devnet
solana airdrop 2          # or use https://faucet.solana.com if rate-limited
anchor build
anchor deploy
```

Copy the deployed program ID into `Anchor.toml` and the frontend config.

### Run the frontend

```bash
cd app
npm install
npm run dev
```

Open `http://localhost:3000`, connect your wallet (devnet) and you are ready to go.

## Demo walkthrough

1. Connect a wallet as the **organizer** and create a campaign with two milestones (for example, "Buy 100 kg of rice" and "Buy 20 sacks of cement").
2. Switch to a **donor** wallet and donate devnet USDC. Watch the vault balance and the public timeline update.
3. As the organizer, try to release funds for a milestone. **The transaction is rejected** because no proof exists.
4. Upload a receipt, submit its hash with `submit_proof`, then approve it as the **verifier**.
5. Call `release_funds`. The vendor's wallet receives the payment, and the timeline shows donation, proof, approval and payout in order.

## Project status

- [ ] Anchor program: campaign, milestones, donation, vault
- [ ] Proof submission and verifier approval
- [ ] Fund release to vendor wallets
- [ ] Tests (happy path and rejected release)
- [ ] Devnet deployment
- [ ] Frontend: create campaign, donate, public timeline
- [ ] Proof upload to decentralized storage
- [ ] Demo video

## Roadmap

- **Cross-chain donations with Aurora Intents.** Give each campaign a persistent deposit address that accepts funds from many chains and settles them into USDC on Solana, so donors do not need to bridge manually.
- **Donor voting.** Let donors take part in approving milestones, in addition to a single verifier.
- **Campaign expiry and refunds.** Return unspent funds to donors if a campaign fails to document its milestones in time.
- **Verifier reputation.** Track the history of organizers and verifiers on-chain.

## License

MIT
