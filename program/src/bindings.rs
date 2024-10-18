use std::collections::HashSet;
use std::iter::once;

#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "solana")]
use solana_program as domichain_program;

use borsh::BorshSerialize;
use domichain_program::instruction::{AccountMeta, Instruction};
use domichain_program::pubkey::Pubkey;
use domichain_program::{system_program, sysvar};

use crate::*;

pub fn create_multisig(
    funder_pubkey: &Pubkey,
    seed: u128,
    owners: Vec<Pubkey>,
    threshold: u64,
) -> Instruction {
    let multisig_pubkey = get_multisig_address(seed);

    let data = MultisigInstruction::CreateMultisig {
        seed,
        owners,
        threshold,
    }
    .try_to_vec()
    .expect("pack");

    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(*funder_pubkey, true),
            AccountMeta::new(multisig_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data,
    }
}

pub fn get_instruction_accounts(ixs: &[Instruction]) -> Vec<Pubkey> {
    let mut seen: HashSet<Pubkey> = HashSet::new();
    let transaction_accounts: Vec<_> = ixs
        .iter()
        .flat_map(|ix| {
            ix.accounts
                .iter()
                .map(|acc| acc.pubkey)
                .chain(once(ix.program_id))
        })
        .filter(|pk| {
            let is_new = seen.insert(*pk);
            is_new
        })
        .collect();

    transaction_accounts
}

pub fn encode_instructions(ixs: Vec<Instruction>) -> (Vec<Pubkey>, Vec<TransactionInstruction>) {
    let transaction_accounts: Vec<_> = get_instruction_accounts(&ixs);
    let pubkey_index = |pk| {
        transaction_accounts
            .iter()
            .position(|tx_pk| *tx_pk == pk)
            .unwrap() as u8
    };

    let mut transaction_instructions = Vec::with_capacity(ixs.len());
    for ix in ixs {
        let mut accounts = ix
            .accounts
            .into_iter()
            .map(|acc| TransactionAccount {
                pubkey_index: pubkey_index(acc.pubkey),
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            })
            .collect::<Vec<_>>();

        // Add transaction's program ID
        accounts.push(TransactionAccount {
            pubkey_index: pubkey_index(ix.program_id),
            is_signer: false,
            is_writable: false,
        });
        transaction_instructions.push(TransactionInstruction {
            program_id_index: pubkey_index(ix.program_id),
            accounts,
            data: ix.data,
        });
    }
    (transaction_accounts, transaction_instructions)
}

pub fn create_transaction(
    funder_pubkey: &Pubkey,
    proposer_pubkey: &Pubkey,
    multisig_pubkey: &Pubkey,
    seed: u128,
    ixs: Vec<Instruction>,
) -> Instruction {
    let transaction_pubkey = get_transaction_address(seed);
    let (transaction_accounts, transaction_instructions) = encode_instructions(ixs);

    let data = MultisigInstruction::CreateTransaction {
        seed,
        accounts: transaction_accounts,
        instructions: transaction_instructions,
    }
    .try_to_vec()
    .expect("pack");

    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(*funder_pubkey, true),
            AccountMeta::new(*proposer_pubkey, true),
            AccountMeta::new(*multisig_pubkey, false),
            AccountMeta::new(transaction_pubkey, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(sysvar::rent::id(), false),
        ],
        data,
    }
}

pub fn add_owner(multisig_pubkey: &Pubkey, owner: Pubkey) -> Instruction {
    let data = MultisigInstruction::AddOwner { owner }
        .try_to_vec()
        .expect("pack");

    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(*multisig_pubkey, true)],
        data,
    }
}

pub fn approve(
    proposer_pubkey: &Pubkey,
    multisig_pubkey: &Pubkey,
    transaction_pubkey: &Pubkey,
) -> Instruction {
    let data = MultisigInstruction::Approve.try_to_vec().expect("pack");

    Instruction {
        program_id: id(),
        accounts: vec![
            AccountMeta::new(*proposer_pubkey, true),
            AccountMeta::new(*transaction_pubkey, false),
            AccountMeta::new_readonly(*multisig_pubkey, false),
        ],
        data,
    }
}

pub fn execute_transaction(
    multisig_pubkey: &Pubkey,
    transaction_pubkey: &Pubkey,
    transaction_pubkeys: &[Pubkey],
    transaction_accounts: &[TransactionAccount],
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*multisig_pubkey, false),
        AccountMeta::new(*transaction_pubkey, false),
    ];

    for account in transaction_accounts {
        let mut account_meta: AccountMeta = account.to_account_meta(transaction_pubkeys);
        if account_meta.pubkey == *multisig_pubkey {
            account_meta.is_signer = false;
        }
        accounts.push(account_meta);
    }

    let data = MultisigInstruction::ExecuteTransaction
        .try_to_vec()
        .expect("pack");

    Instruction {
        program_id: id(),
        accounts,
        data,
    }
}

pub fn delete_pending_transaction(
    multisig_pubkey: &Pubkey,
    pending_transaction: Pubkey,
) -> Instruction {
    let data = MultisigInstruction::DeletePendingTransaction {
        pending_transaction,
    }
    .try_to_vec()
    .expect("pack");

    Instruction {
        program_id: id(),
        accounts: vec![AccountMeta::new(*multisig_pubkey, true)],
        data,
    }
}

pub fn get_multisig_address(seed: u128) -> Pubkey {
    Pubkey::find_program_address(&[br"multisig", &seed.to_le_bytes()], &id()).0
}

pub fn get_transaction_address(seed: u128) -> Pubkey {
    Pubkey::find_program_address(&[br"transaction", &seed.to_le_bytes()], &id()).0
}
