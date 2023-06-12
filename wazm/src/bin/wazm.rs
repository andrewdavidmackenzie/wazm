pub fn main() {
    let filename = std::env::args().nth(1).expect("Filename is required");
    println!("file: {filename}");
}