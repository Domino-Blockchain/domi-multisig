#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "solana")]
use solana_program as domichain_program;

use borsh::{BorshDeserialize, BorshSerialize};
use domichain_program::pubkey::Pubkey;

use crate::TransactionInstruction;

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum MultisigInstruction {
    /// Initializes a new multisig account with a set of owners and a threshold
    ///
    /// # Account references
    /// ...
    CreateMultisig {
        seed: u128,
        owners: Vec<Pubkey>,
        threshold: u64,
    },

    /// Add a new account to custodian list
    ///
    /// # Account references
    /// ...
    AddOwner { owner: Pubkey },

    /// Delete account from custodian list
    ///
    /// # Account references
    /// ...
    DeleteOwner { owner: Pubkey },

    /// Update threshold
    ///
    /// # Account references
    /// ...
    UpdateThreshold { threshold: u64 },

    /// Creates a new transaction account, automatically signed by the creator,
    /// which must be one of the owners of the multisig
    ///
    /// # Account references
    /// ...
    CreateTransaction {
        seed: u128,
        accounts: Vec<Pubkey>,
        instructions: Vec<TransactionInstruction>,
    },

    /// Approves a transaction on behalf of an owner of the multisig
    ///
    /// # Account references
    /// ...
    Approve,

    /// Execute transaction
    ///
    /// # Account references
    /// ...
    ExecuteTransaction,

    /// Delete pending transaction
    ///
    /// # Account references
    /// ...
    DeletePendingTransaction { pending_transaction: Pubkey },
}
