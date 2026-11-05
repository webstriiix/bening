use {
    anchor_lang::{
        error::ERROR_CODE_OFFSET,
        prelude::Pubkey,
        solana_program::{
            instruction::Instruction,
            system_instruction,
            system_program,
        },
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::get_associated_token_address,
        token::spl_token::{
            self,
            instruction::initialize_mint2,
            state::Mint,
        },
    },
    bening::{
        constants::TOKEN_DECIMALS,
        error::BeningError,
        state::{Campaign, MilestoneInput, MilestoneStatus},
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError, TransactionError},
};

const CAMPAIGN_ID: u64 = 42;
const TITLE: &str = "Rice Distribution Campaign";
const METADATA_URI: &str = "https://example.com/campaign-metadata";

fn custom_code(e: BeningError) -> u32 {
    ERROR_CODE_OFFSET + e as u32
}

fn setup_svm() -> (LiteSVM, Keypair, Keypair, Pubkey) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(bening::ID, "../../target/deploy/bening.so")
        .expect("load bening.so");

    let organizer = Keypair::new();
    let verifier = Keypair::new();
    let vendor = Keypair::new();
    let mint = Keypair::new();

    let organizer_pubkey = organizer.pubkey();
    let verifier_pubkey = verifier.pubkey();

    svm.airdrop(&organizer_pubkey, 10_000_000_000).unwrap();
    svm.airdrop(&verifier_pubkey, 10_000_000_000).unwrap();

    let mint_size = Mint::LEN;
    let mint_rent = svm.minimum_balance_for_rent_exemption(mint_size);

    let create_mint_ix = system_instruction::create_account(
        &organizer_pubkey,
        &mint.pubkey(),
        mint_rent,
        mint_size as u64,
        &spl_token::ID,
    );

    let init_mint_ix = initialize_mint2(
        &spl_token::ID,
        &mint.pubkey(),
        &organizer_pubkey,
        None,
        TOKEN_DECIMALS,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_mint_ix, init_mint_ix],
        Some(&organizer_pubkey),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &mint]).unwrap();
    svm.send_transaction(tx).unwrap();

    let (campaign_pda, _bump) = Pubkey::find_program_address(
        &[b"campaign", organizer_pubkey.as_ref(), &CAMPAIGN_ID.to_le_bytes()],
        &bening::ID,
    );
    let vault_pda = get_associated_token_address(&campaign_pda, &mint.pubkey());

    let accounts = bening::accounts::CreateCampaign {
        organizer: organizer_pubkey,
        campaign: campaign_pda,
        vault: vault_pda,
        mint: mint.pubkey(),
        system_program: system_program::ID,
        token_program: spl_token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        rent: anchor_lang::solana_program::rent::id(),
    };

    let data = bening::instruction::CreateCampaign {
        campaign_id: CAMPAIGN_ID,
        title: TITLE.to_string(),
        metadata_uri: METADATA_URI.to_string(),
        verifier: verifier_pubkey,
        milestones: vec![MilestoneInput {
            label: "Milestone 1".to_string(),
            amount: 1_000_000,
            vendor: vendor.pubkey(),
        }],
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer_pubkey), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    (svm, organizer, verifier, campaign_pda)
}

/// N3: approve by non-verifier fails with NotVerifier.
#[test]
fn n3_approve_by_non_verifier_fails_not_verifier() {
    let (mut svm, _organizer, _verifier, campaign_pda) = setup_svm();
    let non_verifier = Keypair::new();
    svm.airdrop(&non_verifier.pubkey(), 1_000_000_000).unwrap();

    let accounts = bening::accounts::ApproveMilestone {
        verifier: non_verifier.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::ApproveMilestone { milestone_index: 0 }.data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&non_verifier.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&non_verifier]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("non-verifier approval must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::NotVerifier))
        )
    );
}

/// Approve when Pending fails with InvalidStatus.
#[test]
fn approve_milestone_when_pending_fails() {
    let (mut svm, _organizer, verifier, campaign_pda) = setup_svm();

    let accounts = bening::accounts::ApproveMilestone {
        verifier: verifier.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::ApproveMilestone { milestone_index: 0 }.data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&verifier.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&verifier]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("approving pending milestone must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// Approve milestone with index out of range fails with InvalidMilestoneIndex.
#[test]
fn approve_milestone_index_out_of_range_fails() {
    let (mut svm, _organizer, verifier, campaign_pda) = setup_svm();

    let accounts = bening::accounts::ApproveMilestone {
        verifier: verifier.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::ApproveMilestone { milestone_index: 5 }.data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&verifier.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&verifier]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("out of range index must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidMilestoneIndex))
        )
    );
}

/// Approve twice (INV-4) — first approve succeeds, second fails with InvalidStatus.
#[test]
fn approve_milestone_twice_fails() {
    let (mut svm, organizer, verifier, campaign_pda) = setup_svm();

    // First submit proof
    let submit_accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };
    let submit_data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();
    let ix_submit = Instruction::new_with_bytes(bening::ID, &submit_data, submit_accounts.to_account_metas(None));
    let blockhash0 = svm.latest_blockhash();
    let msg0 = Message::new_with_blockhash(&[ix_submit], Some(&organizer.pubkey()), &blockhash0);
    let tx0 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg0), &[&organizer]).unwrap();
    svm.send_transaction(tx0).unwrap();

    // First approval (succeeds)
    let accounts = bening::accounts::ApproveMilestone {
        verifier: verifier.pubkey(),
        campaign: campaign_pda,
    };
    let data = bening::instruction::ApproveMilestone { milestone_index: 0 }.data();
    let ix1_inst = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash1 = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1_inst], Some(&verifier.pubkey()), &blockhash1);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&verifier]).unwrap();
    svm.send_transaction(tx1).unwrap();

    // Second approval (fails because status is already Approved, not ProofSubmitted)
    let dummy_ix = system_instruction::transfer(&verifier.pubkey(), &verifier.pubkey(), 1);
    let ix2_inst = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2_inst, dummy_ix], Some(&verifier.pubkey()), &blockhash2);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&verifier]).unwrap();

    let res = svm.send_transaction(tx2);
    let err = res.expect_err("second approval must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// Happy path: ProofSubmitted -> Approved, assert approved_at set.
#[test]
fn approve_milestone_happy_path() {
    let (mut svm, organizer, verifier, campaign_pda) = setup_svm();

    // Submit proof first
    let submit_accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };
    let submit_data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();
    let ix_submit = Instruction::new_with_bytes(bening::ID, &submit_data, submit_accounts.to_account_metas(None));
    let blockhash0 = svm.latest_blockhash();
    let msg0 = Message::new_with_blockhash(&[ix_submit], Some(&organizer.pubkey()), &blockhash0);
    let tx0 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg0), &[&organizer]).unwrap();
    svm.send_transaction(tx0).unwrap();

    // Approve milestone
    let accounts = bening::accounts::ApproveMilestone {
        verifier: verifier.pubkey(),
        campaign: campaign_pda,
    };
    let data = bening::instruction::ApproveMilestone { milestone_index: 0 }.data();
    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash1 = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix], Some(&verifier.pubkey()), &blockhash1);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&verifier]).unwrap();
    svm.send_transaction(tx1).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.milestones[0].status, MilestoneStatus::Approved);
    assert!(campaign.milestones[0].approved_at >= 0);
}