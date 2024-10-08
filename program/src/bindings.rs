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

pub fn create_transaction(
    funder_pubkey: &Pubkey,
    proposer_pubkey: &Pubkey,
    multisig_pubkey: &Pubkey,
    seed: u128,
    ixs: Vec<Instruction>,
) -> Instruction {
    let transaction_pubkey = get_transaction_address(seed);

    let mut transaction_instructions = Vec::with_capacity(ixs.len());
    for ix in ixs {
        let mut accounts = ix
            .accounts
            .into_iter()
            .map(|acc| TransactionAccount {
                pubkey: acc.pubkey,
                is_signer: acc.is_signer,
                is_writable: acc.is_writable,
            })
            .collect::<Vec<_>>();

        // Add transaction's program ID
        accounts.push(TransactionAccount {
            pubkey: ix.program_id,
            is_signer: false,
            is_writable: false,
        });
        transaction_instructions.push(TransactionInstruction {
            program_id: ix.program_id,
            accounts,
            data: ix.data,
        });
    }

    let data = MultisigInstruction::CreateTransaction {
        seed,
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
    transaction_accounts: &[TransactionAccount],
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(*multisig_pubkey, false),
        AccountMeta::new(*transaction_pubkey, false),
    ];

    for account in transaction_accounts {
        let mut account_meta: AccountMeta = account.into();
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
