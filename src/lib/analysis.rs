use std::path::Path;
use crate::errors::*;
use std::collections::HashMap;
use wasmparser::{Parser, FunctionBody, Payload::*};
use wasmparser::ExportSectionReader;
use wasmparser::ImportSectionReader;
use wasmparser::ExternalKind;
use wasmparser::ElementSectionReader;
use wasmparser::ElementItems::*;
use wasmparser::TypeRef;
use wasmparser::RefType;
use wasmparser::Operator;
use core::ops::Range;
use std::fmt;
use std::collections::BTreeMap;
use leb128;

pub struct Section {
    section_type: String,
    header_location: usize,
    item_count: Option<u32>,
    range: Range<usize>,
    size: usize,
}

impl Section {
    fn header(f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Header Start     Content Start    Content End     Size (HEX)    Size    Type               Items")
    }
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.item_count {
            Some(count) => write!(f, "{:#014x} : {:#014x} - {:#014x}{:#10x}{:#10}  {:<18}{:#8}",
                                  self.header_location,
                                  self.range.start,
                                  self.range.end - 1,
                                  self.size,
                                  self.size,
                                  self.section_type,
                                  count),
            None => write!(f, "{:#014} : {:#014x} - {:#014x}{:#10x}{:#10}  {:<18}",
                           "",
                           self.range.start,
                           self.range.end - 1,
                           self.size,
                           self.size,
                           self.section_type),
        }
    }
}

/// Analysis results of a wasm file
#[derive(Default)]
pub struct Analysis {
    pub source: String,
    pub version: u16,
    pub file_size: u64,

    pub include_functions: bool,
    pub implemented_function_count: u64,
    pub imported_functions: BTreeMap<usize, String>,
    pub exported_functions: BTreeMap<usize, String>,

    pub include_function_call_tree: bool,
    pub static_function_calls: HashMap<usize, Vec<usize>>, // index of caller --> vector of indexes called
    pub dynamic_dispatch_functions: Vec<usize>,
    pub include_sections: bool,
    pub sections: Vec<Section>,
    pub sections_size_total: usize,

    pub include_operators: bool,
    pub operator_usage: BTreeMap<String, u64>,
    pub sorted_operator_usage: Vec<(String, u64)>,
    pub operator_count: u64,
}

impl Analysis {
    fn add_elements(&mut self, elements_reader: ElementSectionReader)
                 -> Result<()> {
        self.add_section("ElementSection", Some(elements_reader.count()), &elements_reader.range())?;

        for element in elements_reader.clone().into_iter().flatten() {
            if element.ty == RefType::FUNCREF || element.ty == RefType::FUNC {
                if let Functions(section) = element.items {
                    self.dynamic_dispatch_functions = section.into_iter()
                        .map(|e| e.unwrap() as usize)
                        .collect::<Vec<usize>>();
                    self.dynamic_dispatch_functions.sort();
                    self.dynamic_dispatch_functions.dedup();
                }
            }
        }

        Ok(())
    }

    fn add_section(&mut self, section_type: &str, item_count: Option<u32>, range: &Range<usize>)
    -> Result<()> {
        if self.include_sections {
            let size = range.end - range.start;
            self.sections_size_total += size;

            let section_header_size = if section_type.starts_with("Magic") {
                0
            } else {
                let mut buf = [0; 4]; // LEB128 encoding of u32 should not exceed 4 bytes
                let mut writable = &mut buf[..];
                leb128::write::unsigned(&mut writable, size as u64)
                    .expect("Could not encode in LEB128") + 1  // one byte for section type
            };

            self.sections_size_total += section_header_size;

            self.sections.push(
                Section {
                    header_location: range.start - section_header_size,
                    section_type: section_type.to_owned(),
                    item_count,
                    range: range.clone(),
                    size,
                }
            );
        }

        Ok(())
    }

    fn add_function_call(&mut self, caller_index: usize, called_index: usize) {
        self.static_function_calls.entry(caller_index)
            .and_modify(|v| { if !v.contains(&called_index) { v.push(called_index) } })
            .or_insert(vec!());
    }

    fn add_function(&mut self, function_body: &FunctionBody, index: usize) -> Result<()> {
        if !self.include_functions {
            return Ok(());
        }

        let mut reader = function_body.get_operators_reader()?;
        while !reader.eof() {
            let operator = reader.read()?;

            if let Operator::Call{function_index} = operator {
                self.add_function_call(index, function_index as usize);
            }

            if self.include_operators {
                let opname = format!("{:?}", operator).split_whitespace().next().unwrap_or("")
                    .to_string();
                self.operator_usage.entry(opname)
                    .and_modify(|count| *count += 1)
                    .or_insert(1);
                self.operator_count += 1;
            }
        }

        self.implemented_function_count += 1;

        Ok(())
    }

    fn add_exports(&mut self, reader: &ExportSectionReader) -> Result<()> {
        self.add_section("ExportSection", Some(reader.count()), &reader.range())?;

        if self.include_functions {
            for export in reader.clone().into_iter().flatten() {
                if export.kind == ExternalKind::Func {
                    self.exported_functions.insert(export.index as usize, export.name.to_owned());
                }
            }
        }

        Ok(())
    }

