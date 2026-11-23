//! The STREAM kernel.
//!
//! Everything needed to read a log from its first line and nothing else:
//! the entry grammar, canonical JSON, decimals, the 64 verbs and the phase,
//! addresses, hashing and the seal rule. It has no operating-system
//! dependency (`no_std` + `alloc`) so the same code runs on servers, in the
//! browser (WebAssembly) and on devices.
//!
//! The normative text is `spec/kernel.descriptor`; its SHA-256 is the
//! kernel's identity ([`kernel_hash`]).

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod decimal;
pub mod entry;
pub mod hash;
pub mod json;
pub mod verb;

pub use decimal::Decimal;
pub use entry::{Address, Entry, KernelError, Time};
pub use hash::{hex, sha256, sha256_hex};
pub use json::{Object, Value};
pub use verb::{Phase, Type, Verb};

/// The kernel descriptor, byte for byte.
pub const DESCRIPTOR: &[u8] = include_bytes!("../../../spec/kernel.descriptor");

/// SHA-256 of the descriptor, lowercase hex: the kernel's identity.
pub fn kernel_hash() -> alloc::string::String {
    sha256_hex(DESCRIPTOR)
}

/// The first line of every log written under this kernel.
pub fn preamble() -> alloc::string::String {
    alloc::format!("STREAM/1 kernel=sha256:{}", kernel_hash())
}

/// Read a preamble line and return the kernel hash it names.
pub fn parse_preamble(line: &str) -> Result<&str, KernelError> {
    let h = line
        .strip_prefix("STREAM/1 kernel=sha256:")
        .ok_or(KernelError::Preamble)?;
    if h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(h)
    } else {
        Err(KernelError::Preamble)
    }
}

/// Chain rule for seals: sha256(prev_chain_hex || body_hash_hex).
pub fn chain(prev_hex: &str, body_hex: &str) -> alloc::string::String {
    let mut buf = alloc::vec::Vec::with_capacity(128);
    buf.extend_from_slice(prev_hex.as_bytes());
    buf.extend_from_slice(body_hex.as_bytes());
    sha256_hex(&buf)
}

/// The chain value before the first seal.
pub const GENESIS_CHAIN: &str = "0000000000000000000000000000000000000000000000000000000000000000";
