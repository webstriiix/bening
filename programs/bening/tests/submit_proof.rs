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

fn setup_svm() -> (LiteSVM, Keypair, Keypair, Keypair, Keypair, Pubkey) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(bening::ID, "../../target/deploy/bening.so")
        .expect("load bening.so");

    let organizer = Keypair::new();
    let verifier = Keypair::new();
    let vendor1 = Keypair::new();
    let vendor2 = Keypair::new();
    let mint = Keypair::new();

    let organizer_pubkey = organizer.pubkey();
    let verifier_pubkey = verifier.pubkey();

    svm.airdrop(&organizer_pubkey, 10_000_000_000).unwrap();

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
        milestones: vec![
            MilestoneInput {
                label: "Milestone 1".to_string(),
                amount: 1_000_000,
                vendor: vendor1.pubkey(),
            },
            MilestoneInput {
                label: "Milestone 2".to_string(),
                amount: 2_000_000,
                vendor: vendor2.pubkey(),
            },
        ],
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer_pubkey), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    (svm, organizer, verifier, vendor1, vendor2, campaign_pda)
}

/// N4: submit proof by non-organizer fails with NotOrganizer.
#[test]
fn n4_submit_proof_by_non_organizer_fails_not_organizer() {
    let (mut svm, _organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();
    let non_organizer = Keypair::new();
    svm.airdrop(&non_organizer.pubkey(), 1_000_000_000).unwrap();

    let accounts = bening::accounts::SubmitProof {
        organizer: non_organizer.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&non_organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&non_organizer]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("non-organizer submit must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::NotOrganizer))
        )
    );
}

/// Submit proof with all-zero proof_hash fails with InvalidProofHash.
#[test]
fn submit_proof_zero_hash_fails() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [0u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("zero proof_hash must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidProofHash))
        )
    );
}

/// Submit proof with index out of range fails with InvalidMilestoneIndex.
#[test]
fn submit_proof_index_out_of_range_fails() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::SubmitProof {
        milestone_index: 5,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();

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

/// Submit proof with proof_uri over MAX_URI_LEN (129 bytes) fails with InvalidUriLength.
#[test]
fn submit_proof_uri_129_bytes_fails() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let long_uri = "https://example.com/".to_string() + &"a".repeat(110); // 130 bytes total
    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: long_uri,
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("uri > 128 bytes must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidUriLength))
        )
    );
}

/// Submit proof with proof_uri of 128 bytes succeeds.
#[test]
fn submit_proof_uri_128_bytes_success() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let exact_uri = "a".repeat(128);
    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: exact_uri.clone(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();

    svm.send_transaction(tx).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.milestones[0].status, MilestoneStatus::ProofSubmitted);
    assert_eq!(campaign.milestones[0].proof_uri, exact_uri);
}

/// Submit when not Pending fails with InvalidStatus.
#[test]
fn submit_proof_when_not_pending_fails() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    // First submit proof for milestone 0 (transitions to ProofSubmitted)
    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    // Try submitting proof again for milestone 0 (status is now ProofSubmitted, not Pending)
    let dummy_ix = system_instruction::transfer(&organizer.pubkey(), &organizer.pubkey(), 1);
    let ix2 = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash2 = svm.latest_blockhash();
    let msg2 = Message::new_with_blockhash(&[ix2, dummy_ix], Some(&organizer.pubkey()), &blockhash2);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[&organizer]).unwrap();

    let res = svm.send_transaction(tx2);
    let err = res.expect_err("submitting when not pending must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// Happy path: Pending -> ProofSubmitted (assert hash, URI, proof_submitted_at)
#[test]
fn submit_proof_happy_path() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    let proof_hash = [42u8; 32];
    let proof_uri = "https://example.com/proof-uri".to_string();
    let data = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash,
        proof_uri: proof_uri.clone(),
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    let m = &campaign.milestones[0];
    assert_eq!(m.status, MilestoneStatus::ProofSubmitted);
    assert_eq!(m.proof_hash, proof_hash);
    assert_eq!(m.proof_uri, proof_uri);
    assert!(m.proof_submitted_at >= 0);
}

/// Two milestones: submit proof for milestone 0, then milestone 1 on same campaign, both succeed
#[test]
fn submit_proof_two_milestones_success() {
    let (mut svm, organizer, _verifier, _vendor1, _vendor2, campaign_pda) = setup_svm();

    let accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };

    // Submit milestone 0
    let data0 = bening::instruction::SubmitProof {
        milestone_index: 0,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/m0".to_string(),
    }
    .data();
    let ix0 = Instruction::new_with_bytes(bening::ID, &data0, accounts.to_account_metas(None));
    let blockhash0 = svm.latest_blockhash();
    let msg0 = Message::new_with_blockhash(&[ix0], Some(&organizer.pubkey()), &blockhash0);
    let tx0 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg0), &[&organizer]).unwrap();
    svm.send_transaction(tx0).unwrap();

    // Submit milestone 1
    let data1 = bening::instruction::SubmitProof {
        milestone_index: 1,
        proof_hash: [2u8; 32],
        proof_uri: "https://example.com/m1".to_string(),
    }
    .data();
    let ix1 = Instruction::new_with_bytes(bening::ID, &data1, accounts.to_account_metas(None));
    let blockhash1 = svm.latest_blockhash();
    let msg1 = Message::new_with_blockhash(&[ix1], Some(&organizer.pubkey()), &blockhash1);
    let tx1 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg1), &[&organizer]).unwrap();
    svm.send_transaction(tx1).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.milestones[0].status, MilestoneStatus::ProofSubmitted);
    assert_eq!(campaign.milestones[1].status, MilestoneStatus::ProofSubmitted);
}