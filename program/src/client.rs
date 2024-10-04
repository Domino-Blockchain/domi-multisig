#![cfg(feature = "test-bpf")]

#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "domichain")]
use domichain_program_test;
#[cfg(feature = "domichain")]
use domichain_sdk;

#[cfg(feature = "solana")]
use solana_program as domichain_program;
#[cfg(feature = "solana")]
use solana_program_test as domichain_program_test;
#[cfg(feature = "solana")]
use solana_sdk as domichain_sdk;

use std::time::Duration;

use domichain_program::program_pack::Pack;
use domichain_program::pubkey::Pubkey;
use domichain_program_test::tokio::time::{timeout, Instant};
use domichain_program_test::BanksClient;
use domichain_sdk::account::ReadableAccount;
use domichain_sdk::instruction::Instruction;
use domichain_sdk::signature::{Keypair, Signer};
use domichain_sdk::transaction::Transaction;

use crate::Multisig;

pub struct MultisigClient {
    banks_client: BanksClient,
    funder: Keypair,
    voter: Keypair,
    pub multisig_address: Pubkey,
    pub multisig_data: Multisig,
}

impl Drop for MultisigClient {
    fn drop(&mut self) {
        eprintln!("drop(MultisigClient)");
    }
}

impl MultisigClient {
    pub async fn create_new(
        threshold: u64,
        owners: Vec<Pubkey>,
        mut banks_client: BanksClient,
        funder: Keypair,
        voter: Keypair,
    ) -> Self {
        let recent_blockhash = banks_client.get_latest_blockhash().await.unwrap();

        // Create Multisig
        let seed = uuid::Uuid::new_v4().as_u128();

        let mut transaction = Transaction::new_with_payer(
            &[crate::create_multisig(
                &funder.pubkey(),
                seed,
                owners,
                threshold,
            )],
            Some(&funder.pubkey()),
        );
        transaction.sign(&[&funder], recent_blockhash);

        banks_client
            .process_transaction(transaction)
            .await
            .expect("process_transaction");

        let multisig_address = crate::get_multisig_address(seed);

        Self::get_by_address(multisig_address, banks_client, funder, voter).await
    }

    pub async fn get_by_address(
        multisig_address: Pubkey,
        mut banks_client: BanksClient,
        funder: Keypair,
        voter: Keypair,
    ) -> Self {
        let multisig_info = banks_client
            .get_account(multisig_address)
            .await
            .expect("get_account")
            .expect("account");

        let multisig_data = crate::Multisig::unpack(multisig_info.data()).expect("multisig unpack");

        Self {
            banks_client,
            funder,
            voter,
            multisig_address,
            multisig_data,
        }
    }

    pub async fn get_transaction_data_by_address(
        &mut self,
        transaction_address: Pubkey,
    ) -> crate::Transaction {
        let transaction_info = self
            .banks_client
            .get_account(transaction_address)
            .await
            .expect("get_account")
            .expect("account");

        let transaction_data = crate::Transaction::unpack_from_slice(transaction_info.data())
            .expect("transaction unpack");

        transaction_data
    }

    pub async fn sync_multisig_data(&mut self) {
        let multisig_info = self
            .banks_client
            .get_account(self.multisig_address)
            .await
            .expect("get_account")
            .expect("account");

        self.multisig_data =
            crate::Multisig::unpack(multisig_info.data()).expect("multisig unpack");
    }

