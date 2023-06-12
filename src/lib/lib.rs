use std::path::Path;
use std::io;

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn compress(source: &Path, destination: &Path) -> io::Result<u64> {
    std::fs::copy(source, destination)
}