use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use crate::errors::*;
use std::io::Read;
use wasmparser::{Parser, Chunk, FunctionBody, Payload::*};
use core::ops::Range;
use std::fmt;
use std::collections::BTreeMap;
use leb128;

struct Section {
    section_type: String,
    item_count: Option<u32>,
    range: Range<usize>,
    size: usize,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.item_count {
            Some(count) => write!(f, "{:#010x} - {:#010x} {:#10x} {:#10} \t\t {:#18} {}",
                                  self.range.start,
                                  self.range.end - 1,
                                  self.size,
                                  self.size,
                                  self.section_type,
                                  count),
            None => write!(f, "{:#010x} - {:#010x} {:#10x} {:#10} \t\t {:#18}",
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

    include_sections: bool,
    sections: Vec<Section>,
    function_count: u64,
    sections_size_total: usize,

    include_operators: bool,
    operator_usage: BTreeMap<String, u64>,
    sorted_operator_usage: Vec<(String, u64)>,
    operator_count: u64,
}

impl Analysis {
    fn add_section(&mut self, section_type: &str, item_count: Option<u32>, range: &Range<usize>) {
        let size = range.end - range.start;
        self.sections_size_total += size;

        if !section_type.starts_with("Magic") {
            let mut buf = [0; 4]; // LEB128 encoding of u32 should not exceed 4 bytes
            let mut writable = &mut buf[..];
            let section_header_size = leb128::write::unsigned(&mut writable, size as u64)
                .expect("Could not encode in LEB128") + 1;
            self.sections_size_total += section_header_size; // one byte for section type
        }

        self.sections.push(
            Section {
                section_type: section_type.to_owned(),
                item_count,
                range: range.clone(),
                size,
            }
        );
    }

    // TODO here we can iterate over `body` to parse the function
// and its locals
// get_binary_reader();
// get_locals_reader();
    fn add_function(&mut self, function_body: &FunctionBody) -> Result<()> {
        let mut reader = function_body.get_operators_reader()?;
        while !reader.eof() {
            let operator = reader.read()?;
            let opname = format!("{:?}", operator).split_whitespace().next().unwrap_or("")
                .to_string();
            self.operator_usage.entry(opname)
                .and_modify(|count| *count += 1)
                .or_insert(1);
            self.operator_count += 1;
        }
        self.function_count += 1;

        Ok(())
    }
}

impl fmt::Display for Analysis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "WASM File: {}", self.source)?;
        writeln!(f, "WASM Version: {}", self.version)?;
        writeln!(f, "Operators Count: {}", self.operator_count)?;
        writeln!(f, "Function Count: {}", self.function_count)?;
        writeln!(f, "File Size on Disk: {}", self.file_size)?;

        if self.include_sections {
            writeln!(f, "\nSections:")?;
            writeln!(f, "   Start        End       Size (HEX)  Size (Dec)         Type               Item Count")?;
            for section in &self.sections {
                writeln!(f, "{}", section)?;
            }

            writeln!(f, "Sections Size Total: {}", self.sections_size_total)?;
            let unaccounted_for = self.file_size - self.sections_size_total as u64;
            if unaccounted_for != 0 {
                writeln!(f, "Bytes unaccounted for: {}", unaccounted_for)?;
            }
        }

        if self.include_operators {
            writeln!(f, "\nOperator Usage:")?;
            writeln!(f, "\tOperator             Count")?;
            for (opname, count) in &self.sorted_operator_usage {
                writeln!(f, "\t{:#018}{:#8}", opname, count)?;
            }
        }

        Ok(())
    }
}

/// Analyze the file at `source` to see what sections it has and operators it uses
pub fn analyze(source: &Path) -> Result<Analysis> {
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
        function_count: 0,

        include_sections: true,
        sections: Vec::new(),
        sections_size_total: 0,

        include_operators: true,
        operator_usage: BTreeMap::<String, u64>::new(),
        sorted_operator_usage: vec!(),
        operator_count: 0,
    };

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
                analysis.add_section("CodeSectionStart", Some(count), &range),
            CodeSectionEntry(function_body) => analysis.add_function(&function_body)?,
            ComponentSection { parser, range } =>
                analysis.add_section("ComponentSection", None, &range),
            ComponentInstanceSection(section) =>
                analysis.add_section("ComponentInstanceSection", None, &section.range()),
            ComponentAliasSection(section) =>
                analysis.add_section("ComponentAliasSection", None, &section.range()),
            ComponentTypeSection(section) =>
                analysis.add_section("ComponentTypeSection", None, &section.range()),
            ComponentCanonicalSection(section) =>
                analysis.add_section("ComponentCanonicalSection", None, &section.range()),
            ComponentStartSection { start, range } =>
                analysis.add_section("ComponentStartSection", None, &range),
            ComponentImportSection(section) =>
                analysis.add_section("ComponentImportSection", None, &section.range()),
            ComponentExportSection(section) =>
                analysis.add_section("ComponentExportSection", None, &section.range()),
            CoreTypeSection(section) =>
                analysis.add_section("CoreTypeSection", None, &section.range()),
            CustomSection(section) =>
                analysis.add_section("CustomSection", None, &section.range()),
            DataCountSection { count, range } =>
                analysis.add_section("DataCountSection", Some(count), &range),
            DataSection(section) =>
                analysis.add_section("DataSection", Some(section.count()), &section.range()),
            ElementSection(section) =>
                analysis.add_section("ElementSection", Some(section.count()), &section.range()),
            ExportSection(section) =>
                analysis.add_section("ExportSection", Some(section.count()), &section.range()),
            FunctionSection(section) =>
                analysis.add_section("FunctionSection", Some(section.count()), &section.range()),
            GlobalSection(section) =>
                analysis.add_section("GlobalSection", Some(section.count()), &section.range()),
            ImportSection(section) =>
                analysis.add_section("ImportSection", Some(section.count()), &section.range()),
            InstanceSection(section) =>
                analysis.add_section("InstanceSection", Some(section.count()), &section.range()),
            MemorySection(section) =>
                analysis.add_section("MemorySection", Some(section.count()), &section.range()),
            ModuleSection { parser, range } =>
                analysis.add_section("ModuleSection", None, &range),
            StartSection { func, range } =>
                analysis.add_section("StartSection", None, &range),
            TableSection(section) =>
                analysis.add_section("TableSection", Some(section.count()), &section.range()),
            TagSection(section) =>
                analysis.add_section("TagSection", Some(section.count()), &section.range()),
            TypeSection(section) =>
                analysis.add_section("TypeSection", Some(section.count()), &section.range()),
            UnknownSection { id, contents, range } =>
                analysis.add_section("UnknownSection", None, &range),
            Version { num, encoding, range } => {
                analysis.version = num;
                analysis.add_section("Magic & Version", None, &range);
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
    }

    // order the operator usage
    let mut vec: Vec<(String, u64)> = analysis.operator_usage.iter()
        .map(|(s, c)| (s.to_string(), *c)).collect();
    vec.sort_by(|a, b| b.1.cmp(&a.1));
    analysis.sorted_operator_usage = vec;

    Ok(analysis)
}