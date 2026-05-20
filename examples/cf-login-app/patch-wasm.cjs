// Patch wasm-bindgen generated JS to skip __wbindgen_start() call.
// wasm-opt strips __wbindgen_start(), causing runtime errors in wrangler.
// With workers-rs #[event(fetch)], wasm init is handled by the fetch entry point.
const fs = require('fs');
const path = require('path');

// wasm-pack --target bundler outputs to pkg/, wrangler copies build/pkg/
// Check both locations
const jsPath = fs.existsSync(path.join(__dirname, 'pkg', 'cf_login_app.js'))
  ? path.join(__dirname, 'pkg', 'cf_login_app.js')
  : path.join(__dirname, 'build', 'pkg', 'cf_login_app.js');

if (!fs.existsSync(jsPath)) {
  console.error('wasm-bindgen JS not found at:', jsPath);
  process.exit(1);
}

let content = fs.readFileSync(jsPath, 'utf8');

// Remove the wasm.__wbindgen_start() call
content = content.replace(/\nwasm\.__wbindgen_start\(\);\n/, '\n');

fs.writeFileSync(jsPath, content);
console.log('patched:', jsPath, '— removed __wbindgen_start() call');
