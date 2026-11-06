use {
    anchor_lang::{
        error::ERROR_CODE_OFFSET,
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    anchor_spl::{
        associated_token::get_associated_token_address,
        token::spl_token::{
            self,
            instruction::{initialize_mint2, mint_to},
            state::Mint,
        },
    },
    bening::{
        constants::TOKEN_DECIMALS,
        error::BeningError,
        state::{Campaign, CampaignStatus, MilestoneInput, MilestoneStatus},
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_pack::Pack,
    anchor_lang::solana_program::clock::Clock,
    solana_signer::Signer,
    solana_transaction::{versioned::VersionedTransaction, InstructionError, TransactionError},
};

const CAMPAIGN_ID: u64 = 42;
const TITLE: &str = "Rice Distribution Campaign";
const METADATA_URI: &str = "https://example.com/campaign-metadata";

fn custom_code(e: BeningError) -> u32 {
    ERROR_CODE_OFFSET + e as u32
}

fn setup_svm() -> (LiteSVM, Keypair, Keypair, Keypair, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(bening::ID, "../../target/deploy/bening.so")
        .expect("load bening.so");

    // Set LiteSVM Clock sysvar to 1700000000 (installed litesvm 0.10.0 src/lib.rs:751)
    let mut clock = Clock::default();
    clock.unix_timestamp = 1_700_000_000;
    svm.set_sysvar::<Clock>(&clock);

    let organizer = Keypair::new();
    let verifier = Keypair::new();
    let vendor = Keypair::new();
    let donor = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(&organizer.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&verifier.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&donor.pubkey(), 10_000_000_000).unwrap();

    let mint_size = Mint::LEN;
    let mint_rent = svm.minimum_balance_for_rent_exemption(mint_size);

    let create_mint_ix = system_instruction::create_account(
        &organizer.pubkey(),
        &mint.pubkey(),
        mint_rent,
        mint_size as u64,
        &spl_token::ID,
    );
    let init_mint_ix = initialize_mint2(
        &spl_token::ID,
        &mint.pubkey(),
        &organizer.pubkey(),
        None,
        TOKEN_DECIMALS,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_mint_ix, init_mint_ix],
        Some(&organizer.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &mint]).unwrap();
    svm.send_transaction(tx).unwrap();

    (svm, organizer, verifier, vendor, donor, mint)
}

fn advance_clock(svm: &mut LiteSVM, seconds: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp += seconds;
    svm.set_sysvar::<Clock>(&clock);
}

fn create_test_campaign(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: Pubkey,
    vendor: Pubkey,
    mint: Pubkey,
    milestone_amount: u64,
) -> (Pubkey, Pubkey) {
    create_multi_milestone_campaign(
        svm,
        organizer,
        verifier,
        vec![MilestoneInput {
            label: "Milestone 1".to_string(),
            amount: milestone_amount,
            vendor,
        }],
        mint,
    )
}

fn create_multi_milestone_campaign(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: Pubkey,
    milestones: Vec<MilestoneInput>,
    mint: Pubkey,
) -> (Pubkey, Pubkey) {
    let organizer_pubkey = organizer.pubkey();
    let (campaign_pda, _bump) = Pubkey::find_program_address(
        &[
            b"campaign",
            organizer_pubkey.as_ref(),
            &CAMPAIGN_ID.to_le_bytes(),
        ],
        &bening::ID,
    );
    let vault_pda = get_associated_token_address(&campaign_pda, &mint);

    let accounts = bening::accounts::CreateCampaign {
        organizer: organizer_pubkey,
        campaign: campaign_pda,
        vault: vault_pda,
        mint,
        system_program: system_program::ID,
        token_program: spl_token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        rent: anchor_lang::solana_program::rent::id(),
    };

    let data = bening::instruction::CreateCampaign {
        campaign_id: CAMPAIGN_ID,
        title: TITLE.to_string(),
        metadata_uri: METADATA_URI.to_string(),
        verifier,
        milestones,
    }
    .data();

    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer_pubkey), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    (campaign_pda, vault_pda)
}

