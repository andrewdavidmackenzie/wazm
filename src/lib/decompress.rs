use std::fs;
use std::io::Write;
use std::path::Path;
use crate::errors::*;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
    Module, TypeSection, ValType,
};

/// Decompress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn decompress(_source: &Path, destination: &Path) -> Result<u64> {
    // TODO parse our compressed format

    let mut module = Module::new();

    // Encode the type section.
    let mut types = TypeSection::new();
    let params = vec![ValType::I32, ValType::I32];
    let results = vec![ValType::I32];
    types.function(params, results);
    module.section(&types);

    // Encode the function section.
    let mut functions = FunctionSection::new();
    let type_index = 0;
    functions.function(type_index);
    module.section(&functions);

    // Encode the export section.
    let mut exports = ExportSection::new();
    exports.export("f", ExportKind::Func, 0);
    module.section(&exports);

    // Encode the code section.
    let mut codes = CodeSection::new();
    let locals = vec![];
    let mut f = Function::new(locals);
    f.instruction(&Instruction::LocalGet(0));
    f.instruction(&Instruction::LocalGet(1));
    f.instruction(&Instruction::I32Add);
    f.instruction(&Instruction::End);
    codes.function(&f);
    module.section(&codes);

    // Extract the encoded Wasm bytes for this module.
    let wasm_bytes = module.finish();

    // We generated a valid Wasm module!
    assert!(wasmparser::validate(&wasm_bytes).is_ok());

    let mut file = fs::File::create(destination)?;
    file.write_all(&wasm_bytes)?;

    Ok(wasm_bytes.len() as u64)
}
