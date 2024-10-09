#![cfg(feature = "test-bpf")]

#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "domichain")]
use domichain_program_test;
#[cfg(feature = "domichain")]
use domichain_sdk;

use multisig::MultisigClient;
#[cfg(feature = "solana")]
use solana_program as domichain_program;
#[cfg(feature = "solana")]
use solana_program_test as domichain_program_test;
#[cfg(feature = "solana")]
use solana_sdk as domichain_sdk;

use std::sync::Arc;
use std::time::Duration;

use domichain_program::program_pack::Pack;
use domichain_program::pubkey::Pubkey;
use domichain_program_test::tokio::sync::Mutex;
use domichain_program_test::tokio::time::timeout;
use domichain_program_test::{processor, tokio, ProgramTest};
use domichain_sdk::account::ReadableAccount;
use domichain_sdk::native_token::SATOMIS_PER_DOMI;
use domichain_sdk::rent::Rent;
use domichain_sdk::signature::{Keypair, Signer};
use domichain_sdk::system_instruction::transfer;
use domichain_sdk::transaction::Transaction;

#[tokio::test]
async fn test_client_combined_mint_case_n_times() {
    for _ in 0..5 {
        for_test_client_combined_mint_case().await;
    }
}

async fn for_test_client_combined_mint_case() {
    let program_test = ProgramTest::new(
        "multisig",
        multisig::id(),
        processor!(multisig::Processor::process),
    );

    // Start Program Test
    let (banks_client, funder, _recent_blockhash) = program_test.start().await;

    let threshold = 2;
    let n_owners = 3;

    let owners_keys: Vec<_> = (0..n_owners).map(|_| Keypair::new()).collect();
    let owners: Vec<_> = owners_keys.iter().map(|k| k.pubkey()).collect();

    let mut multisig_address = None;
    let mut multisig_clients = Vec::with_capacity(owners.len());
    for (i, voter) in owners_keys.into_iter().enumerate() {
        let multisig_client = if let Some(multisig_address) = multisig_address {
            MultisigClient::get_by_address(
                multisig_address,
                banks_client.clone(),
                funder.insecure_clone(),
                voter,
            )
            .await
        } else {
            let multisig_client = MultisigClient::create_new(
                threshold,
                owners.clone(),
                banks_client.clone(),
                funder.insecure_clone(),
                voter,
            )
            .await;
            multisig_address = Some(multisig_client.multisig_address);
            multisig_client
        };
        multisig_clients.push((i, Arc::new(Mutex::new(multisig_client))));
    }

    let dur = Duration::from_secs(20);

    let mut handles = Vec::new();
    for (i, (j, multisig_client)) in multisig_clients.clone().into_iter().enumerate() {
        assert_eq!(i, j);

        let mut other_clients = multisig_clients.clone();
        other_clients.remove(i);
        let other_clients_ = other_clients.iter().map(|(k, _)| *k).collect::<Vec<_>>();
        // dbg!(i, &other_clients_);
        let other_multisig_client_index = i % other_clients_.len();

        handles.push(tokio::spawn(async move {
            let seed = uuid::Uuid::new_v4().as_u128();
            let owner = Pubkey::new_unique();
            // dbg!(&owner, seed);
            // eprintln!("{}?", "\t".repeat(i));
            let mut multisig_client = timeout(dur, multisig_client.lock()).await.unwrap();
            // eprintln!("{}+", "\t".repeat(i));
            let ix = multisig::add_owner(&multisig_client.multisig_address, owner);
            let transaction_address = timeout(
                dur,
                multisig_client.add_transaction_with_seed(vec![ix], seed),
            )
            .await
            .map_err(|e| (e, i))
            .unwrap();
            drop(multisig_client);
            // eprintln!("{}-", "\t".repeat(i));

            // eprintln!(
            //     "{}?",
            //     "\t".repeat(other_clients_[other_multisig_client_index])
            // );
            let mut other_multisig_client =
                timeout(dur, other_clients[other_multisig_client_index].1.lock())
                    .await
                    .unwrap();
            // eprintln!(
            //     "{}+",
            //     "\t".repeat(other_clients_[other_multisig_client_index])
            // );
            other_multisig_client.approve(transaction_address).await;
            other_multisig_client
                .execute::<[&Keypair; 0]>(transaction_address, &[])
                .await;
            // eprintln!(
            //     "{}-",
            //     "\t".repeat(other_clients_[other_multisig_client_index])
            // );
        }));
    }
    for (i, handle) in handles.into_iter().enumerate() {
        handle.await.map_err(|e| (e, i)).unwrap();
    }
}

