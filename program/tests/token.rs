#![cfg(feature = "test-bpf")]

#[cfg(feature = "domichain")]
use domichain_program_test;
#[cfg(feature = "domichain")]
use domichain_sdk;

#[cfg(feature = "solana")]
use solana_program_test as domichain_program_test;
#[cfg(feature = "solana")]
use solana_sdk as domichain_sdk;

use domichain_program_test::{processor, tokio, ProgramTest, ProgramTestContext};
use domichain_sdk::signature::{Keypair, Signer};
use multisig::MultisigClient;
use spl_token_client;

use {
    domichain_program_test::tokio::sync::Mutex,
    spl_token_client::{
        client::{ProgramBanksClient, ProgramBanksClientProcessTransaction, ProgramClient},
        token::Token,
    },
    std::sync::Arc,
};

struct TestContext {
    pub ctx: Arc<Mutex<ProgramTestContext>>,

    pub decimals: u8,
    pub mint_authority: Keypair,
    pub token: Token<ProgramBanksClientProcessTransaction>,

    pub alice: Keypair,
    pub bob: Keypair,
}

impl TestContext {
    async fn new(program_test: ProgramTest) -> Self {
        let ctx = program_test.start_with_context().await;
        let ctx = Arc::new(Mutex::new(ctx));

        let payer = ctx.lock().await.payer.insecure_clone();

        let client: Arc<dyn ProgramClient<ProgramBanksClientProcessTransaction>> =
            Arc::new(ProgramBanksClient::new_from_context(
                Arc::clone(&ctx),
                ProgramBanksClientProcessTransaction,
            ));

        let decimals: u8 = 6;

        let mint_account = Keypair::new();
        let mint_authority = Keypair::new();

        let token = Token::new(
            Arc::clone(&client),
            &spl_token::id(),
            &mint_account.pubkey(),
            Some(decimals),
            Arc::new(payer.insecure_clone()),
        );

        token
            .create_mint(&mint_authority.pubkey(), None, vec![], &[&mint_account])
            .await
            .expect("failed to create mint");

        Self {
            ctx,

            decimals,
            mint_authority,
            token,

            alice: Keypair::new(),
            bob: Keypair::new(),
        }
    }
}

#[tokio::test]
async fn test_transfer() {
    let TestContext {
        decimals,
        mint_authority,
        token,
        alice,
        bob,
        ..
    } = TestContext::new(ProgramTest::default()).await;

    token
        .create_associated_token_account(&alice.pubkey())
        .await
        .expect("failed to create associated token account");
    let alice_vault = token.get_associated_token_address(&alice.pubkey());
    token
        .create_associated_token_account(&bob.pubkey())
        .await
        .expect("failed to create associated token account");
    let bob_vault = token.get_associated_token_address(&bob.pubkey());

    let mint_amount = 10 * u64::pow(10, decimals as u32);
    token
        .mint_to(
            &alice_vault,
            &mint_authority.pubkey(),
            mint_amount,
            &[&mint_authority],
        )
        .await
        .expect("failed to mint token");

    let transfer_amount = mint_amount.overflowing_div(3).0;
    token
        .transfer(
            &alice_vault,
            &bob_vault,
            &alice.pubkey(),
            transfer_amount,
            &[&alice],
        )
        .await
        .expect("failed to transfer");

    assert_eq!(
        token
            .get_account_info(&alice_vault)
            .await
            .expect("failed to get account")
            .base
            .amount,
        mint_amount - transfer_amount
    );
    assert_eq!(
        token
            .get_account_info(&bob_vault)
            .await
            .expect("failed to get account")
            .base
            .amount,
        transfer_amount
    );
}

#[tokio::test]
async fn test_multisig_token_transfer() {
    let program_test = ProgramTest::new(
        "multisig",
        multisig::id(),
        processor!(multisig::Processor::process),
    );

    // TokenClient
    let TestContext {
        ctx,

        decimals,
        mint_authority,
        token,

        alice,
        bob,
        ..
    } = TestContext::new(program_test).await;
    let banks_client = ctx.lock().await.banks_client.clone();
    let funder = ctx.lock().await.payer.insecure_clone();

    // Multisig
    let threshold = 1;

    let custodian_1 = Keypair::new();
    let owners = vec![custodian_1.pubkey()];

    let voter = custodian_1.insecure_clone();

    let mut multisig_client_1 = MultisigClient::create_new(
        threshold,
        owners,
        banks_client.clone(),
        funder.insecure_clone(),
        voter,
    )
    .await;

    // Create ATA for Multisig
    let multisig_address = multisig_client_1.multisig_address;
    token
        .create_associated_token_account(&multisig_address)
        .await
        .expect("failed to create associated token account");
    let multisig_vault = token.get_associated_token_address(&multisig_address);
    // Create ATA for Destinations
    token
        .create_associated_token_account(&alice.pubkey())
        .await
        .expect("failed to create associated token account");
    let alice_vault = token.get_associated_token_address(&alice.pubkey());
    token
        .create_associated_token_account(&bob.pubkey())
        .await
        .expect("failed to create associated token account");
    let bob_vault = token.get_associated_token_address(&bob.pubkey());

    // Mint to multisig
    let mint_amount = 10 * u64::pow(10, decimals as u32);
    token
        .mint_to(
            &multisig_vault,
            &mint_authority.pubkey(),
            mint_amount,
            &[&mint_authority],
        )
        .await
        .expect("failed to mint token");

    // Transfer from miltisig:
    let transfer_amount = mint_amount.overflowing_div(3).0;

    // - create transfer instruction
    // - create proposal
    // Combine into one list of instructions
    let mut ixs = Vec::new();
    // Transfer from Multisig to Alice
    ixs.extend(
        token
            .transfer_ix(
                &multisig_vault,
                &alice_vault,
                &multisig_address,
                transfer_amount,
                &[multisig_address],
            )
            .await
            .expect("failed to create transfer ix"),
    );
    // Transfer from Multisig to Bob
    ixs.extend(
        token
            .transfer_ix(
                &multisig_vault,
                &bob_vault,
                &multisig_address,
                transfer_amount,
                &[multisig_address],
            )
            .await
            .expect("failed to create transfer ix"),
    );

    let transaction_address = multisig_client_1.add_transaction(ixs).await;

    // - execute proposal
    multisig_client_1.execute(transaction_address).await;

    // - verify transfer
    assert_eq!(
        token
            .get_account_info(&multisig_vault)
            .await
            .expect("failed to get account")
            .base
            .amount,
        mint_amount - transfer_amount * 2
    );
    assert_eq!(
        token
            .get_account_info(&alice_vault)
            .await
            .expect("failed to get account")
            .base
            .amount,
        transfer_amount
    );
    assert_eq!(
        token
            .get_account_info(&bob_vault)
            .await
            .expect("failed to get account")
            .base
            .amount,
        transfer_amount
    );
}
