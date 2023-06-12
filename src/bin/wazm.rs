use std::path::Path;

pub fn main() {
    let source_filename = std::env::args().nth(1).expect("Filename is required");
    let source = Path::new(&source_filename);
    let destination_filename = format!("{source_filename}.wz");
    let destination = Path::new(&destination_filename);
    let _ = wazm::compress(&source, &destination);
}