fn create_donor_ata(svm: &mut LiteSVM, payer: &Keypair, owner: Pubkey, mint: Pubkey) -> Pubkey {
    let ata = get_associated_token_address(&owner, &mint);
    let ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        &owner,
        &mint,
        &spl_token::ID,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap();
    ata
}

fn mint_tokens_to(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: Pubkey,
    destination: Pubkey,
    amount: u64,
) {
    let ix = mint_to(
        &spl_token::ID,
        &mint,
        &destination,
        &payer.pubkey(),
        &[&payer.pubkey()],
        amount,
    )
    .unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[payer]).unwrap();
    svm.send_transaction(tx).unwrap();
}

fn send_donate(
    svm: &mut LiteSVM,
    donor: &Keypair,
    campaign_pda: Pubkey,
    donor_ata: Pubkey,
    vault_pda: Pubkey,
    mint: Pubkey,
    amount: u64,
) -> Result<(), FailedTransactionMetadata> {
    let donor_pubkey = donor.pubkey();

    let campaign_account = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_account.data.as_slice()).unwrap();

    let (donation_pda, _bump) = Pubkey::find_program_address(
        &[
            b"donation",
            campaign_pda.as_ref(),
            &campaign.donation_count.to_le_bytes(),
        ],
        &bening::ID,
    );

    let accounts = bening::accounts::Donate {
        donor: donor_pubkey,
        campaign: campaign_pda,
        donation: donation_pda,
        donor_ata,
        vault: vault_pda,
        mint,
        system_program: system_program::ID,
        token_program: spl_token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        clock: anchor_lang::prelude::pubkey!("SysvarC1ock11111111111111111111111111111111"),
    };

    let data = bening::instruction::Donate { amount }.data();
    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&donor_pubkey), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[donor]).unwrap();

    svm.send_transaction(tx).map(|_| ())
}

fn submit_and_approve(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: &Keypair,
    campaign_pda: Pubkey,
    milestone_index: u8,
) {
    advance_clock(svm, 10);
    let submit_accounts = bening::accounts::SubmitProof {
        organizer: organizer.pubkey(),
        campaign: campaign_pda,
    };
    let submit_data = bening::instruction::SubmitProof {
        milestone_index,
        proof_hash: [1u8; 32],
        proof_uri: "https://example.com/proof".to_string(),
    }
    .data();
    let ix = Instruction::new_with_bytes(bening::ID, &submit_data, submit_accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    advance_clock(svm, 10);
    let approve_accounts = bening::accounts::ApproveMilestone {
        verifier: verifier.pubkey(),
        campaign: campaign_pda,
    };
    let approve_data = bening::instruction::ApproveMilestone { milestone_index }.data();
    let ix2 = Instruction::new_with_bytes(bening::ID, &approve_data, approve_accounts.to_account_metas(None));
    let msg2 = Message::new_with_blockhash(&[ix2], Some(&verifier.pubkey()), &blockhash);
    let tx2 = VersionedTransaction::try_new(VersionedMessage::Legacy(msg2), &[verifier]).unwrap();
    svm.send_transaction(tx2).unwrap();
}

fn send_release(
    svm: &mut LiteSVM,
    caller: &Keypair,
    campaign_pda: Pubkey,
    mint: Pubkey,
    vault_pda: Pubkey,
    vendor_ata: Pubkey,
    milestone_index: u8,
) -> Result<(), FailedTransactionMetadata> {
    advance_clock(svm, 10);
    let accounts = bening::accounts::ReleaseFunds {
        caller: caller.pubkey(),
        campaign: campaign_pda,
        mint,
        vault: vault_pda,
        vendor_token_account: vendor_ata,
        token_program: spl_token::ID,
    };

    let data = bening::instruction::ReleaseFunds { milestone_index }.data();
    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&caller.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[caller]).unwrap();
    svm.send_transaction(tx).map(|_| ())
}

