#[cfg(feature = "domichain")]
use domichain_program;
#[cfg(feature = "solana")]
use solana_program as domichain_program;

mod error;
mod instruction;
mod processor;
mod state;
mod utils;
mod client;

pub use self::error::*;
pub use self::instruction::*;
pub use self::processor::*;
pub use self::state::*;
pub use self::client::*;

#[cfg(feature = "wasm")]
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
extern crate wasm_bindgen;

#[cfg(feature = "wasm")]
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub mod wasm;

#[cfg(feature = "bindings")]
mod bindings;

#[cfg(feature = "bindings")]
pub use self::bindings::*;

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

domichain_program::declare_id!("msigLK5Pz1XD5GcTTSx3JNik7NTx8JbfbD8eF5sw89h");
