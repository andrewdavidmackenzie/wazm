use std::path::Path;
use std::fs;
use std::collections::HashMap;
use errors::*;

/// A module with error types
pub mod errors;

/// Analysis results of a wasm file
#[derive(Debug)]
#[allow(dead_code)] // source not used in lib
pub struct Analysis {
    source: String,
    operator_usage: HashMap<String, u64>,
    operator_count: u64,
    file_size: u64,
}

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn compress(source: &Path, destination: &Path) -> Result<u64> {
    std::fs::copy(source, destination).chain_err(|| "Could not compress")
}

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn analyze(source: &Path) -> Result<Analysis> {
    let mut opcode_usage = HashMap::<String, u64>::new();
    let mut operator_count = 0;

    let wasm_bytecodes = fs::read(source)?;
    let mut reader = wasmparser::BinaryReader::new(&wasm_bytecodes);
    while !reader.eof() {
        let opname = match reader.read_operator() {
            Ok(op) => format!("{:?}", op),
            Err(_) => "IllegalOpCode".into(), // TODO
        };
        opcode_usage.entry(opname)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        operator_count += 1;
    }

    Ok(Analysis {
        source: source.display().to_string(),
        operator_usage: opcode_usage,
        operator_count,
        file_size: source.metadata().unwrap().len(),
    })
}