#[tokio::test]
async fn test_client() {
    let program_test = ProgramTest::new(
        "multisig",
        multisig::id(),
        processor!(multisig::Processor::process),
    );

    // Start Program Test
    let (banks_client, funder, _recent_blockhash) = program_test.start().await;

    let threshold = 2;

    let custodian_1 = Keypair::new();
    let custodian_2 = Keypair::new();
    let custodian_3 = Keypair::new();
    let owners = vec![
        custodian_1.pubkey(),
        custodian_2.pubkey(),
        custodian_3.pubkey(),
    ];

    let voter = custodian_1.insecure_clone();

    let mut multisig_client_1 = MultisigClient::create_new(
        threshold,
        owners,
        banks_client.clone(),
        funder.insecure_clone(),
        voter,
    )
    .await;

    let voter = custodian_2.insecure_clone();
    let mut multisig_client_2 = MultisigClient::get_by_address(
        multisig_client_1.multisig_address,
        banks_client,
        funder,
        voter,
    )
    .await;

    // Add owners
    let is_batch = false;
    if is_batch {
        let mut transaction_addresses = Vec::new();
        for _ in 0..7 {
            let owner = Pubkey::new_unique();
            transaction_addresses.push(multisig_client_1.add_owner(owner).await);
        }
        multisig_client_2
            .batch_approve_execute(&transaction_addresses)
            .await;
    } else {
        for _ in 0..7 {
            let owner = Pubkey::new_unique();
            let transaction_address = multisig_client_1.add_owner(owner).await;

            multisig_client_2.approve(transaction_address).await;
            multisig_client_2
                .execute::<[&Keypair; 0]>(transaction_address, &[])
                .await;
        }
    }
    multisig_client_1.sync_multisig_data().await;

    assert_eq!(
        multisig_client_1.multisig_data.owners.len(),
        multisig::MAX_SIGNERS
    );

    // Create pending transactions
    let mut pending_transactions = Vec::new();

    for _ in 0..multisig::MAX_TRANSACTIONS - 1 {
        let owner = Pubkey::new_unique();
        pending_transactions.push(multisig_client_1.add_owner(owner).await);
    }

    let pending_transaction = *pending_transactions.last().unwrap();

    let transaction_address = multisig_client_1
        .delete_pending_transaction(pending_transaction)
        .await;

    assert_eq!(
        multisig_client_1.multisig_data.pending_transactions.len(),
        multisig::MAX_TRANSACTIONS
    );

    multisig_client_2.approve(transaction_address).await;
    multisig_client_2
        .execute::<[&Keypair; 0]>(transaction_address, &[])
        .await;

    assert_eq!(
        multisig_client_2.multisig_data.pending_transactions.len(),
        multisig::MAX_TRANSACTIONS - 2
    );
}

