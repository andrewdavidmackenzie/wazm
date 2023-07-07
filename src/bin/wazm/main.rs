use std::path::Path;
use clap::{Arg, ArgMatches, Command};
use std::process::exit;
use log::LevelFilter;
use env_logger::Builder;
use core::str::FromStr;

mod errors;

use wazm::Module;
use crate::errors::Result;
use crate::errors::bail;

/// Main for flowr binary - call `run()` and print any error that results or exit silently if OK
pub fn main() {
    let matches = get_matches();
    let default = String::from("error");
    let verbosity = matches.get_one::<String>("verbosity").unwrap_or(&default);
    let level = LevelFilter::from_str(verbosity).unwrap_or(LevelFilter::Error);
    let mut builder = Builder::from_default_env();
    builder.filter_level(level).init();

    match run(matches) {
        Err(ref e) => {
            eprintln!("{e}");
            for e in e.iter().skip(1) {
                eprintln!("caused by: {e}");
            }

            // The backtrace is generated if env var `RUST_BACKTRACE` is set to `1` or `full`
            if let Some(backtrace) = e.backtrace() {
                eprintln!("backtrace: {backtrace:?}");
            }

            exit(1);
        }
        Ok(_) => exit(0),
    }
}

fn run(matches: ArgMatches) -> Result<()> {
    let source_filename = matches.get_one::<String>("wasm-file")
        .expect("WASM file name must be specified"); // TODO clap to do this
    let source = Path::new(&source_filename);
    if !source.exists() {
        bail!("File '{}' does not exist", source_filename);
    }
    if source.extension() != Some("wasm".as_ref()) &&
        source.extension() != Some("wz".as_ref()) {
        bail!("File '{}' does not have a .wasm nor .wz extension", source_filename);
    }

    if matches.get_flag("analyze") {
        let buf: Vec<u8> = std::fs::read(source)?;
        let module = Module::parse(source, &buf)?;
        let analysis = wazm::analyze(&module,
                                     matches.get_flag("analyze-sections"),
                                     matches.get_flag("analyze-functions"),
                                     matches.get_flag("analyze-operators"),
                                     matches.get_flag("analyze-call-tree"),
        )?;
        println!("{}", analysis);

        let unaccounted_for = module.file_size - analysis.sections_size_total as u64;
        if unaccounted_for != 0 {
            println!("Bytes unaccounted for: {}", unaccounted_for);
        }
    } else if source.extension() == Some("wasm".as_ref()) {
        let destination_filename = format!("{source_filename}.wz");
        let destination = Path::new(&destination_filename);
        wazm::compress(source, destination)?;
    } else {
        let destination_filename = source.with_extension("");
        let destination = Path::new(&destination_filename);
        wazm::decompress(source, destination)?;
    }

    Ok(())
}

// Parse the command line arguments using clap
fn get_matches() -> ArgMatches {
    let app = Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .arg(Arg::new("verbosity")
            .short('v')
            .long("verbosity")
            .number_of_values(1)
            .value_name("VERBOSITY_LEVEL")
            .help("Set verbosity level for output (trace, debug, info, warn, default: error)"))
        .arg(Arg::new("analyze")
            .short('a')
            .long("analyze")
            .action(clap::ArgAction::SetTrue)
            .help("Analyze the WASM file"))
        .arg(Arg::new("analyze-sections")
            .short('s')
            .long("analyze-sections")
            .requires("analyze")
            .action(clap::ArgAction::SetTrue)
            .help("Analyze the Sections of the WASM file"))
        .arg(Arg::new("analyze-functions")
            .short('f')
            .long("analyze-functions")
            .requires("analyze")
            .action(clap::ArgAction::SetTrue)
            .help("Analyze the Functions in the WASM file"))
        .arg(Arg::new("analyze-call-tree")
            .short('t')
            .long("analyze-call-tree")
            .requires("analyze")
            .action(clap::ArgAction::SetTrue)
            .help("Analyze the call-tree of Functions in the WASM file"))
        .arg(Arg::new("analyze-operators")
            .short('o')
            .long("analyze-operators")
            .requires("analyze")
            .requires("analyze-functions")
            .action(clap::ArgAction::SetTrue)
            .help("Analyze the Operators used in the WASM file"))
        .arg(Arg::new("wasm-file")
            .num_args(1)
            .help("the file path of the wasm file to compress/decompress"));

    // TODO add an option to validate contents are equivalent after compressing by
    // decompressing, parsing and then comparing
    app.get_matches()
}