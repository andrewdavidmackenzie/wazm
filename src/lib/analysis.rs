use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use crate::errors::*;
use std::io::Read;
use std::collections::HashMap;
use wasmparser::{Parser, Chunk, FunctionBody, Payload::*};
use wasmparser::ExportSectionReader;
use wasmparser::ExternalKind;
use wasmparser::Operator;
use core::ops::Range;
use std::fmt;
use std::collections::BTreeMap;
use leb128;

struct Section {
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
pub struct Analysis {
    source: String,
    version: u16,
    file_size: u64,

    include_functions: bool,
    function_count: u64,
    exported_functions_count: u32,
    exported_functions: HashMap<usize, String>,

    include_function_call_tree: bool,
    function_call_list: HashMap<usize, Vec<usize>>, // index of caller --> vector of indexes called

    include_sections: bool,
    sections: Vec<Section>,
    sections_size_total: usize,

    include_operators: bool,
    operator_usage: BTreeMap<String, u64>,
    sorted_operator_usage: Vec<(String, u64)>,
    operator_count: u64,
}

impl Analysis {
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

    fn add_function_call(&mut self, caller_index: usize, function_index: u32) {
        if self.include_function_call_tree {
            let called_index = function_index as usize;
            self.function_call_list.entry(caller_index)
                .and_modify(|v| { if !v.contains(&called_index) { v.push(called_index) } })
                .or_insert(vec!());
        }
    }

    fn add_function(&mut self, function_body: &FunctionBody, index: usize) -> Result<()> {
        if !self.include_functions {
            return Ok(());
        }

        let mut found_return = false;
        let mut reader = function_body.get_operators_reader()?;
        while !reader.eof() {
            let operator = reader.read()?;

            match operator {
                Operator::Return => found_return = true,
                Operator::Call{function_index} => self.add_function_call(index, function_index),
                _ => {}
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

        if !found_return {
           // TODO println!("Function did not contain Return");
        }

        self.function_count += 1;

        Ok(())
    }

    fn add_exports(&mut self, reader: &ExportSectionReader) -> Result<()> {
        if self.include_functions {
            for export in reader.clone().into_iter() {
                if let Ok(ex) = export {
                    if ex.kind == ExternalKind::Func {
                        self.exported_functions.insert(ex.index as usize, ex.name.to_owned());
                        self.exported_functions_count += 1;
                    }
                }
            }
        }

        Ok(())
    }

    fn print_called_list(&self, call_chain: Vec<usize>, f: &mut fmt::Formatter) -> fmt::Result {
        let index = call_chain.last().unwrap_or(&1);
        if let Some(called_list) = self.function_call_list.get(index) {
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
        writeln!(f, "")
    }
}

impl fmt::Display for Analysis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "File Stats:")?;
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
            writeln!(f, "Function Count: {}", self.function_count)?;
            writeln!(f, "Exported Function Count: {}", self.exported_functions_count)?;

            for (function_index, export_name) in &self.exported_functions {
                writeln!(f, "\t Exported Function: {:#5} '{}'", function_index, export_name)?;
            }

            if self.include_function_call_tree {
                writeln!(f, "\nFunction Call Tree:")?;
                for index in self.function_call_list.keys() {
                    if let Some(name) = self.exported_functions.get(index) {
                        self.print_call_tree(index, name, f)?;
                    }
                }
                writeln!(f, "Uncalled Functions:")?;
                let mut all_functions: Vec<usize> = (0..self.function_count)
                    .map(|e| e as usize ).collect();
                let called_functions = self.function_call_list.keys().cloned().collect::<Vec<usize>>();
                all_functions.retain(|e| {
                    !called_functions.contains(&e)
                });
                if !all_functions.is_empty() {
                    writeln!(f, "{:?}", all_functions)?;
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
    let f = File::open(source)?;
    let mut reader = BufReader::new(f);
    let mut buf = Vec::new();
    let mut parser = Parser::new(0);
    let mut eof = false;
    let mut stack = Vec::new();

    let mut analysis = Analysis {
        source: source.canonicalize()?.display().to_string(),
        file_size: source.metadata()?.len(),
        version: 0,

        include_functions,
        function_count: 0,
        exported_functions_count: 0,
        exported_functions: HashMap::<usize, String>::new(),

        include_function_call_tree,
        function_call_list: HashMap::<usize, Vec<usize>>::new(),

        include_sections,
        sections: Vec::new(),
        sections_size_total: 0,

        include_operators,
        operator_usage: BTreeMap::<String, u64>::new(),
        sorted_operator_usage: vec!(),
        operator_count: 0,
    };

    let mut function_index = 0;

    loop {
        let (payload, consumed) = match parser.parse(&buf, eof)? {
            Chunk::NeedMoreData(hint) => {
                assert!(!eof); // otherwise an error would be returned

                // Use the hint to preallocate more space, then read
                // some more data into our buffer.
                //
                // Note that the buffer management here is not ideal,
                // but it's compact enough to fit in an example!
                let len = buf.len();
                buf.extend((0..hint).map(|_| 0u8));
                let n = reader.read(&mut buf[len..])?;
                buf.truncate(len + n);
                eof = n == 0;
                continue;
            }

            Chunk::Parsed { consumed, payload } => (payload, consumed),
        };

        #[allow(unused_variables)]
        match payload {
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
            ElementSection(section) =>
                analysis.add_section("ElementSection", Some(section.count()), &section.range())?,
            ExportSection(section) => {
                analysis.add_section("ExportSection", Some(section.count()), &section.range())?;
                analysis.add_exports(&section)?;
            }
            FunctionSection(section) =>
                analysis.add_section("FunctionSection", Some(section.count()), &section.range())?,
            GlobalSection(section) =>
                analysis.add_section("GlobalSection", Some(section.count()), &section.range())?,
            ImportSection(section) =>
                analysis.add_section("ImportSection", Some(section.count()), &section.range())?,
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

            // Once we've reached the end of a parser we either resume
            // at the parent parser or we break out of the loop because
            // we're done.
            End(_) => {
                if let Some(parent_parser) = stack.pop() {
                    parser = parent_parser;
                } else {
                    break;
                }
            }
        }

        // once we're done processing the payload we can forget the
        // original.
        buf.drain(..consumed);

        // analyze function call graph (if requested) starting at "exported" entry points
        /*
                if let Some(name) = self.exported_functions.get(&index) {
            println!("Analyzing exported function: {} with index {}", name, index);
        }

         */
    }

    // order the operator usage
    let mut vec: Vec<(String, u64)> = analysis.operator_usage.iter()
        .map(|(s, c)| (s.to_string(), *c)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1));
    analysis.sorted_operator_usage = vec;

    Ok(analysis)
}