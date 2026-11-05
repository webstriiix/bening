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
        state::{Campaign, MilestoneInput},
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

fn setup_svm() -> (LiteSVM, Keypair, Keypair, Keypair, Keypair, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(bening::ID, "../../target/deploy/bening.so")
        .expect(
            "failed to load ../../target/deploy/bening.so; run cargo build-sbf --arch v0 first",
        );

    let organizer = Keypair::new();
    let verifier = Keypair::new();
    let vendor = Keypair::new();
    let donor = Keypair::new();
    let mint = Keypair::new();

    svm.airdrop(&organizer.pubkey(), 10_000_000_000).unwrap();
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
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&organizer, &mint]).unwrap();
    svm.send_transaction(tx).unwrap();

    (svm, organizer, verifier, vendor, donor, mint)
}

fn create_test_campaign(
    svm: &mut LiteSVM,
    organizer: &Keypair,
    verifier: Pubkey,
    vendor: Pubkey,
    mint: Pubkey,
    milestone_amount: u64,
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

    let milestones = vec![MilestoneInput {
        label: "Milestone 1".to_string(),
        amount: milestone_amount,
        vendor,
    }];

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

/// N6: donate with wrong mint fails with BeningError::InvalidMint.
#[test]
fn n6_donate_wrong_mint_fails_invalid_mint() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        1_000_000,
    );

    let wrong_mint = Keypair::new();
    let mint_size = Mint::LEN;
    let mint_rent = svm.minimum_balance_for_rent_exemption(mint_size);
    let create_mint_ix = system_instruction::create_account(
        &donor.pubkey(),
        &wrong_mint.pubkey(),
        mint_rent,
        mint_size as u64,
        &spl_token::ID,
    );
    let init_mint_ix = initialize_mint2(
        &spl_token::ID,
        &wrong_mint.pubkey(),
        &donor.pubkey(),
        None,
        TOKEN_DECIMALS,
    )
    .unwrap();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_mint_ix, init_mint_ix],
        Some(&donor.pubkey()),
        &blockhash,
    );
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&donor, &wrong_mint])
        .unwrap();
    svm.send_transaction(tx).unwrap();

    let wrong_donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), wrong_mint.pubkey());
    mint_tokens_to(
        &mut svm,
        &donor,
        wrong_mint.pubkey(),
        wrong_donor_ata,
        1_000_000,
    );

    let res = send_donate(
        &mut svm,
        &donor,
        campaign_pda,
        wrong_donor_ata,
        vault_pda,
        wrong_mint.pubkey(),
        500_000,
    );

    let err = res.expect_err("donate with wrong mint must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(2009)
        )
    );
}

/// N8: donate zero amount fails with BeningError::InvalidAmount.
#[test]
fn n8_donate_zero_amount_fails_invalid_amount() {
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

    let res = send_donate(
        &mut svm,
        &donor,
        campaign_pda,
        donor_ata,
        vault_pda,
        mint.pubkey(),
        0,
    );

    let err = res.expect_err("donate(0) must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(custom_code(BeningError::InvalidAmount))
        )
    );
}

/// N9: donate beyond the target fails with BeningError::DonationOverflow.
#[test]
fn n9_donate_beyond_target_fails_overflow() {
    let (mut svm, organizer, verifier, vendor, donor, mint) = setup_svm();
    let target = 1_000_000;
    let (campaign_pda, vault_pda) = create_test_campaign(
        &mut svm,
        &organizer,
        verifier.pubkey(),
        vendor.pubkey(),
        mint.pubkey(),
        target,
    );
    let donor_ata = create_donor_ata(&mut svm, &donor, donor.pubkey(), mint.pubkey());
    mint_tokens_to(&mut svm, &organizer, mint.pubkey(), donor_ata, 2_000_000);

    let res = send_donate(
        &mut svm,
        &donor,
        campaign_pda,
        donor_ata,
        vault_pda,
        mint.pubkey(),
        target + 1,
    );

    let err = res.expect_err("donate beyond target must fail");
    assert_eq!(
        err.err,
        TransactionError::InstructionError(
            0,
            InstructionError::Custom(6011)
        )
    );
}
