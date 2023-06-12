use wax::Glob;
use std::path::PathBuf;
use std::process::Command;
use std::fs;

const TOOL_LIST : [(&str, &str); 2] = [("wazm", "wz"), ("gzip", "gz")];

#[test]
fn test() {
    let test_files_dir = PathBuf::from("tests/test_files");
    let test_output_dir = PathBuf::from("tests/test_output");
    let _ = std::fs::remove_dir_all(&test_output_dir);
    let _ = std::fs::create_dir(&test_output_dir);

    let path = std::env::var("PATH").unwrap();
    let extended_path = &format!("target/debug:target/release:{}", path);

    let glob = Glob::new("**/*.wasm").expect("Globbing error");
    for entry in glob.walk(test_files_dir) {
        let entry = entry.unwrap();
        let path = entry.path();

        let original_size = path.metadata().unwrap().len();
        println!("{} {}", path.file_name().unwrap().to_string_lossy(), original_size);

        for (tool, extension) in TOOL_LIST {
            let mut test_output_file = test_output_dir.clone();
            test_output_file.push(path.file_name().unwrap());
            fs::copy(path, &test_output_file).unwrap();
            assert!(Command::new(tool)
                        .env("PATH", extended_path)
                        .arg(path)
                        .status().unwrap()
                        .success(), "Could not run tool");
            let output = format!("{}.{}", path.display(), extension);
            let output_path = PathBuf::from(output);
            let new_size = output_path.metadata().unwrap().len();
            println!("{tool} {new_size} {}%", (new_size * 100) / original_size);
        }
    }
}