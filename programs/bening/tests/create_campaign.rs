use {
    anchor_lang::{
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
            state::{Account as TokenAccount, Mint},
        },
    },
    bening::{
        constants::TOKEN_DECIMALS,
        state::{Campaign, CampaignStatus, MilestoneInput},
    },
    litesvm::{types::FailedTransactionMetadata, LiteSVM},
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

const CAMPAIGN_ID: u64 = 42;
const TITLE: &str = "Rice Distribution Campaign";
const METADATA_URI: &str = "https://example.com/campaign-metadata";
const MILESTONE_COUNT: usize = 3;

fn setup_svm() -> (LiteSVM, Keypair, Keypair, Keypair, Keypair, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    let _ = svm.add_program_from_file(
        bening::ID,
        "../../target/deploy/bening.so",
    );

    let organizer = Keypair::new();
    let verifier = Keypair::new();
    let vendor1 = Keypair::new();
    let vendor2 = Keypair::new();
    let vendor3 = Keypair::new();
    let mint = Keypair::new();

    // Airdrop SOL to organizer
    svm.airdrop(&organizer.pubkey(), 10_000_000_000).unwrap();

    // Create mint (original SPL Token program)
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
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &mint])
        .unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "mint init failed: {:?}", res);

    (svm, organizer, verifier, vendor1, vendor2, vendor3, mint)
}

fn build_create_campaign_ix(
    program_id: Pubkey,
    organizer: Pubkey,
    campaign_id: u64,
    title: String,
    metadata_uri: String,
    verifier: Pubkey,
    milestones: Vec<MilestoneInput>,
    mint: Pubkey,
    vault: Pubkey,
    campaign: Pubkey,
) -> Instruction {
    let accounts = bening::accounts::CreateCampaign {
        organizer,
        campaign,
        vault,
        mint,
        system_program: system_program::ID,
        token_program: spl_token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        rent: anchor_lang::solana_program::rent::id(),
    };

    let data = bening::instruction::CreateCampaign {
        campaign_id,
        title,
        metadata_uri,
        verifier,
        milestones,
    }
    .data();

    Instruction::new_with_bytes(program_id, &data, accounts.to_account_metas(None))
}

fn send_custom_campaign(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: Pubkey,
    mint_pubkey: Pubkey,
    title: &str,
    metadata_uri: &str,
    campaign_id: u64,
    milestones: Vec<MilestoneInput>,
) -> Result<(Pubkey, Pubkey), FailedTransactionMetadata> {
    let organizer_pubkey = organizer.pubkey();

    // Derive campaign PDA
    let (campaign_pda, _bump) = Pubkey::find_program_address(
        &[
            b"campaign",
            organizer_pubkey.as_ref(),
            &campaign_id.to_le_bytes(),
        ],
        &bening::ID,
    );

    // Derive vault ATA (associated token account of campaign PDA for the mint)
    let vault_pda = get_associated_token_address(&campaign_pda, &mint_pubkey);

    let ix = build_create_campaign_ix(
        bening::ID,
        organizer_pubkey,
        campaign_id,
        title.to_string(),
        metadata_uri.to_string(),
        verifier,
        milestones,
        mint_pubkey,
        vault_pda,
        campaign_pda,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer_pubkey), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[organizer]).unwrap();

    svm.send_transaction(tx).map(|_| (campaign_pda, vault_pda))
}

fn send_create_campaign(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: Pubkey,
    vendors: &[Pubkey],
    mint_pubkey: Pubkey,
    title: &str,
    metadata_uri: &str,
    campaign_id: u64,
    milestone_count: usize,
) -> Result<(Pubkey, Pubkey), FailedTransactionMetadata> {
    let mut milestones = Vec::with_capacity(milestone_count);
    let vendors_mod = vendors.to_vec();
    for i in 0..milestone_count {
        let vendor = if i < vendors_mod.len() {
            vendors_mod[i]
        } else {
            Keypair::new().pubkey()
        };
        milestones.push(MilestoneInput {
            label: format!("Milestone {}", i + 1),
            amount: 1_000_000 + (i as u64 * 100_000),
            vendor,
        });
    }

    send_custom_campaign(
        svm,
        organizer,
        verifier,
        mint_pubkey,
        title,
        metadata_uri,
        campaign_id,
        milestones,
    )
}