#[tokio::test]
async fn test() {
    let program_test = ProgramTest::new(
        "multisig",
        multisig::id(),
        processor!(multisig::Processor::process),
    );

    // Start Program Test
    let (mut banks_client, funder, recent_blockhash) = program_test.start().await;

    // Create Multisig
    let seed = uuid::Uuid::new_v4().as_u128();

    let threshold = 2;

    let custodian_1 = Keypair::new();
    let custodian_2 = Keypair::new();
    let custodian_3 = Keypair::new();

    let mut transaction = Transaction::new_with_payer(
        &[multisig::create_multisig(
            &funder.pubkey(),
            seed,
            vec![
                custodian_1.pubkey(),
                custodian_2.pubkey(),
                custodian_3.pubkey(),
            ],
            threshold,
        )],
        Some(&funder.pubkey()),
    );
    transaction.sign(&[&funder], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    let multisig_address = multisig::get_multisig_address(seed);

    let multisig_info = banks_client
        .get_account(multisig_address)
        .await
        .expect("get_account")
        .expect("account");

    let multisig_data = multisig::Multisig::unpack(multisig_info.data()).expect("multisig unpack");

    assert_eq!(multisig_data.is_initialized, true);
    assert_eq!(multisig_data.threshold, threshold);
    assert_eq!(
        multisig_data.owners,
        vec![
            custodian_1.pubkey(),
            custodian_2.pubkey(),
            custodian_3.pubkey()
        ]
    );

    // dbg!(funder.pubkey());
    // dbg!(custodian_1.pubkey());
    // dbg!(custodian_2.pubkey());
    // dbg!(custodian_3.pubkey());

    // Add owners
    for _ in 0..7 {
        let seed = uuid::Uuid::new_v4().as_u128();

        let owner = Pubkey::new_unique();
        // dbg!(owner);

        // Create Transaction instruction
        let mut transaction = Transaction::new_with_payer(
            &[multisig::create_transaction(
                &funder.pubkey(),
                &custodian_1.pubkey(),
                &multisig_address,
                seed,
                vec![multisig::add_owner(&multisig_address, owner)],
            )],
            Some(&funder.pubkey()),
        );
        transaction.sign(&[&funder, &custodian_1], recent_blockhash);

        banks_client
            .process_transaction(transaction)
            .await
            .expect("process_transaction");

        let transaction_address = multisig::get_transaction_address(seed);
        // dbg!(transaction_address);

        let transaction_info = banks_client
            .get_account(transaction_address)
            .await
            .expect("get_account")
            .expect("account");

        let transaction_data = multisig::Transaction::unpack_from_slice(transaction_info.data())
            .expect("transaction unpack");

        // dbg!(&transaction_data);

        assert_eq!(transaction_data.is_initialized, true);
        assert_eq!(transaction_data.did_execute, false);
        assert_eq!(transaction_data.instructions[0].program_id, multisig::id());

        // Approve
        let mut transaction = Transaction::new_with_payer(
            &[multisig::approve(
                &custodian_2.pubkey(),
                &multisig_address,
                &transaction_address,
            )],
            Some(&funder.pubkey()),
        );
        transaction.sign(&[&funder, &custodian_2], recent_blockhash);

        banks_client
            .process_transaction(transaction)
            .await
            .expect("process_transaction");

        let transaction_info = banks_client
            .get_account(transaction_address)
            .await
            .expect("get_account")
            .expect("account");

        let transaction_data = multisig::Transaction::unpack_from_slice(transaction_info.data())
            .expect("transaction unpack");

        assert_eq!(transaction_data.signers[0], true);
        assert_eq!(transaction_data.signers[1], true);
        assert_eq!(transaction_data.signers[2], false);

        // Execute
        let accounts = transaction_data.instructions[0].clone().accounts;
        // dbg!(&accounts);

        let mut transaction = Transaction::new_with_payer(
            &[multisig::execute_transaction(
                &multisig_address,
                &transaction_address,
                &accounts,
            )],
            Some(&funder.pubkey()),
        );
        // dbg!(&transaction);

        // dbg!(transaction.signer_key(0, 0));
        // dbg!(transaction.signer_key(0, 1));
        // dbg!(transaction.signer_key(0, 2));
        // dbg!(transaction.signer_key(0, 3));
        // dbg!(transaction.signer_key(0, 4));
        // dbg!(transaction.signer_key(0, 5));
        transaction.sign(&[&funder], recent_blockhash);

        banks_client
            .process_transaction(transaction)
            .await
            .expect("process_transaction");

        let transaction_info = banks_client
            .get_account(transaction_address)
            .await
            .expect("get_account")
            .expect("account");

        let transaction_data = multisig::Transaction::unpack_from_slice(transaction_info.data())
            .expect("transaction unpack");

        assert_eq!(transaction_data.did_execute, true);
    }

    // Check multisig owners
    let multisig_info = banks_client
        .get_account(multisig_address)
        .await
        .expect("get_account")
        .expect("account");

    let multisig_data = multisig::Multisig::unpack(multisig_info.data()).expect("multisig unpack");

    assert_eq!(multisig_data.owners.len(), multisig::MAX_SIGNERS);

    // Create pending transactions
    let mut pending_transactions = Vec::new();

    for _ in 0..multisig::MAX_TRANSACTIONS - 1 {
        let owner = Pubkey::new_unique();
        let seed = uuid::Uuid::new_v4().as_u128();

        // Create Transaction instruction
        let mut transaction = Transaction::new_with_payer(
            &[multisig::create_transaction(
                &funder.pubkey(),
                &custodian_1.pubkey(),
                &multisig_address,
                seed,
                vec![multisig::add_owner(&multisig_address, owner)],
            )],
            Some(&funder.pubkey()),
        );
        transaction.sign(&[&funder, &custodian_1], recent_blockhash);

        banks_client
            .process_transaction(transaction)
            .await
            .expect("process_transaction");

        let transaction_address = multisig::get_transaction_address(seed);
        pending_transactions.push(transaction_address);
    }

    // Delete last pending transaction
    let seed = uuid::Uuid::new_v4().as_u128();
    let pending_transaction = *pending_transactions.last().unwrap();

    // Create Transaction instruction
    let mut transaction = Transaction::new_with_payer(
        &[multisig::create_transaction(
            &funder.pubkey(),
            &custodian_1.pubkey(),
            &multisig_address,
            seed,
            vec![multisig::delete_pending_transaction(
                &multisig_address,
                pending_transaction,
            )],
        )],
        Some(&funder.pubkey()),
    );
    transaction.sign(&[&funder, &custodian_1], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Check multisig pending transactions
    let multisig_info = banks_client
        .get_account(multisig_address)
        .await
        .expect("get_account")
        .expect("account");

    let multisig_data = multisig::Multisig::unpack(multisig_info.data()).expect("multisig unpack");

    assert_eq!(
        multisig_data.pending_transactions.len(),
        multisig::MAX_TRANSACTIONS
    );

    // Approve
    let transaction_address = multisig::get_transaction_address(seed);

    let mut transaction = Transaction::new_with_payer(
        &[multisig::approve(
            &custodian_2.pubkey(),
            &multisig_address,
            &transaction_address,
        )],
        Some(&funder.pubkey()),
    );
    transaction.sign(&[&funder, &custodian_2], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Execute
    let transaction_info = banks_client
        .get_account(transaction_address)
        .await
        .expect("get_account")
        .expect("account");

    let transaction_data = multisig::Transaction::unpack_from_slice(transaction_info.data())
        .expect("transaction unpack");

    let accounts = transaction_data.instructions[0].clone().accounts;

    let mut transaction = Transaction::new_with_payer(
        &[multisig::execute_transaction(
            &multisig_address,
            &transaction_address,
            &accounts,
        )],
        Some(&funder.pubkey()),
    );

    transaction.sign(&[&funder], recent_blockhash);

    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Check multisig pending transactions
    let multisig_info = banks_client
        .get_account(multisig_address)
        .await
        .expect("get_account")
        .expect("account");

    let multisig_data = multisig::Multisig::unpack(multisig_info.data()).expect("multisig unpack");

    assert_eq!(
        multisig_data.pending_transactions.len(),
        multisig::MAX_TRANSACTIONS - 2
    );
}

#[ignore = "cannot transfer SOL from non-empty account"]
#[tokio::test]
async fn test_client_multiple_instructions() {
    let program_test = ProgramTest::new(
        "multisig",
        multisig::id(),
        processor!(multisig::Processor::process),
    );

    // Start Program Test
    let (mut banks_client, funder, recent_blockhash) = program_test.start().await;

    let threshold = 1;

    let custodian_1 = Keypair::new();
    let owners = vec![custodian_1.pubkey()];

    let voter = custodian_1.insecure_clone();

    let multisig_client_1 = MultisigClient::create_new(
        threshold,
        owners,
        banks_client.clone(),
        funder.insecure_clone(),
        voter,
    )
    .await;

    let ix = transfer(
        &funder.pubkey(),
        &multisig_client_1.multisig_address,
        SATOMIS_PER_DOMI,
    );
    let mut transaction = Transaction::new_with_payer(&[ix], Some(&funder.pubkey()));
    transaction.sign(&[&funder], recent_blockhash);
    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Verify balance
    let actual_balance = banks_client
        .get_balance(multisig_client_1.multisig_address)
        .await
        .expect("get_balance");

    let multisig_data = banks_client
        .get_account(multisig_client_1.multisig_address)
        .await
        .expect("get_account")
        .unwrap();
    let minimum_rent = Rent::default().minimum_balance(multisig_data.data.len());
    let expected_balance = SATOMIS_PER_DOMI + minimum_rent;

    assert_eq!(actual_balance, expected_balance);

    let transfer_destination = Pubkey::new_unique();
    let ix = transfer(
        &multisig_client_1.multisig_address,
        &transfer_destination,
        SATOMIS_PER_DOMI,
    );

    let seed = uuid::Uuid::new_v4().as_u128();
    let transaction_address = multisig::get_transaction_address(seed);

    // Create Transaction instruction
    let ixs = vec![ix];
    let mut transaction = Transaction::new_with_payer(
        &[multisig::create_transaction(
            &funder.pubkey(),
            &custodian_1.pubkey(),
            &multisig_client_1.multisig_address,
            seed,
            ixs,
        )],
        Some(&funder.pubkey()),
    );
    transaction.sign(&[&funder, &custodian_1], recent_blockhash);
    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Execute
    let transaction_info = banks_client
        .get_account(transaction_address)
        .await
        .expect("get_account")
        .expect("account");
    let transaction_data = multisig::Transaction::unpack_from_slice(transaction_info.data())
        .expect("transaction unpack");

    let accounts = transaction_data.instructions[0].clone().accounts;
    let mut transaction = Transaction::new_with_payer(
        &[multisig::execute_transaction(
            &multisig_client_1.multisig_address,
            &transaction_address,
            &accounts,
        )],
        Some(&funder.pubkey()),
    );
    transaction.sign(&[&funder], recent_blockhash);
    dbg!(&transaction);
    banks_client
        .process_transaction(transaction)
        .await
        .expect("process_transaction");

    // Check multisig pending transactions
    let multisig_info = banks_client
        .get_account(multisig_client_1.multisig_address)
        .await
        .expect("get_account")
        .expect("account");
    let multisig_data = multisig::Multisig::unpack(multisig_info.data()).expect("multisig unpack");
    assert_eq!(multisig_data.pending_transactions.len(), 0);

    assert_eq!(
        banks_client
            .get_balance(transfer_destination)
            .await
            .expect("get_balance"),
        SATOMIS_PER_DOMI
    );
}
