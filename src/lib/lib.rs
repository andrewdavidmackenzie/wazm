use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use errors::*;
//use std::ops::Range;
use std::io::Read;
use wasmparser::{Parser, Chunk, Payload::*};
use core::ops::Range;

/// A module with error types
pub mod errors;

#[derive(Debug)]
struct Section {
    section_type: String,
    item_count: Option<u32>,
    range: Range<usize>,
    size: usize,
}

/// Analysis results of a wasm file
#[derive(Debug)]
#[allow(dead_code)] // source not used in lib
pub struct Analysis {
    source: String,
//    operator_usage: BTreeMap<String, u64>,
//    operator_count: u64,
    version: u16,
    sections: Vec<Section>,
    function_count: u64,
    section_size_total: usize,
    file_size: u64,
}

impl Analysis {
    fn add_section(&mut self, section_type: &str, item_count: Option<u32>, range: &Range<usize>) {
        let size = range.end - range.start;
        self.section_size_total += size;
        self.sections.push(
            Section {
                section_type: section_type.to_owned(),
                item_count,
                range: range.clone(),
                size,
            }
        );
    }
}

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn compress(source: &Path, destination: &Path) -> Result<u64> {
    std::fs::copy(source, destination).chain_err(|| "Could not compress")
}

fn parse(mut reader: impl Read, analysis: &mut Analysis) -> Result<()> {
    let mut buf = Vec::new();
    let mut parser = Parser::new(0);
    let mut eof = false;
    let mut stack = Vec::new();

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
            CodeSectionEntry(function_body) => {
                // TODO here we can iterate over `body` to parse the function
                // and its locals
                // get_binary_reader();
                // get_locals_reader();
                // get_operators_reader()
                analysis.function_count += 1;
            }
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
            Version { num, encoding, range } =>
                analysis.add_section("Version", None, &range),

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

    Ok(())
}

/// Compress file at `source`into a new file at `destination`
/// Return a Result with the size of the output file in bytes
pub fn analyze(source: &Path) -> Result<Analysis> {
//    let mut operator_usage = HashMap::<String, u64>::new();
//    let mut operator_count = 0;

    let mut analysis = Analysis {
        source: source.canonicalize()?.display().to_string(),
//        operator_usage: sorted_operator_usage,
//        operator_count,
        file_size: source.metadata()?.len(),
        version: 0,
        sections: Vec::new(),
        section_size_total: 0,
        function_count: 0,
    };

    let f = File::open(source)?;
    let reader = BufReader::new(f);

    let _ = parse(reader, &mut analysis)?;

    Ok(analysis)
}

/*
fn print_range(section: &str, range: &Range<usize>) {
    println!("{:>40}: {:#010x} - {:#010x}", section, range.start, range.end);
}

 */