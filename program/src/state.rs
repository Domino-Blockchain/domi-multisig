#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "solana")]
use solana_program as domichain_program;

use borsh::{BorshDeserialize, BorshSerialize};

use domichain_program::instruction::{AccountMeta, Instruction};
use domichain_program::program_error::ProgramError;
use domichain_program::program_pack::{IsInitialized, Pack, Sealed};
use domichain_program::pubkey::Pubkey;

use multisig_derive::MultisigPack;

/// Minimum number of multisignature signers
pub const MIN_SIGNERS: usize = 1;
/// Maximum number of multisignature signers
pub const MAX_SIGNERS: usize = 10;
/// Maximum number of pending transactions
pub const MAX_TRANSACTIONS: usize = 15;

#[derive(Debug, BorshSerialize, BorshDeserialize, MultisigPack)]
#[multisig_pack(length = 833)]
pub struct Multisig {
    pub is_initialized: bool,
    // Set of custodians
    pub owners: Vec<Pubkey>,
    // Required number of signers
    pub threshold: u64,
    // Set of pending transactions
    pub pending_transactions: Vec<Pubkey>,
    // Seed to derive PDA
    pub seed: u128,
}

impl Sealed for Multisig {}

impl IsInitialized for Multisig {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    pub is_initialized: bool,
    // The multisig account this transaction belongs to.
    pub multisig: Pubkey,
    // signers[index] is true if multisig.owners[index] signed the transaction.
    pub signers: Vec<bool>,
    // Boolean ensuring one time execution.
    pub did_execute: bool,
    // Instructions for the transaction.
    pub instructions: Vec<TransactionInstruction>,
}

impl Sealed for Transaction {}

impl IsInitialized for Transaction {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Transaction {
    pub fn pack_into_slice(&self, dst: &mut [u8]) {
        let data = self.try_to_vec().unwrap();
        let (left, _) = dst.split_at_mut(data.len());
        left.copy_from_slice(&data);
    }

    pub fn unpack_from_slice(mut src: &[u8]) -> Result<Self, ProgramError> {
        let unpacked = Self::deserialize(&mut src)?;
        Ok(unpacked)
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize)]
pub struct TransactionInstruction {
    // Target program to execute against.
    pub program_id: Pubkey,
    // Accounts required for the instruction.
    pub accounts: Vec<TransactionAccount>,
    // Instruction data for the instruction.
    pub data: Vec<u8>,
}

impl From<&TransactionInstruction> for Instruction {
    fn from(tx: &TransactionInstruction) -> Instruction {
        Instruction {
            program_id: tx.program_id,
            accounts: tx.accounts.iter().map(Into::into).collect(),
            data: tx.data.clone(),
        }
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize)]
pub struct TransactionAccount {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl From<&TransactionAccount> for AccountMeta {
    fn from(account: &TransactionAccount) -> AccountMeta {
        match account.is_writable {
            false => AccountMeta::new_readonly(account.pubkey, account.is_signer),
            true => AccountMeta::new(account.pubkey, account.is_signer),
        }
    }
}
