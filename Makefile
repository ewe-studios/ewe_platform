PACKAGES = ewe_platform foundations_ext foundations_jsnostd foundations_nostd ewe_trace ewe_async_utils ewe_channels ewe_domain ewe_devserver ewe_domain_server ewe_html ewe_html_macro ewe_mem ewe_routing ewe_spawn ewe_spawn ewe_template_macro ewe_templates ewe_temple ewe_watch_utils ewe_watchers ewe_web
TESTS_PACKAGES = $(wildcard ./tests/integrations/*)

TEST_DIRECTORY ?= ./test/integrations/tests_callfunction
TEST_PACKAGE ?= $(notdir $(TEST_DIRECTORY))

bacon:
	bacon -j bacon-ls

sandbox:
	cargo run --profile dev --bin ewe_platform sandbox

build-test-directory:
	@RUSTFLAGS='-C link-arg=-s' cargo build --package $(TEST_PACKAGE) --target wasm32-unknown-unknown
	cp ./target/wasm32-unknown-unknown/debug/$(TEST_PACKAGE).d $(TEST_DIRECTORY)/$(TEST_PACKAGE).d
	cp ./target/wasm32-unknown-unknown/debug/$(TEST_PACKAGE).wasm $(TEST_DIRECTORY)/module.wasm
	wasm2wat $(TEST_DIRECTORY)/module.wasm -o $(TEST_DIRECTORY)/module.wat

build-demos:
	@RUSTFLAGS='-C link-arg=-s' cargo build --package intro --target wasm32-unknown-unknown
	cp target/wasm32-unknown-unknown/debug/intro.wasm ./assets/public/intro.wasm
	wasm2wat ./assets/public/intro.wasm -o ./assets/public/intro.wat

build-tests:
	$(foreach var,$(TESTS_PACKAGES), $(MAKE) TEST_DIRECTORY=$(var) TEST_PACKAGE=$(notdir $(var)) build-test-directory;)

wasm-tests: build-tests
	$(foreach var,$(TESTS_PACKAGES), node $(var)/index.node.js;)

lint:
	cargo fmt

test:
	$(foreach var,$(PACKAGES), cargo test --package $(var);)

publish:
	$(foreach var,$(PACKAGES), cargo publish --package $(var);)
