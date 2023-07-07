use crate::errors::*;
use std::collections::HashMap;
use wasmparser::{FunctionBody, Payload::*};
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
use std::ops::RangeInclusive;
use leb128;

use crate::Module;

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

/// Analysis results of a wasm module
#[derive(Default)]
pub struct Analysis {
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
    fn track_size(&mut self, section_type: &str, range: &Range<usize>) -> Result<usize> {
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

        Ok(section_header_size)
    }

    fn add_section(&mut self, section_type: &str, item_count: Option<u32>, range: &Range<usize>)
                   -> Result<()> {
        if self.include_sections {
            let header_size = self.track_size(section_type, range)?;
            let section = Section {
                header_location: range.start - header_size,
                section_type: section_type.to_owned(),
                item_count,
                range: range.clone(),
                size: range.end - range.start,
            };
            self.sections.push(section);
        }

        Ok(())
    }

    fn add_elements(&mut self, elements_reader: &ElementSectionReader) -> Result<()> {
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

    fn add_function_call(&mut self, caller_index: usize, called_index: usize) {
        self.static_function_calls.entry(caller_index)
            .and_modify(|v| { if !v.contains(&called_index) { v.push(called_index) } })
            .or_insert(vec!());
    }

    fn add_function(&mut self, function_body: &FunctionBody, index: &mut usize) -> Result<()> {
        if !self.include_functions {
            return Ok(());
        }

        let mut reader = function_body.get_operators_reader()?;
        while !reader.eof() {
            let operator = reader.read()?;

            if let Operator::Call{function_index} = operator {
                self.add_function_call(*index, function_index as usize);
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

        *index += 1;

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

#[derive(PartialEq, Debug, Default)]
struct RangeVec(Vec<RangeVecEntry>);
#[derive(PartialEq, Debug)]
enum RangeVecEntry {
    RangeEntry(RangeInclusive<usize>),
    SingleEntry(usize)
}

impl fmt::Display for RangeVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[")?;
        for entry in &self.0[0..self.0.len() -1] {
            match entry {
                RangeVecEntry::RangeEntry(range) =>write!(f, "{}..{}, ",
                                                          range.start(), range.end())?,
                RangeVecEntry::SingleEntry(number) => write!(f, "{}, ", number)?,
            }
        }
        match &self.0[self.0.len()-1] {
            RangeVecEntry::RangeEntry(range) =>write!(f, "{}..{}",
                                                      range.start(), range.end())?,
            RangeVecEntry::SingleEntry(number) => write!(f, "{}", number)?,
        }
        write!(f, "]")
    }
}

// assumes the input vector is already ordered
impl From<&Vec<usize>> for RangeVec {
    fn from(input: &Vec<usize>) -> Self {
        let mut output: RangeVec = RangeVec::default();
        let mut start = input[0];
        let mut end = input[0];
        for i in input[1..].iter() {
            if *i != end + 1 {
                if start == end {
                    output.0.push(RangeVecEntry::SingleEntry(end));
                } else {
                    output.0.push(RangeVecEntry::RangeEntry(start..=end));
                }
                start = *i;
                end = *i;
            } else {
                end = *i;
            }
        }

        if start == end {
            output.0.push(RangeVecEntry::SingleEntry(end));
        } else {
            output.0.push(RangeVecEntry::RangeEntry(start..=end));
        }

        output
    }
}

impl fmt::Display for Analysis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.include_sections {
            writeln!(f, "Sections:")?;
            Section::header(f)?;
            for section in &self.sections {
                writeln!(f, "{}", section)?;
            }

            writeln!(f, "Total Size: {}", self.sections_size_total)?;
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
                writeln!(f, "\nStatically Called Functions ({}): {}",
                         called_functions.len(), RangeVec::from(&called_functions))?;
            }

            if !self.dynamic_dispatch_functions.is_empty() {
                let mut dynamic = self.dynamic_dispatch_functions.clone();
                dynamic.sort();
                writeln!(f, "\nDynamic Dispatch Functions ({}): {}",
                         dynamic.len(), RangeVec::from(&dynamic))?;
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
                writeln!(f, "\nUncalled Functions ({}): {}", all_functions.len(),
                         RangeVec::from(&all_functions))?;
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

/// Analyze the parsed [Module] to see what sections it has and operators it uses
pub fn analyze(module: &Module,
               include_sections: bool,
               include_functions: bool,
               include_operators: bool,
               include_function_call_tree: bool,
) -> Result<Analysis> {
    let mut analysis = Analysis {
        include_sections,
        include_functions,
        include_operators,
        include_function_call_tree,
        ..Default::default() };

    let mut function_index = 0;
    for payload in &module.sections {
        #[allow(unused_variables)]
        match payload {
            CodeSectionStart { count, range, size } =>
                analysis.add_section("CodeSectionStart", Some(*count), range)?,
            CodeSectionEntry(function_body) => analysis.add_function(function_body,
                                                                     &mut function_index)?,
            ComponentSection { parser, range } =>
                analysis.add_section("ComponentSection", None, range)?,
            ComponentInstanceSection(section) =>
                analysis.add_section("ComponentInstanceSection", None, &section.range())?,
            ComponentAliasSection(section) =>
                analysis.add_section("ComponentAliasSection", None, &section.range())?,
            ComponentTypeSection(section) =>
                analysis.add_section("ComponentTypeSection", None, &section.range())?,
            ComponentCanonicalSection(section) =>
                analysis.add_section("ComponentCanonicalSection", None, &section.range())?,
            ComponentStartSection { start, range } =>
                analysis.add_section("ComponentStartSection", None, range)?,
            ComponentImportSection(section) =>
                analysis.add_section("ComponentImportSection", None, &section.range())?,
            ComponentExportSection(section) =>
                analysis.add_section("ComponentExportSection", None, &section.range())?,
            CoreTypeSection(section) =>
                analysis.add_section("CoreTypeSection", None, &section.range())?,
            CustomSection(section) =>
                analysis.add_section("CustomSection", None, &section.range())?,
            DataCountSection { count, range } =>
                analysis.add_section("DataCountSection", Some(*count), range)?,
            DataSection(section) =>
                analysis.add_section("DataSection", Some(section.count()), &section.range())?,
            ElementSection(reader) => analysis.add_elements(reader)?,
            ExportSection(reader) => analysis.add_exports(reader)?,
            FunctionSection(section) =>
                analysis.add_section("FunctionSection", Some(section.count()), &section.range())?,
            GlobalSection(section) =>
                analysis.add_section("GlobalSection", Some(section.count()), &section.range())?,
            ImportSection(reader) => analysis.add_imports(reader, &mut function_index)?,
            InstanceSection(section) =>
                analysis.add_section("InstanceSection", Some(section.count()), &section.range())?,
            MemorySection(section) =>
                analysis.add_section("MemorySection", Some(section.count()), &section.range())?,
            ModuleSection { parser, range } =>
                analysis.add_section("ModuleSection", None, range)?,
            StartSection { func, range } =>
                analysis.add_section("StartSection", None, range)?,
            TableSection(section) =>
                analysis.add_section("TableSection", Some(section.count()), &section.range())?,
            TagSection(section) =>
                analysis.add_section("TagSection", Some(section.count()), &section.range())?,
            TypeSection(section) =>
                analysis.add_section("TypeSection", Some(section.count()), &section.range())?,
            UnknownSection { id, contents, range } =>
                analysis.add_section("UnknownSection", None, range)?,
            Version { num, encoding, range } =>
                analysis.sections_size_total += 8,
            End(_) => bail!("End section should have been parsed out prior to analysis"),
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
    use crate::analysis::RangeVec;
    use crate::analysis::RangeVecEntry::{RangeEntry, SingleEntry};

    #[test]
    fn test_to_ranges() {
        let ranges = RangeVec::from(&vec!(1, 2, 4, 5, 7, 9, 10));
        assert_eq!(ranges, RangeVec(vec!(RangeEntry(1..=2),
                                         RangeEntry(4..=5),
                                         SingleEntry(7),
                                         RangeEntry(9..=10))));
    }

    #[test]
    fn test_to_ranges_end_single() {
        let ranges = RangeVec::from(&vec!(1, 2, 4, 5, 7, 9));
        assert_eq!(ranges, RangeVec(vec!(RangeEntry(1..=2),
                                         RangeEntry(4..=5),
                                         SingleEntry(7),
                                         SingleEntry(9))));
    }

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
        let buf: Vec<u8> = fs::read(&wasm).expect("Could not read wasm file");
        let module = super::Module::parse(&wasm, &buf).expect("Could not parse test wasm");
        assert_eq!(module.version, 1);
        let analysis = super::analyze(&module, true, true, true, true)
            .expect("Analysis of wasm file failed");
        assert_eq!(analysis.exported_functions.len(), 1);
        assert_eq!(analysis.implemented_function_count, 2);
        let _ = fs::remove_file(&wasm);
    }
}