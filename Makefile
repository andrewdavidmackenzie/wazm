TOOL_LIST := gzip wazm
WASM_FILE_LIST := $(shell ls test_files)

export PATH := $(PWD)/wazm/target/debug:$(PWD)/wazm/target/release:$(PATH)

test: build_wazm
	@mkdir -p test_output || true
	@for tool in $(TOOL_LIST) ; do \
  		echo "Testing '$$tool'" ; \
  		rm -f test_output/* ; \
		for wasm_file in $(WASM_FILE_LIST) ; do \
		  cp test_files/$$wasm_file test_output/$$wasm_file ; \
		  echo "Testing file: '$$wasm_file'" ; \
		  $$tool test_output/$$wasm_file ; \
		done ; \
		echo "" ; \
	done

build_wazm:
	cargo build --manifest-path=wazm/Cargo.toml