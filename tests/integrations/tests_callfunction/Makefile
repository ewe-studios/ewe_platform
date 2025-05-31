
build:
	@RUSTFLAGS='-C link-arg=-s' cargo build --package  tests_callfunction --target wasm32-unknown-unknown 
	cp ../../../target/wasm32-unknown-unknown/debug/tests_callfunction.wasm ./tests_callfunction.wasm
	cp ../../../target/wasm32-unknown-unknown/debug/tests_callfunction.d ./tests_callfunction.d
	wasm2wat ./tests_callfunction.wasm -o ./tests_callfunction.wat
