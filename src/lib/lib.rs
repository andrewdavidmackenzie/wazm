use std::path::Path;
use errors::*;

/// A module with error types
pub mod errors;

// A module for analyzing a wasm file
pub mod analysis;

pub use analysis::analyze;

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn compress(source: &Path, destination: &Path) -> Result<u64> {
    std::fs::copy(source, destination).chain_err(|| "Could not compress")
}
