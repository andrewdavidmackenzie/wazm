use wax::Glob;
use std::path::PathBuf;

const TOOL_LIST : [(&str, &str); 2] = [("wazm", "wz"), ("gzip", "gz")];

#[test]
fn test() {
//    export PATH := $(PWD)/wazm/target/debug:$(PWD)/wazm/target/release:$(PATH)

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut test_files_dir = root.clone();
    test_files_dir.push("tests");
    test_files_dir.push("test_files");

    let mut test_output_dir = root.clone();
    test_output_dir.push("tests");
    test_output_dir.push("test_output");
    let _ = std::fs::create_dir(test_output_dir);

    let glob = Glob::new("**/*.wasm").expect("Globbing error");
    for entry in glob.walk(test_files_dir) {
        let entry = entry.unwrap();
        let path = entry.path();
        println!("Testing file {}", path.display());

        for (tool, extension) in TOOL_LIST {
            println!("Testing tool {tool} with extension {extension}");
        }
        //output_file.set_extension("dot.svg");

        /*
        #[allow(clippy::needless_borrow)]
        if Command::new(&dot)
            .args(vec!["-Tsvg", &format!("-o{}", output_file.display()), &path_name])
            .status()?.success() {
            debug!(".dot.svg successfully generated from {path_name}");
            if delete_dots {
//                    std::fs::remove_file(path)?;
                debug!("Source file {path_name} was removed after SVG generation")
            }
        } else {
            bail!("Error executing 'dot'");
        }

         */
    }
}

    /*
    @rm -f test_output/ * ;
	@for wasm_file in $(WASM_FILE_LIST) ; do \
	  original=`du test_files/$$wasm_file | cut -f1` ; \
	  echo "$$wasm_file	$$original" ; \
		for tool in $(TOOL_LIST) ; do \
	  	  cp test_files/$$wasm_file test_output/$$wasm_file ; \
		  $$tool test_output/$$wasm_file ; \
		  size_after=`du test_output/$$wasm_file | cut -f1` ; \
		  echo "$$tool	$$size_after" ; \
		done ; \
		echo "" ; \
	done
     */