/// N1: release when milestone status is Pending fails with InvalidStatus.
#[test]
fn n1_release_when_pending_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0);
    let err = res.expect_err("release Pending milestone must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// N2: release when milestone status is ProofSubmitted (not approved) fails with InvalidStatus.
#[test]
fn n2_release_when_proof_submitted_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    advance_clock(&mut svm, 10);
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
    let ix = Instruction::new_with_bytes(bening::ID, &submit_data, submit_accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0);
    let err = res.expect_err("release ProofSubmitted milestone must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// N5: release the same milestone twice fails with InvalidStatus.
#[test]
fn n5_release_twice_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0).unwrap();
    svm.expire_blockhash();

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0);
    let err = res.expect_err("second release must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidStatus))
        )
    );
}

/// N7: release to a token account not owned by the milestone vendor fails with InvalidVendorAccount.
#[test]
fn n7_release_wrong_vendor_ata_owner_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let wrong_owner = Keypair::new();
    let wrong_vendor_ata = create_donor_ata(&mut svm, &organizer, wrong_owner.pubkey(), mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, wrong_vendor_ata, 0);
    let err = res.expect_err("release to wrong vendor owner must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidVendorAccount))
        )
    );
}

/// a: release by a random third-party keypair succeeds; vendor +amount, vault -amount, status Released, released_at set, total_released updated.
#[test]
fn a_release_by_random_third_party_succeeds() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let vault_before_acct = svm.get_account(&vault_pda).unwrap();
    let vault_before_amount = spl_token::state::Account::unpack(&vault_before_acct.data).unwrap().amount;

    let vendor_before_acct = svm.get_account(&vendor_ata).unwrap();
    let vendor_before_amount = spl_token::state::Account::unpack(&vendor_before_acct.data).unwrap().amount;

    let third_party = Keypair::new();
    svm.airdrop(&third_party.pubkey(), 10_000_000_000).unwrap();
    send_release(&mut svm, &third_party, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.milestones[0].status, MilestoneStatus::Released);
    assert!(campaign.milestones[0].proof_submitted_at < campaign.milestones[0].approved_at);
    assert!(campaign.milestones[0].approved_at < campaign.milestones[0].released_at);
    assert_eq!(campaign.total_released, 1_000_000);

    let vault_after_acct = svm.get_account(&vault_pda).unwrap();
    let vault_after_amount = spl_token::state::Account::unpack(&vault_after_acct.data).unwrap().amount;
    assert_eq!(vault_after_amount, vault_before_amount - 1_000_000);

    let vendor_after_acct = svm.get_account(&vendor_ata).unwrap();
    let vendor_after_amount = spl_token::state::Account::unpack(&vendor_after_acct.data).unwrap().amount;
    assert_eq!(vendor_after_amount, vendor_before_amount + 1_000_000);
}

/// b: milestone index out of range fails with InvalidMilestoneIndex.
#[test]
fn b_milestone_index_out_of_range_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 5);
    let err = res.expect_err("release with out-of-range index must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidMilestoneIndex))
        )
    );
}

/// c: vault underfunded fails with InsufficientVaultBalance.
#[test]
fn c_vault_underfunded_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 500_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 500_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, 0);
    let err = res.expect_err("release with underfunded vault must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InsufficientVaultBalance))
        )
    );
}

/// d: vendor token account with wrong mint fails with InvalidVendorAccount.
#[test]
fn d_vendor_token_wrong_mint_fails() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let wrong_mint = Keypair::new();
    let wrong_mint_size = Mint::LEN;
    let wrong_mint_rent = svm.minimum_balance_for_rent_exemption(wrong_mint_size);
    let create_wrong_mint_ix = system_instruction::create_account(
        &organizer.pubkey(),
        &wrong_mint.pubkey(),
        wrong_mint_rent,
        wrong_mint_size as u64,
        &spl_token::ID,
    );
    let init_wrong_mint_ix = initialize_mint2(
        &spl_token::ID,
        &wrong_mint.pubkey(),
        &organizer.pubkey(),
        None,
        TOKEN_DECIMALS,
    )
    .unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_wrong_mint_ix, init_wrong_mint_ix],
        Some(&organizer.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &wrong_mint]).unwrap();
    svm.send_transaction(tx).unwrap();

    let wrong_vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), wrong_mint.pubkey());

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, wrong_vendor_ata, 0);
    let err = res.expect_err("release with vendor ATA wrong mint must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidVendorAccount))
        )
    );
}

