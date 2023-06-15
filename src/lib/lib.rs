use std::path::Path;
use std::fs;
use std::collections::{BTreeMap, HashMap};
use errors::*;

/// A module with error types
pub mod errors;

/// Analysis results of a wasm file
#[derive(Debug)]
#[allow(dead_code)] // source not used in lib
pub struct Analysis {
    source: String,
    operator_usage: BTreeMap<String, u64>,
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
    let mut operator_usage = HashMap::<String, u64>::new();
    let mut operator_count = 0;

    let wasm_bytecodes = fs::read(source)?;
    let mut reader = wasmparser::BinaryReader::new(&wasm_bytecodes);
    while !reader.eof() {
        let opname = match reader.read_operator() {
            Ok(op) => format!("{:?}", op),
            Err(_) => {
                eprintln!("Illegal Operation");
                "IllegalOpCode".into()
            }, // TODO
        };
        operator_usage.entry(opname)
            .and_modify(|count| *count += 1)
            .or_insert(1);
        operator_count += 1;
    }

    let mut vec: Vec<(&String, &u64)> = operator_usage.iter().collect();
    vec.sort_by(|a, b| b.1.cmp(a.1));
    let mut sorted_operator_usage = BTreeMap::<String, u64>::new();
    for (op, count) in vec {
        sorted_operator_usage.insert(op.to_string(), *count);
    }

    Ok(Analysis {
        source: source.canonicalize()?.display().to_string(),
        operator_usage: sorted_operator_usage,
        operator_count,
        file_size: source.metadata()?.len(),
    })
}