    fn add_imports(&mut self, reader: &ImportSectionReader, function_index: &mut usize) -> Result<()> {
        self.add_section("ImportSection", Some(reader.count()), &reader.range())?;

        if self.include_functions {
            for import in reader.clone().into_iter().flatten() {
                if matches!(import.ty, TypeRef::Func(_)) {
                    self.imported_functions.insert(*function_index, import.name.to_owned());
                    *function_index += 1;
                }
            }
        }

        Ok(())
    }

    fn post_process(&mut self) {
        // order the operator usage
        let mut vec: Vec<(String, u64)> = self.operator_usage.iter()
            .map(|(s, c)| (s.to_string(), *c)).collect();
        vec.sort_by(|a, b| b.1.cmp(&a.1));
        self.sorted_operator_usage = vec;
    }

    fn print_called_list(&self, call_chain: Vec<usize>, f: &mut fmt::Formatter) -> fmt::Result {
        let index = call_chain.last().unwrap_or(&1);
        if let Some(called_list) = self.static_function_calls.get(index) {
            let level = call_chain.len();
            for called in called_list {
                if call_chain.contains(called) {
                    writeln!(f, "     {}+- #{} Cyclic call", format_args!("{: >1$}", "", level * 3), called)?;
                } else {
                    writeln!(f, "     {}+- #{}", format_args!("{: >1$}", "", level * 3), called)?;
                    let mut new_chain = call_chain.clone();
                    new_chain.push(*called);
                    self.print_called_list(new_chain, f)?;
                }
            }
        }
        Ok(())
    }

    fn print_call_tree(&self, root_index: &usize, name: &str, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "\t#{} '{}'", root_index, name)?;
        self.print_called_list(vec!(*root_index), f)?;
        writeln!(f)
    }
}

impl fmt::Display for Analysis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "WASM File: {}", self.source)?;
        writeln!(f, "WASM Version: {}", self.version)?;
        writeln!(f, "File Size: {}", self.file_size)?;

        if self.include_sections {
            writeln!(f, "\nSections:")?;
            Section::header(f)?;
            for section in &self.sections {
                writeln!(f, "{}", section)?;
            }

            writeln!(f, "Total Size: {}", self.sections_size_total)?;
            let unaccounted_for = self.file_size - self.sections_size_total as u64;
            if unaccounted_for != 0 {
                writeln!(f, "Bytes unaccounted for: {}", unaccounted_for)?;
            }
        }

        if self.include_functions {
            writeln!(f, "\nFunctions:")?;
            writeln!(f, "Imported Functions ({}):", self.imported_functions.len())?;
            for (function_index, import_name) in &self.imported_functions {
                writeln!(f, " {:#5} '{}'", function_index, import_name)?;
            }
            writeln!(f, "Implemented Functions ({}):", self.implemented_function_count)?;
            writeln!(f, "Exported Functions ({}):", self.exported_functions.len())?;

            for (function_index, export_name) in &self.exported_functions {
                writeln!(f, " {:#5} '{}'", function_index, export_name)?;
            }

            let mut called_functions = vec!();
            for called_list in self.static_function_calls.values() {
                called_functions.extend(called_list);
            }
            called_functions.sort();
            called_functions.dedup();
            if !called_functions.is_empty() {
                writeln!(f, "\nStatically Called Functions ({}): {:?}",
                         called_functions.len(), called_functions)?;
            }

            if !self.dynamic_dispatch_functions.is_empty() {
                let mut dynamic = self.dynamic_dispatch_functions.clone();
                dynamic.sort();
                writeln!(f, "\nDynamic Dispatch Functions ({}): {:?}",
                         dynamic.len(), dynamic)?;
            }

            let mut all_functions: Vec<usize> = (0..self.implemented_function_count)
                .map(|e| e as usize ).collect();
            // Remove all functions that have been called by others
            all_functions.retain(|e| {
                !called_functions.contains(e)
            });
            // Remove all imported functions
            all_functions.retain(|e| {
                !self.imported_functions.contains_key(e)
            });
            // Remove all exported functions
            all_functions.retain(|e| {
                !self.exported_functions.contains_key(e)
            });
            // Remove functions that maybe called dynamically at runtime via a table
            all_functions.retain(|e| {
                !self.dynamic_dispatch_functions.contains(e)
            });
            if !all_functions.is_empty() {
                all_functions.sort();
                writeln!(f, "\nUncalled Functions ({}):", all_functions.len())?;
                writeln!(f, "{:?}", all_functions)?;
            }

            if self.include_function_call_tree {
                writeln!(f, "\nFunction Call Tree:")?;
                for index in self.static_function_calls.keys() {
                    if let Some(name) = self.exported_functions.get(index) {
                        self.print_call_tree(index, name, f)?;
                    }
                }
            }

            if self.include_operators {
                writeln!(f, "\nOperators:")?;
                writeln!(f, "Operators Count: {}", self.operator_count)?;
                writeln!(f, "Operator Usage:")?;
                writeln!(f, "\tOperator             Count")?;
                for (opname, count) in &self.sorted_operator_usage {
                    writeln!(f, "\t{:#018}{:#8}", opname, count)?;
                }
            }
        }

        Ok(())
    }
}