/// e: a second token account owned by the campaign PDA (same mint, funded), but not the official vault ATA, passed as vault.
/// Anchor error code 2015: ConstraintTokenOwner (anchor-lang-error-1.2.1/src/lib.rs:132).
#[test]
fn e_second_vault_account_rejected() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let fake_vault = Keypair::new();
    let token_account_len = spl_token::state::Account::LEN;
    let rent = svm.minimum_balance_for_rent_exemption(token_account_len);
    let create_account_ix = system_instruction::create_account(
        &organizer.pubkey(),
        &fake_vault.pubkey(),
        rent,
        token_account_len as u64,
        &spl_token::ID,
    );
    let init_account_ix = spl_token::instruction::initialize_account(
        &spl_token::ID,
        &fake_vault.pubkey(),
        &mint.pubkey(),
        &campaign_pda,
    ).unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_account_ix, init_account_ix],
        Some(&organizer.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &fake_vault]).unwrap();
    svm.send_transaction(tx).unwrap();

    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), fake_vault.pubkey(), 1_000_000);

    let res = send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), fake_vault.pubkey(), vendor_ata, 0);
    let err = res.expect_err("second vault account must be rejected");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(2009) // ConstraintTokenOwner (anchor-lang-error-1.2.1/src/lib.rs:132)
        )
    );
}

/// f: wrong token program id must be rejected.
/// Anchor error code 3008: AccountProgramNotOwner (anchor-lang-error-1.2.1/src/error.rs:38).
#[test]
fn f_wrong_token_program_id_rejected() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);

    let vendor_ata = create_donor_ata(&mut svm, &organizer, vendor.pubkey(), mint.pubkey());

    let wrong_program_id = Keypair::new().pubkey();

    let accounts = bening::accounts::ReleaseFunds {
        caller: donor.pubkey(),
        campaign: campaign_pda,
        mint: mint.pubkey(),
        vault: vault_pda,
        vendor_token_account: vendor_ata,
        token_program: wrong_program_id,
    };

    let data = bening::instruction::ReleaseFunds { milestone_index: 0 }.data();
    let ix = Instruction::new_with_bytes(bening::ID, &data, accounts.to_account_metas(None));
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&donor.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&donor]).unwrap();

    let res = svm.send_transaction(tx);
    let err = res.expect_err("wrong token program must be rejected");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(3008) // AccountProgramNotOwner (anchor-lang-error-1.2.1/src/error.rs:38)
        )
    );
}

/// g: two milestones released in reverse order, both succeed (D-9).
#[test]
fn g_two_milestones_reverse_order_succeed() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let vendor1 = vendor.pubkey();
    let vendor2_kp = Keypair::new();
    let vendor2 = vendor2_kp.pubkey();

    let milestones = vec![
        MilestoneInput { label: "M1".to_string(), amount: 400_000, vendor: vendor1 },
        MilestoneInput { label: "M2".to_string(), amount: 600_000, vendor: vendor2 },
    ];

    let (campaign_pda, vault_pda) = create_multi_milestone_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        milestones,
        mint.pubkey(),
    );

    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 1_000_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 1_000_000).unwrap();

    let v1_ata = create_donor_ata(&mut svm, &organizer, vendor1, mint.pubkey());
    let v2_ata = create_donor_ata(&mut svm, &organizer, vendor2, mint.pubkey());

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 1);
    send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, v2_ata, 1).unwrap();

    submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, 0);
    send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, v1_ata, 0).unwrap();

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.milestones[0].status, MilestoneStatus::Released);
    assert_eq!(campaign.milestones[1].status, MilestoneStatus::Released);
    assert_eq!(campaign.total_released, 1_000_000);
    assert_eq!(campaign.status, CampaignStatus::Completed);
}

