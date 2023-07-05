use std::path::Path;
use crate::errors::*;

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn compress(source: &Path, destination: &Path) -> Result<u64> {
    // TODO generate our compressed format
    std::fs::copy(source, destination).chain_err(|| "Could not compress")
}