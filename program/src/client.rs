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
use spl_token_client::client::{ProgramClient, SendTransaction};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use domichain_program::program_pack::Pack;
use domichain_program::pubkey::Pubkey;
use domichain_program_test::tokio::time::timeout;
use domichain_sdk::account::ReadableAccount;
use domichain_sdk::instruction::Instruction;
use domichain_sdk::signature::Signer;
use domichain_sdk::signers::Signers;
use domichain_sdk::transaction::Transaction;

use crate::{Multisig, TransactionAccount};

pub struct MultisigClient<T> {
    banks_client: Arc<dyn ProgramClient<T>>,
    funder: Arc<dyn Signer>,
    voter: Arc<dyn Signer>,
    pub multisig_address: Pubkey,
    pub multisig_data: Multisig,
}

impl<T> MultisigClient<T>
where
    T: SendTransaction,
{
    pub async fn create_new(
        threshold: u64,
        owners: Vec<Pubkey>,
        banks_client: Arc<dyn ProgramClient<T>>,
        funder: Arc<dyn Signer>,
        voter: Arc<dyn Signer>,
    ) -> Self {
        let seed = uuid::Uuid::new_v4().as_u128();

        Self::create_new_with_seed(threshold, owners, banks_client, funder, voter, seed).await
    }

    pub async fn create_new_with_seed(
        threshold: u64,
        owners: Vec<Pubkey>,
        banks_client: Arc<dyn ProgramClient<T>>,
        funder: Arc<dyn Signer>,
        voter: Arc<dyn Signer>,
        seed: u128,
    ) -> Self {
        let recent_blockhash = banks_client.get_latest_blockhash().await.unwrap();

        // Create Multisig
        let mut transaction = Transaction::new_with_payer(
            &[crate::create_multisig(
                &funder.pubkey(),
                seed,
                owners,
                threshold,
            )],
            Some(&funder.pubkey()),
        );
        transaction.sign(&[funder.as_ref()], recent_blockhash);

        banks_client
            .send_transaction(&transaction)
            .await
            .expect("process_transaction");

        let multisig_address = crate::get_multisig_address(seed);

        Self::get_by_address(multisig_address, banks_client, funder, voter).await
    }

    pub async fn get_by_address(
        multisig_address: Pubkey,
        banks_client: Arc<dyn ProgramClient<T>>,
        funder: Arc<dyn Signer>,
        voter: Arc<dyn Signer>,
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

    async fn submit_transaction(&mut self, transaction: Transaction) {
        let _output = self
            .banks_client
            .send_transaction(&transaction)
            .await
            .unwrap();
    }

    async fn try_submit_transaction(&mut self, transaction: Transaction) -> Result<(), ()> {
        match self.banks_client.send_transaction(&transaction).await {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }
    /*
       async fn submit_transaction(&mut self, transaction: Transaction) {
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
    */
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
        transaction.sign(
            &[self.funder.as_ref(), self.voter.as_ref()],
            recent_blockhash,
        );

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

    pub async fn execute<S: Signers + ?Sized>(&mut self, transaction_address: Pubkey, signers: &S) {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;

        // Execute
        let mut transaction = Transaction::new_with_payer(
            &[crate::execute_transaction(
                &self.multisig_address,
                &transaction_address,
                &transaction_data.accounts,
                &unique_accounts(transaction_data.instructions),
            )],
            Some(&self.funder.pubkey()),
        );

        transaction.partial_sign(signers, recent_blockhash);
        transaction.sign(&[self.funder.as_ref()], recent_blockhash);

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

            ixs.push(crate::execute_transaction(
                &self.multisig_address,
                transaction_address,
                &transaction_data.accounts,
                &unique_accounts(transaction_data.instructions),
            ));
        }

        let mut transaction = Transaction::new_with_payer(&ixs, Some(&self.funder.pubkey()));
        transaction.sign(
            &[self.funder.as_ref(), self.voter.as_ref()],
            recent_blockhash,
        );

        self.submit_transaction(transaction).await;
    }

    pub async fn add_transaction_with_seed(
        &mut self,
        ixs: Vec<Instruction>,
        seed: u128,
    ) -> Result<Pubkey, ()> {
        let recent_blockhash = self.banks_client.get_latest_blockhash().await.unwrap();

        // Create Transaction instruction
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
        transaction.sign(
            &[self.funder.as_ref(), self.voter.as_ref()],
            recent_blockhash,
        );

        match timeout(
            Duration::from_secs(10),
            self.try_submit_transaction(transaction),
        )
        .await
        .unwrap()
        {
            Ok(_) => (),
            Err(_) => return Err(()),
        };

        let transaction_address = crate::get_transaction_address(seed);

        let transaction_data = self
            .get_transaction_data_by_address(transaction_address)
            .await;

        assert_eq!(transaction_data.is_initialized, true);
        assert_eq!(transaction_data.did_execute, false);

        self.sync_multisig_data().await;

        Ok(transaction_address)
    }

    pub async fn add_transaction(&mut self, ixs: Vec<Instruction>) -> Pubkey {
        let seed = uuid::Uuid::new_v4().as_u128();
        let transaction_address = self.add_transaction_with_seed(ixs, seed).await.unwrap();
        transaction_address
    }

    pub async fn add_owner(&mut self, owner: Pubkey) -> Pubkey {
        let ix = crate::add_owner(&self.multisig_address, owner);
        let transaction_address = self.add_transaction(vec![ix]).await;
        transaction_address
    }

    pub async fn delete_pending_transaction(&mut self, pending_transaction: Pubkey) -> Pubkey {
        let ix = crate::delete_pending_transaction(&self.multisig_address, pending_transaction);
        let transaction_address = self.add_transaction(vec![ix]).await;
        transaction_address
    }
}

fn unique_accounts(
    transaction_instructions: Vec<crate::TransactionInstruction>,
) -> Vec<TransactionAccount> {
    let accounts: Vec<_> = transaction_instructions
        .into_iter()
        .flat_map(|ix| ix.accounts)
        .collect();
    let mut pubkeys_to_account: HashMap<u8, TransactionAccount> = HashMap::new();
    for new_account in accounts {
        pubkeys_to_account
            .entry(new_account.pubkey_index)
            .and_modify(|existing_account| {
                // Update signer/writable if other instruction have it
                existing_account.is_signer |= new_account.is_signer;
                existing_account.is_writable |= new_account.is_writable;
            })
            .or_insert(new_account);
    }
    pubkeys_to_account.into_values().collect()
}
