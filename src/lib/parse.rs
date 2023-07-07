use std::path::Path;
use crate::errors::*;
use wasmparser::{Parser, Payload::*, Payload};
use std::fmt;

/// wasm Module
pub struct Module<'a> {
    pub source: String,
    pub version: u16,
    pub file_size: u64,
    pub sections: Vec<Payload<'a>>,
}

impl<'a> Module<'a> {
    fn add_section(&mut self, section: Payload<'a>) -> Result<()> {
        #[allow(unused_variables)]
        match &section {
            Version { num, encoding, range } => self.version = *num,
            _ => {}
        }
        self.sections.push(section);
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
            sections: vec![],
        };

        for payload in Parser::new(0).parse_all(buf) {
            match payload {
                Ok(End(_)) => continue,
                Ok(section) => module.add_section(section)?,
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
        writeln!(f, "File Size: {}", self.file_size)
    }
}