/// TEST-1 (strengthened per PRD line 234):
/// "create a campaign with 3 milestones; two donors donate up to the target;
/// submit proof, approve and release each milestone; assert balances, statuses,
/// timestamps and INV-1 to INV-6."
#[test]
fn test_1_release_funds_happy_path() {
    let PRD_LINE_234: &str = "create a campaign with 3 milestones; two donors donate up to the target; submit proof, approve and release each milestone; assert balances, statuses, timestamps and INV-1 to INV-6.";
    assert!(!PRD_LINE_234.is_empty());

    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let vendor1 = vendor.pubkey();
    let vendor2_kp = Keypair::new();
    let vendor2 = vendor2_kp.pubkey();
    let vendor3_kp = Keypair::new();
    let vendor3 = vendor3_kp.pubkey();
    let donor2 = Keypair::new();

    svm.airdrop(&donor2.pubkey(), 10_000_000_000).unwrap();

    let milestones = vec![
        MilestoneInput { label: "M1".to_string(), amount: 300_000, vendor: vendor1 },
        MilestoneInput { label: "M2".to_string(), amount: 400_000, vendor: vendor2 },
        MilestoneInput { label: "M3".to_string(), amount: 300_000, vendor: vendor3 },
    ];

    let (campaign_pda, vault_pda) = create_multi_milestone_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        milestones,
        mint.pubkey(),
    );

    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    let donor2_ata = create_donor_ata(&mut svm, &donor2, donor2.pubkey(), mint.pubkey());

    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 600_000);
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor2_ata, 400_000);
    send_donate(&mut svm, &donor, campaign_pda, donor_ata, vault_pda, mint.pubkey(), 600_000).unwrap();
    send_donate(&mut svm, &donor2, campaign_pda, donor2_ata, vault_pda, mint.pubkey(), 400_000).unwrap();

    let v1_ata = create_donor_ata(&mut svm, &organizer, vendor1, mint.pubkey());
    let v2_ata = create_donor_ata(&mut svm, &organizer, vendor2, mint.pubkey());
    let v3_ata = create_donor_ata(&mut svm, &organizer, vendor3, mint.pubkey());

    for i in 0..3 {
        submit_and_approve(&mut svm, &organizer, &verifier, campaign_pda, i);
        let vendor_ata = match i {
            0 => v1_ata,
            1 => v2_ata,
            _ => v3_ata,
        };
        send_release(&mut svm, &donor, campaign_pda, mint.pubkey(), vault_pda, vendor_ata, i).unwrap();
    }

    let campaign_acct = svm.get_account(&campaign_pda).unwrap();
    let campaign = Campaign::try_deserialize(&mut campaign_acct.data.as_slice()).unwrap();
    assert_eq!(campaign.status, CampaignStatus::Completed);
    assert_eq!(campaign.total_released, 1_000_000);
    assert_eq!(campaign.total_raised, 1_000_000);

    let vault_after_acct = svm.get_account(&vault_pda).unwrap();
    let vault_after_amount = spl_token::state::Account::unpack(&vault_after_acct.data).unwrap().amount;
    assert_eq!(vault_after_amount, campaign.total_raised - campaign.total_released);

    assert!(campaign.total_released <= campaign.total_raised);
    assert!(campaign.total_raised <= campaign.target_amount);

    let sum: u64 = campaign.milestones.iter().map(|m| m.amount).sum();
    assert_eq!(sum, campaign.target_amount);

    for m in &campaign.milestones {
        assert_eq!(m.status, MilestoneStatus::Released);
        assert!(m.proof_submitted_at < m.approved_at);
        assert!(m.approved_at < m.released_at);
    }

    assert_eq!(vault_after_amount, 0);

    assert_eq!(campaign.milestones[0].label, "M1");
    assert_eq!(campaign.milestones[0].amount, 300_000);
    assert_eq!(campaign.milestones[0].vendor, vendor1);
}