/// Analyze the file at `source` to see what sections it has and operators it uses
pub fn analyze(source: &Path,
               include_sections: bool,
               include_functions: bool,
               include_operators: bool,
               include_function_call_tree: bool,
) -> Result<Analysis> {
    let mut analysis = Analysis {
        source: source.canonicalize()?.display().to_string(),
        file_size: source.metadata()?.len(),
        include_sections,
        include_functions,
        include_operators,
        include_function_call_tree,
        ..Default::default() };

    let mut function_index = 0;

    let buf: Vec<u8> = std::fs::read(source)?;
    for payload in Parser::new(0).parse_all(&buf) {
        #[allow(unused_variables)]
        match payload? {
            // Here we know how many functions we'll be receiving as
            // `CodeSectionEntry`, so we can prepare for that, and
            // afterwards we can parse and handle each function
            // individually.
            CodeSectionStart { count, range, size } =>
                analysis.add_section("CodeSectionStart", Some(count), &range)?,
            CodeSectionEntry(function_body) => {
                analysis.add_function(&function_body, function_index)?;
                function_index += 1;
            },
            ComponentSection { parser, range } =>
                analysis.add_section("ComponentSection", None, &range)?,
            ComponentInstanceSection(section) =>
                analysis.add_section("ComponentInstanceSection", None, &section.range())?,
            ComponentAliasSection(section) =>
                analysis.add_section("ComponentAliasSection", None, &section.range())?,
            ComponentTypeSection(section) =>
                analysis.add_section("ComponentTypeSection", None, &section.range())?,
            ComponentCanonicalSection(section) =>
                analysis.add_section("ComponentCanonicalSection", None, &section.range())?,
            ComponentStartSection { start, range } =>
                analysis.add_section("ComponentStartSection", None, &range)?,
            ComponentImportSection(section) =>
                analysis.add_section("ComponentImportSection", None, &section.range())?,
            ComponentExportSection(section) =>
                analysis.add_section("ComponentExportSection", None, &section.range())?,
            CoreTypeSection(section) =>
                analysis.add_section("CoreTypeSection", None, &section.range())?,
            CustomSection(section) =>
                analysis.add_section("CustomSection", None, &section.range())?,
            DataCountSection { count, range } =>
                analysis.add_section("DataCountSection", Some(count), &range)?,
            DataSection(section) =>
                analysis.add_section("DataSection", Some(section.count()), &section.range())?,
            ElementSection(section) => analysis.add_elements(section)?,
            ExportSection(section) => analysis.add_exports(&section)?,
            FunctionSection(section) =>
                analysis.add_section("FunctionSection", Some(section.count()), &section.range())?,
            GlobalSection(section) =>
                analysis.add_section("GlobalSection", Some(section.count()), &section.range())?,
            ImportSection(section) => analysis.add_imports(&section, &mut function_index)?,
            InstanceSection(section) =>
                analysis.add_section("InstanceSection", Some(section.count()), &section.range())?,
            MemorySection(section) =>
                analysis.add_section("MemorySection", Some(section.count()), &section.range())?,
            ModuleSection { parser, range } =>
                analysis.add_section("ModuleSection", None, &range)?,
            StartSection { func, range } =>
                analysis.add_section("StartSection", None, &range)?,
            TableSection(section) =>
                analysis.add_section("TableSection", Some(section.count()), &section.range())?,
            TagSection(section) =>
                analysis.add_section("TagSection", Some(section.count()), &section.range())?,
            TypeSection(section) =>
                analysis.add_section("TypeSection", Some(section.count()), &section.range())?,
            UnknownSection { id, contents, range } =>
                analysis.add_section("UnknownSection", None, &range)?,
            Version { num, encoding, range } => {
                analysis.version = num;
                analysis.add_section("Magic & Version", None, &range)?;
            }
            End(_) => {}
        }
    }

    analysis.post_process();

    Ok(analysis)
}

#[cfg(test)]
mod test {
    use std::fs;
    use std::process::Command;
    use std::path::PathBuf;

    fn test_file(test_file_name: &str) -> PathBuf {
        let source = PathBuf::from(&format!("{}/tests/test_files/{}",
                                            env!("CARGO_MANIFEST_DIR"),
        test_file_name));
        let mut wasm = source.clone();
        wasm.set_extension("wasm");
        let _ = fs::remove_file(&wasm);
        let mut compiler = Command::new("wat2wasm");
        compiler.arg(source);
        compiler.arg("-o");
        compiler.arg(&wasm);
        compiler.output().expect("wat2wasm compile failed");
        wasm
    }

    #[test]
    fn test_analyze_hello_web() {
        let wasm = test_file("hello_web.wat");
        let analysis = super::analyze(&wasm, true, true, true, true)
            .expect("Analysis of wasm file failed");
        assert_eq!(analysis.version, 1);
        assert_eq!(analysis.exported_functions.len(), 1);
        assert_eq!(analysis.implemented_function_count, 2);
        let _ = fs::remove_file(&wasm);
    }
}