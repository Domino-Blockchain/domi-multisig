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
    // Accounts used in transaction
    pub accounts: Vec<Pubkey>,
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
    pub program_id_index: u8,
    // Accounts required for the instruction.
    pub accounts: Vec<TransactionAccount>,
    // Instruction data for the instruction.
    pub data: Vec<u8>,
}

impl TransactionInstruction {
    pub fn to_instruction(&self, accounts: &[Pubkey]) -> Instruction {
        Instruction {
            program_id: accounts[self.program_id_index as usize],
            accounts: self
                .accounts
                .iter()
                .map(|account| account.to_account_meta(accounts))
                .collect(),
            data: self.data.clone(),
        }
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, serde::Serialize, serde::Deserialize)]
pub struct TransactionAccount {
    pub pubkey_index: u8,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl TransactionAccount {
    pub fn to_account_meta(&self, accounts: &[Pubkey]) -> AccountMeta {
        let pubkey = accounts[self.pubkey_index as usize];
        match self.is_writable {
            false => AccountMeta::new_readonly(pubkey, self.is_signer),
            true => AccountMeta::new(pubkey, self.is_signer),
        }
    }
}
