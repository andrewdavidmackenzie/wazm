use std::path::Path;
use clap::{Arg, ArgMatches, Command};
use std::process::exit;
use log::LevelFilter;
use env_logger::Builder;
use core::str::FromStr;

mod errors;

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

    if matches.get_flag("analyze") {
        let analysis = wazm::analyze(&source)?;
        println!("Analysis: {:#?}", analysis);
    } else {
        let destination_filename = format!("{source_filename}.wz");
        let destination = Path::new(&destination_filename);
        wazm::compress(&source, &destination)?;
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
        .arg(Arg::new("wasm-file")
            .num_args(1)
            .help("the file path of the wasm file to compress/decompress"));

    app.get_matches()
}