#[test]
fn test_create_campaign_happy_path() {
    let (mut svm, organizer, verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_ok(), "create_campaign failed: {:?}", res);

    let (campaign_pda, vault_pda) = res.unwrap();

    let campaign_account = svm.get_account(&campaign_pda).expect("campaign account not found");
    let campaign = Campaign::try_deserialize(&mut campaign_account.data.as_slice())
        .expect("failed to deserialize campaign");

    assert_eq!(campaign.organizer, organizer.pubkey());
    assert_eq!(campaign.verifier, verifier.pubkey());
    assert_eq!(campaign.mint, mint.pubkey());
    assert_eq!(campaign.campaign_id, CAMPAIGN_ID);
    assert_eq!(campaign.title, TITLE);
    assert_eq!(campaign.metadata_uri, METADATA_URI);
    assert_eq!(campaign.target_amount, 3_300_000);
    assert_eq!(campaign.total_raised, 0);
    assert_eq!(campaign.total_released, 0);
    assert_eq!(campaign.donation_count, 0);
    assert_eq!(campaign.status, CampaignStatus::Active);
    assert_eq!(campaign.milestones.len(), MILESTONE_COUNT);

    let vault_account = svm.get_account(&vault_pda).expect("vault account not found");
    let mut vault_data_slice = vault_account.data.as_slice();
    let vault_state = TokenAccount::unpack(&mut vault_data_slice).expect("unpack vault failed");
    assert_eq!(vault_state.mint, mint.pubkey());
    assert_eq!(vault_state.owner, campaign_pda);
    assert_eq!(vault_state.amount, 0);
}

#[test]
fn test_create_campaign_zero_milestones() {
    let (mut svm, organizer, verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        0,
    );
    assert!(res.is_err(), "expected error for zero milestones");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Milestone count must be between 1 and MAX_MILESTONES")
            || log.contains("InvalidMilestoneCount"),
        "expected InvalidMilestoneCount error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_too_many_milestones() {
    let (mut svm, organizer, verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        6,
    );
    assert!(res.is_err(), "expected error for >5 milestones");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Maximum milestones exceeded") || log.contains("MaxMilestonesExceeded"),
        "expected MaxMilestonesExceeded error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_verifier_equals_organizer() {
    let (mut svm, organizer, _verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        organizer.pubkey(),
        &[vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_err(), "expected error for verifier == organizer");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Verifier cannot be organizer or default pubkey")
            || log.contains("InvalidVerifier"),
        "expected InvalidVerifier error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_verifier_equals_default_pubkey() {
    let (mut svm, organizer, _verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        Pubkey::default(),
        &[vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_err(), "expected error for verifier == default pubkey");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Verifier cannot be organizer or default pubkey")
            || log.contains("InvalidVerifier"),
        "expected InvalidVerifier error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_vendor_equals_organizer() {
    let (mut svm, organizer, verifier, _vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[organizer.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_err(), "expected error for vendor == organizer");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Milestone vendor cannot be organizer, verifier, or default pubkey")
            || log.contains("InvalidVendor"),
        "expected InvalidVendor error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_vendor_equals_verifier() {
    let (mut svm, organizer, verifier, _vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[verifier.pubkey(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_err(), "expected error for vendor == verifier");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Milestone vendor cannot be organizer, verifier, or default pubkey")
            || log.contains("InvalidVendor"),
        "expected InvalidVendor error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_vendor_equals_default_pubkey() {
    let (mut svm, organizer, verifier, _vendor1, vendor2, vendor3, mint) = setup_svm();

    let res = send_create_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        &[Pubkey::default(), vendor2.pubkey(), vendor3.pubkey()],
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        MILESTONE_COUNT,
    );
    assert!(res.is_err(), "expected error for vendor == default pubkey");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Milestone vendor cannot be organizer, verifier, or default pubkey")
            || log.contains("InvalidVendor"),
        "expected InvalidVendor error, got: {}",
        log
    );
}

// Test Group 2 tests
#[test]
fn test_create_campaign_zero_amount() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: 0, // Zero amount
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for milestone amount == 0");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Milestone amount must be > 0") || log.contains("InvalidAmount"),
        "expected InvalidAmount error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_empty_title() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        "", // Empty title
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for empty title");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Title length must be between 1 and MAX_TITLE_LEN")
            || log.contains("InvalidTitleLength"),
        "expected InvalidTitleLength error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_title_too_long() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let long_title = "A".repeat(49); // 49 bytes (MAX_TITLE_LEN = 48)

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        &long_title,
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for 49-byte title");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Title length must be between 1 and MAX_TITLE_LEN")
            || log.contains("InvalidTitleLength"),
        "expected InvalidTitleLength error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_label_too_long() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let long_label = "A".repeat(33); // 33 bytes (MAX_LABEL_LEN = 32)
    let milestones = vec![MilestoneInput {
        label: long_label,
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for 33-byte label");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Label length must be <= MAX_LABEL_LEN") || log.contains("InvalidLabelLength"),
        "expected InvalidLabelLength error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_uri_too_long() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let long_uri = "https://example.com/".to_string() + &"A".repeat(129); // > 128 bytes (MAX_URI_LEN = 128)

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        TITLE,
        &long_uri,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for 129+-byte metadata_uri");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Metadata URI length must be <= MAX_URI_LEN")
            || log.contains("InvalidMetadataUriLength"),
        "expected InvalidMetadataUriLength error, got: {}",
        log
    );
}

