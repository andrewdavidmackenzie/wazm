use std::path::Path;
use crate::errors::*;
use wasmparser::{Parser, Payload::*, Payload};
use std::fmt;
use log::debug;

/// wasm Module
pub struct Module<'a> {
    pub source: String,
    pub version: u16,
    pub file_size: u64,
    pub payloads: Vec<Payload<'a>>,
}

impl<'a> Module<'a> {
    fn add_payload(&mut self, payload: Payload<'a>) -> Result<()> {
        #[allow(unused_variables)]
        match &payload {
            Version { num, encoding, range } => self.version = *num,
            _ => debug!("Adding non-Version payload"),
        }
        self.payloads.push(payload);
        Ok(())
    }

    /// Check that a [Module] is valid
    pub fn validate(self) -> Result<Self> {
        if self.version == 0 {
            bail!("Invalid WASM version in module");
        }

        Ok(self)
    }

    /// Parse a source file on disk into a [Module}
    pub fn parse(source: &Path, buf: &'a [u8]) -> Result<Self> {
        let mut module = Self {
            source: source.canonicalize()?.display().to_string(),
            version: 0,
            file_size: source.metadata()?.len(),
            payloads: vec![],
        };

        for payload in Parser::new(0).parse_all(buf) {
            match payload {
                Ok(End(_)) => continue,
                Ok(section) => module.add_payload(section)?,
                _ => bail!("Unexpected payload while parsing WASM Module"),
            }
        }

        module.validate()
    }
}

impl<'a> fmt::Display for Module<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "source: {}", self.source)?;
        writeln!(f, "version: {}", self.version)?;
        writeln!(f, "size: {}", self.file_size)
    }
}