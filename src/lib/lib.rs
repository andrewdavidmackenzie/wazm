/// A module with error types
pub mod errors;

/// A module for analyzing a wasm file
pub mod analysis;

/// A Module for compressing ".wasm" files into ".wz"
pub mod compress;

/// A Module for decompressing ".wz" files into ".wasm"
pub mod decompress;

/// A Module to parse a wasm source file
pub mod parse;

pub use analysis::analyze;
pub use parse::Module;
pub use compress::compress;
pub use decompress::decompress;