#[test]
fn test_create_campaign_boundaries_success() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let boundary_title = "A".repeat(48); // Exactly 48 bytes
    let boundary_label = "B".repeat(32); // Exactly 32 bytes
    let boundary_uri = "C".repeat(128); // Exactly 128 bytes

    let milestones = vec![MilestoneInput {
        label: boundary_label,
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        &boundary_title,
        &boundary_uri,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(
        res.is_ok(),
        "expected success for exact boundary lengths (48 title, 32 label, 128 uri), got: {:?}",
        res
    );
}

// Test Group 3 tests
#[test]
fn test_create_campaign_milestone_amount_sum_overflow() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: u64::MAX, // Will overflow on addition
        vendor: vendor1.pubkey(),
    }, MilestoneInput {
        label: "Milestone 2".to_string(),
        amount: 1,
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );
    assert!(res.is_err(), "expected error for amount sum overflow");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("Target amount calculation overflow")
            || log.contains("TargetAmountOverflow"),
        "expected TargetAmountOverflow error, got: {}",
        log
    );
}

// Test Group 4 tests
#[test]
fn test_create_campaign_vault_pre_created() {
    let (mut svm, organizer, verifier, vendor1, _vendor2, _vendor3, mint) = setup_svm();

    // 1. Derive campaign PDA and vault ATA
    let (campaign_pda, _bump) = Pubkey::find_program_address(
        &[
            b"campaign",
            organizer.pubkey().as_ref(),
            &CAMPAIGN_ID.to_le_bytes(),
        ],
        &bening::ID,
    );
    let vault_pda = get_associated_token_address(&campaign_pda, &mint.pubkey());

    // 2. Pre-create the vault ATA using spl_associated_token_account instruction via cross-program invocation or direct instruction
    let create_ata_ix = Instruction {
        program_id: anchor_spl::associated_token::ID,
        accounts: vec![
            anchor_lang::solana_program::instruction::AccountMeta::new(organizer.pubkey(), true),
            anchor_lang::solana_program::instruction::AccountMeta::new(vault_pda, false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(campaign_pda, false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(mint.pubkey(), false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(system_program::ID, false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(spl_token::ID, false),
        ],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[create_ata_ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();
    svm.send_transaction(tx).expect("pre-creation failed");

    // 3. Attempt to create campaign
    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: 1_000_000,
        vendor: vendor1.pubkey(),
    }];

    let res = send_custom_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        mint.pubkey(),
        TITLE,
        METADATA_URI,
        CAMPAIGN_ID,
        milestones,
    );

    // 4. Assert failure
    assert!(res.is_err(), "expected error for pre-created vault ATA");
    let err = res.unwrap_err();
    let log = err.meta.logs.join("\n");
    assert!(
        log.contains("already in use") || 
        log.contains("Account already in use") || 
        log.contains("already initialized") ||
        log.contains("Error processing Instruction 0: custom program error: 0x0") ||
        matches!(err.err, solana_transaction::TransactionError::InstructionError(_, _)),
        "expected error for pre-created vault, got: {:?} logs: {}",
        err, log
    );
}

// Test Group 5: Worst-Case Transaction Size Measurement
#[test]
fn test_create_campaign_worst_case_size() {
    let (mut svm, organizer, verifier, vendor1, vendor2, vendor3, mint) = setup_svm();

    // Build worst-case campaign: 5 milestones, all at max length
    let mut milestones = Vec::with_capacity(5);
    for i in 0..5 {
        let label = "X".repeat(32);
        let vendor = if i < 3 { vec![vendor1.pubkey(), vendor2.pubkey(), vendor3.pubkey()][i] } else { Keypair::new().pubkey() };
        milestones.push(MilestoneInput {
            label,
            amount: 1_000_000 + (i as u64 * 100_000),
            vendor,
        });
    }

    let title = "T".repeat(48);
    let metadata_uri = "U".repeat(128);
    let campaign_id: u64 = 999;

    let (campaign_pda, _bump) = Pubkey::find_program_address(
        &[
            b"campaign",
            organizer.pubkey().as_ref(),
            &campaign_id.to_le_bytes(),
        ],
        &bening::ID,
    );
    let vault_pda = get_associated_token_address(&campaign_pda, &mint.pubkey());

    let ix = build_create_campaign_ix(
        bening::ID,
        organizer.pubkey(),
        campaign_id,
        title,
        metadata_uri,
        verifier.pubkey(),
        milestones.clone(),
        mint.pubkey(),
        vault_pda,
        campaign_pda,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&organizer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer]).unwrap();

    // Serialize the transaction to measure sizes (solana_sdk / solana_transaction serialization)
    let tx_data = bincode::serialize(&tx).unwrap();
    let tx_size = tx_data.len();

    // Instruction data size: discriminator + args
    let instr_data = bening::instruction::CreateCampaign {
        campaign_id,
        title: "T".repeat(48),
        metadata_uri: "U".repeat(128),
        verifier: verifier.pubkey(),
        milestones: milestones.clone(),
    }
    .data();
    let instr_data_size = instr_data.len();

    // Campaign account data length
    let res = svm.send_transaction(tx.clone());
    assert!(res.is_ok(), "Transaction failed during size measurement: {:?}", res);
    let campaign_account = svm.get_account(&campaign_pda).unwrap();
    let campaign_data_len = campaign_account.data.len();

    println!("Transaction size: {} bytes", tx_size);
    println!("Instruction data size: {} bytes", instr_data_size);
    println!("Campaign account data length: {} bytes", campaign_data_len);

    // Assert transaction size and instruction data size are <= Solana transaction limit (1232 bytes)
    assert!(
        tx_size <= 1232,
        "Transaction size {} exceeds Solana limit (1232 bytes)",
        tx_size
    );
    assert!(
        instr_data_size <= 1232,
        "Instruction data size {} exceeds limit",
        instr_data_size,
    );
    // Note: Campaign account storage (1,655 bytes estimate) exceeds transaction limit (1232 bytes),
    // which is standard for Solana accounts allocated via system program (accounts are not limited to 1232 bytes;
    // only transactions are constrained to 1232 bytes). We assert campaign data length equals our expected worst-case estimate (1655 bytes).
    assert_eq!(
        campaign_data_len, 1655,
        "Campaign account data length should match worst-case estimate of 1655 bytes"
    );
}