    pub async fn submit_transaction(&mut self, transaction: Transaction) {
        let is_polling = true;
        let is_simulate = true;
        if is_polling {
            'main: loop {
                let signature = transaction.signatures[0];

                if is_simulate {
                    loop {
                        let res = timeout(
                            Duration::from_secs(1),
                            self.banks_client.simulate_transaction(transaction.clone()),
                        )
                        .await;
                        if let Ok(Ok(res)) = res {
                            if matches!(res.result, Some(Ok(()))) {
                                break;
                            }
                        }
                    }
                }

                timeout(
                    Duration::from_secs(1),
                    self.banks_client.send_transaction(transaction.clone()),
                )
                .await
                .unwrap()
                .expect("send_transaction");

                let start = Instant::now();
                loop {
                    let status = timeout(
                        Duration::from_secs(1),
                        self.banks_client.get_transaction_status(signature),
                    )
                    .await
                    .unwrap()
                    .expect("get_transaction_status");
                    // let slot = self
                    //     .banks_client
                    //     .get_slot_with_context(
                    //         Context::current(),
                    //         domichain_sdk::commitment_config::CommitmentLevel::Confirmed,
                    //     )
                    //     .await
                    //     .expect("get_slot_with_context");
                    // dbg!(slot);
                    if status.is_some() {
                        break 'main;
                    }
                    if start.elapsed() > Duration::from_secs(1) {
                        continue 'main;
                    }
                }
            }
        } else {
            self.banks_client
                .process_transaction(transaction)
                .await
                .expect("process_transaction");
        }
    }

    pub async fn approve(&mut self, transaction_address: Pubkey) {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;
        let signers_before = transaction_data.signers.iter().filter(|s| **s).count();

        // Approve
        let mut transaction = Transaction::new_with_payer(
            &[crate::approve(
                &self.voter.pubkey(),
                &self.multisig_address,
                &transaction_address,
            )],
            Some(&self.funder.pubkey()),
        );
        transaction.sign(&[&self.funder, &self.voter], recent_blockhash);

        self.submit_transaction(transaction).await;

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;
        let signers_after = transaction_data.signers.iter().filter(|s| **s).count();

        assert!(
            signers_after > signers_before,
            "{signers_after} > {signers_before}"
        );

        self.sync_multisig_data().await;
    }

    pub async fn execute(&mut self, transaction_address: Pubkey) {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;

        // Execute
        // FIXME: handle multiple instructions
        let accounts = transaction_data.instructions[0].clone().accounts;

        let mut transaction = Transaction::new_with_payer(
            &[crate::execute_transaction(
                &self.multisig_address,
                &transaction_address,
                &accounts,
            )],
            Some(&self.funder.pubkey()),
        );

        transaction.sign(&[&self.funder], recent_blockhash);

        self.submit_transaction(transaction).await;

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;
        assert_eq!(transaction_data.did_execute, true);

        self.sync_multisig_data().await;
    }

    pub async fn batch_approve_execute(&mut self, transaction_addresses: &[Pubkey]) {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        let mut ixs = Vec::new();
        for transaction_address in transaction_addresses {
            ixs.push(crate::approve(
                &self.voter.pubkey(),
                &self.multisig_address,
                transaction_address,
            ));
        }
        for transaction_address in transaction_addresses {
            let transaction_data = self
                .get_transaction_data_by_address(*transaction_address)
                .await;
            // FIXME: handle multiple instructions
            let accounts = transaction_data.instructions[0].clone().accounts;

            ixs.push(crate::execute_transaction(
                &self.multisig_address,
                transaction_address,
                &accounts,
            ));
        }

        let mut transaction = Transaction::new_with_payer(&ixs, Some(&self.funder.pubkey()));
        transaction.sign(&[&self.funder, &self.voter], recent_blockhash);

        self.submit_transaction(transaction).await;
    }

    pub async fn add_transaction_with_seed(&mut self, ix: Instruction, seed: u128) -> Pubkey {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        // Create Transaction instruction
        // FIXME: handle multiple instructions
        let ixs = vec![ix];
        let mut transaction = Transaction::new_with_payer(
            &[crate::create_transaction(
                &self.funder.pubkey(),
                &self.voter.pubkey(),
                &self.multisig_address,
                seed,
                ixs,
            )],
            Some(&self.funder.pubkey()),
        );
        transaction.sign(&[&self.funder, &self.voter], recent_blockhash);

        timeout(
            Duration::from_secs(10),
            self.submit_transaction(transaction),
        )
        .await
        .unwrap();

        let transaction_address = crate::get_transaction_address(seed);

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;

        assert_eq!(transaction_data.is_initialized, true);
        assert_eq!(transaction_data.did_execute, false);

        self.sync_multisig_data().await;

        transaction_address
    }

    pub async fn add_transaction(&mut self, ix: Instruction) -> Pubkey {
        let seed = uuid::Uuid::new_v4().as_u128();
        self.add_transaction_with_seed(ix, seed).await
    }

    pub async fn add_owner(&mut self, owner: Pubkey) -> Pubkey {
        let ix = crate::add_owner(&self.multisig_address, owner);
        let transaction_address = self.add_transaction(ix).await;
        transaction_address
    }

    pub async fn delete_pending_transaction(&mut self, pending_transaction: Pubkey) -> Pubkey {
        let ix = crate::delete_pending_transaction(&self.multisig_address, pending_transaction);
        let transaction_address = self.add_transaction(ix).await;
        transaction_address
    }
}
