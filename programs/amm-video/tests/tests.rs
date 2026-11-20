use {
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::CreateMint,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_video::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

    // Derive the PDA for the vault associated token account using the config PDA and Mint A
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    )
}

use anchor_lang::{AccountDeserialize, InstructionData};

fn balance(svm: &LiteSVM, key: Pubkey) -> u64 {
    let acc = svm.get_account(&key).unwrap();
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

fn treasury(config: Pubkey, is_x: bool) -> Pubkey {
    let seed: &[u8] = if is_x { b"treasury_x" } else { b"treasury_y" };
    Pubkey::find_program_address(&[seed, config.as_ref()], &amm_video::id()).0
}

#[test]
fn test_initialize() {
    let (mut svm, payer, x, y, config, lp, vx, vy) = setup();
    let mut ix = create_initialise_ix(&mut svm, &payer, x, y, config, lp, vx, vy);
    let data = ix.data.clone();
    ix.data = amm_video::instruction::Initialize { seed: 123, fee: 10_000, authority: Some(payer.pubkey()) }.data();
    assert!(send(&mut svm, &[ix.clone()], &payer, &[&payer]).is_err());
    assert!(svm.get_account(&config).is_none());
    ix.data = data;
    send(&mut svm, &[ix.clone()], &payer, &[&payer]).unwrap();
    let acc = svm.get_account(&config).unwrap();
    let cfg = amm_video::Config::try_deserialize(&mut acc.data.as_slice()).unwrap();
    assert_eq!(cfg.fee, 30);
    assert_eq!(cfg.mint_x, x);
    assert_eq!(cfg.mint_y, y);
    assert_eq!(balance(&svm, treasury(config, true)), 0);
    assert_eq!(balance(&svm, treasury(config, false)), 0);
    assert!(send(&mut svm, &[ix], &payer, &[&payer]).is_err());
}

#[test]
fn test_deposit() {
    let (mut svm, payer, x, y, config, lp, vx, vy) = setup();
    let init = create_initialise_ix(&mut svm, &payer, x, y, config, lp, vx, vy);
    send(&mut svm, &[init], &payer, &[&payer]).unwrap();
    let mut dep = create_deposit_ix(&mut svm, &payer, x, y, lp, config, vx, vy);
    send(&mut svm, &[dep.clone()], &payer, &[&payer]).unwrap();
    let ulp = associated_token::get_associated_token_address(&payer.pubkey(), &lp);
    assert_eq!(balance(&svm, vx), 200_000_000);
    assert_eq!(balance(&svm, vy), 200_000_000);
    assert_eq!(balance(&svm, ulp), 100_000_000);

    dep.data = amm_video::instruction::Deposit { amount: 10_000_000, max_x: 19_999_999, max_y: 20_000_000 }.data();
    assert!(send(&mut svm, &[dep.clone()], &payer, &[&payer]).is_err());
    assert_eq!(balance(&svm, vx), 200_000_000);
    dep.data = amm_video::instruction::Deposit { amount: 10_000_000, max_x: 20_000_000, max_y: 20_000_000 }.data();
    send(&mut svm, &[dep], &payer, &[&payer]).unwrap();
    assert_eq!(balance(&svm, vx), 220_000_000);
    assert_eq!(balance(&svm, vy), 220_000_000);
    assert_eq!(balance(&svm, ulp), 110_000_000);
}

#[test]
fn test_withdraw() {
    let (mut svm, payer, x, y, config, lp, vx, vy) = setup();
    let init = create_initialise_ix(&mut svm, &payer, x, y, config, lp, vx, vy);
    send(&mut svm, &[init], &payer, &[&payer]).unwrap();
    let dep = create_deposit_ix(&mut svm, &payer, x, y, lp, config, vx, vy);
    send(&mut svm, &[dep], &payer, &[&payer]).unwrap();
    let mut ix = create_withdraw_ix(&mut svm, &payer, x, y, lp, config, vx, vy);
    let data = ix.data.clone();
    ix.data = amm_video::instruction::Withdraw { amount: 10_000_000, min_x: 20_000_001, min_y: 20_000_000 }.data();
    assert!(send(&mut svm, &[ix.clone()], &payer, &[&payer]).is_err());
    assert_eq!(balance(&svm, vx), 200_000_000);
    ix.data = data;
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
    let ulp = associated_token::get_associated_token_address(&payer.pubkey(), &lp);
    assert_eq!(balance(&svm, ulp), 90_000_000);
    assert_eq!(balance(&svm, vx), 180_000_000);
    assert_eq!(balance(&svm, vy), 180_000_000);
}

#[test]
fn test_swap_and_treasury() {
    let (mut svm, payer, x, y, config, lp, vx, vy) = setup();
    let init = create_initialise_ix(&mut svm, &payer, x, y, config, lp, vx, vy);
    send(&mut svm, &[init], &payer, &[&payer]).unwrap();
    let dep = create_deposit_ix(&mut svm, &payer, x, y, lp, config, vx, vy);
    send(&mut svm, &[dep], &payer, &[&payer]).unwrap();
    let mut ix = create_swap_ix(&mut svm, &payer, x, y, lp, config, vx, vy);
    let tx = treasury(config, true);
    let ty = treasury(config, false);
    let ux = associated_token::get_associated_token_address(&payer.pubkey(), &x);
    let uy = associated_token::get_associated_token_address(&payer.pubkey(), &y);
    let data = ix.data.clone();
    ix.data = amm_video::instruction::Swap { is_x: true, amount_in: 10_000_000, min_amount_out: u64::MAX }.data();
    assert!(send(&mut svm, &[ix.clone()], &payer, &[&payer]).is_err());
    assert_eq!(balance(&svm, tx), 0);
    ix.data = data;
    let mut bad = ix.clone();
    bad.accounts.iter_mut().find(|a| a.pubkey == tx).unwrap().pubkey = ux;
    assert!(send(&mut svm, &[bad], &payer, &[&payer]).is_err());
    let before_x = balance(&svm, ux);
    let before_y = balance(&svm, uy);
    let expected = (200_000_000u128 * 9_970_000 / 209_970_000) as u64;
    send(&mut svm, &[ix.clone()], &payer, &[&payer]).unwrap();
    assert_eq!(balance(&svm, tx), 6_000);
    assert_eq!(balance(&svm, vx), 209_994_000);
    assert_eq!(balance(&svm, vy), 200_000_000 - expected);
    assert_eq!(balance(&svm, ux), before_x - 10_000_000);
    assert_eq!(balance(&svm, uy), before_y + expected);
    let a = balance(&svm, vx);
    let b = balance(&svm, vy);
    assert!(u128::from(a) * u128::from(b) >= 200_000_000u128.pow(2));
    ix.data = amm_video::instruction::Swap { is_x: false, amount_in: 10_000_000, min_amount_out: 1 }.data();
    send(&mut svm, &[ix], &payer, &[&payer]).unwrap();
    let reverse = (u128::from(a) * 9_970_000 / (u128::from(b) + 9_970_000)) as u64;
    assert_eq!(balance(&svm, ty), 6_000);
    assert_eq!(balance(&svm, vx), a - reverse);
    assert_eq!(balance(&svm, vy), b + 9_994_000);
}
