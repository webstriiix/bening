use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_instruction},
    },
    anchor_spl::token::spl_token::{
        self,
        instruction::{initialize_account3, initialize_mint2, mint_to, transfer},
        state::{Account, Mint},
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_program_pack::Pack,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

#[test]
fn token_smoke() {
    // Verify original SPL Token program (D-2, not Token-2022)
    assert_eq!(
        spl_token::ID.to_string(),
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    );

    let mut svm = LiteSVM::new();
    let payer = Keypair::new();

    // Airdrop SOL
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create mint
    let mint = Keypair::new();
    let mint_size = Mint::LEN;
    let mint_rent = svm.minimum_balance_for_rent_exemption(mint_size);

    let create_mint_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        mint_rent,
        mint_size as u64,
        &spl_token::ID,
    );

    // Initialize mint with 6 decimals
    let init_mint_ix =
        initialize_mint2(&spl_token::ID, &mint.pubkey(), &payer.pubkey(), None, 6).unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(
        &[create_mint_ix, init_mint_ix],
        Some(&payer.pubkey()),
        &blockhash,
    );
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &mint]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "mint init failed: {:?}", res);

    // Create two token accounts
    let ata1 = Pubkey::find_program_address(
        &[
            &payer.pubkey().as_ref(),
            &spl_token::ID.as_ref(),
            &mint.pubkey().as_ref(),
        ],
        &spl_token::ID,
    )
    .0;

    let ata2 = Pubkey::find_program_address(
        &[
            &payer.pubkey().as_ref(),
            &spl_token::ID.as_ref(),
            &mint.pubkey().as_ref(),
        ],
        &spl_token::ID,
    )
    .0;

    // Actually, for token accounts we need proper ATA derivation with mint
    // Let me use a simpler approach - create accounts directly
    let account1 = Keypair::new();
    let account2 = Keypair::new();

    let account_size = Account::LEN;
    let account_rent = svm.minimum_balance_for_rent_exemption(account_size);

    let create_acc1 = system_instruction::create_account(
        &payer.pubkey(),
        &account1.pubkey(),
        account_rent,
        account_size as u64,
        &spl_token::ID,
    );

    let init_acc1 = initialize_account3(
        &spl_token::ID,
        &account1.pubkey(),
        &mint.pubkey(),
        &payer.pubkey(),
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg =
        Message::new_with_blockhash(&[create_acc1, init_acc1], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &account1]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "account1 init failed: {:?}", res);

    // Create second account
    let create_acc2 = system_instruction::create_account(
        &payer.pubkey(),
        &account2.pubkey(),
        account_rent,
        account_size as u64,
        &spl_token::ID,
    );

    let init_acc2 = initialize_account3(
        &spl_token::ID,
        &account2.pubkey(),
        &mint.pubkey(),
        &payer.pubkey(),
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg =
        Message::new_with_blockhash(&[create_acc2, init_acc2], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer, &account2]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "account2 init failed: {:?}", res);

    // Mint tokens to account1
    let mint_ix = mint_to(
        &spl_token::ID,
        &mint.pubkey(),
        &account1.pubkey(),
        &payer.pubkey(),
        &[&payer.pubkey()],
        1_000_000,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[mint_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "mint to failed: {:?}", res);

    // Transfer from account1 to account2
    let transfer_ix = transfer(
        &spl_token::ID,
        &account1.pubkey(),
        &account2.pubkey(),
        &payer.pubkey(),
        &[&payer.pubkey()],
        500_000,
    )
    .unwrap();

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer failed: {:?}", res);

    // Assert destination balance using Pack::unpack (D-15 Step B if needed)
    let account2_data = svm.get_account(&account2.pubkey()).unwrap();
    let mut data_slice = account2_data.data.as_slice();
    let account_state = Account::unpack(&mut data_slice).expect("unpack account2");
    assert!(
        account_state.amount >= 500_000,
        "expected balance >= 500_000, got {}",
        account_state.amount
    );
}
