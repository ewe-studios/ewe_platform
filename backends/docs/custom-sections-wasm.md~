# Custom Sections
Custom sections allow embedding named arbitrary data into a wasm module. The section data is set at compile time and is read directly from the wasm module, it cannot be modified at runtime.

In Rust, custom sections are static arrays ([T; size]) exposed with the #[link_section] attribute:



#[link_section = "hello"]
pub static SECTION: [u8; 24] = *b"This is a custom section";
This adds a custom section named hello to the wasm file, the rust variable name SECTION is arbitrary, changing it wouldn't alter the behaviour. The contents are bytes of text here but could be any arbitrary data.

The custom sections can be read on the JS side using the WebAssembly.Module.customSections function, it takes a wasm Module and the section name as arguments and returns an Array of ArrayBuffers. Multiple sections may be specified using the same name, in which case they will all appear in this array.


WebAssembly.compileStreaming(fetch("sections.wasm"))
.then(mod => {
  const sections = WebAssembly.Module.customSections(mod, "hello");

  const decoder = new TextDecoder();
  const text = decoder.decode(sections[0]);

  console.log(text); // -> "This is a custom section"
});
