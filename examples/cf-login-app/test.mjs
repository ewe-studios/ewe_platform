// Build wasm before tests
import { execSync } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Build the wasm using worker-build
execSync('worker-build --release', {
  cwd: __dirname,
  stdio: 'inherit'
});
console.log('✓ wasm built');

// Now run tests
async function runTests() {
  const { Miniflare } = await import('miniflare');

  const mf = new Miniflare({
    modules: true,
    modulesRules: [
      { type: 'ESModule', include: ['**/*.js'], fallthrough: true },
      { type: 'CompiledWasm', include: ['**/*.wasm'] },
    ],
    scriptPath: path.join(__dirname, 'build/index.js'),
    d1Databases: { DB: 'test-db' },
    compatibilityDate: '2024-01-01',
  });

  try {
    console.log('\n--- Test: GET / (home) ---');
    const res = await mf.dispatchFetch('http://localhost:8789/');
    console.log(`Status: ${res.status}`);
    const text = await res.text();
    console.log(`Body (first 200): ${text.slice(0, 200)}`);
    console.log(res.status === 302 ? '✓ Redirect' : '✗ Unexpected');

    console.log('\n--- Test: GET /login ---');
    const res2 = await mf.dispatchFetch('http://localhost:8789/login');
    console.log(`Status: ${res2.status}`);
    const text2 = await res2.text();
    console.log(text2.includes('Login') ? '✓ Login page' : '✗ No login page');
    console.log(`Body (first 200): ${text2.slice(0, 200)}`);

    console.log('\n--- Test: GET /register ---');
    const res3 = await mf.dispatchFetch('http://localhost:8789/register');
    console.log(`Status: ${res3.status}`);
    const text3 = await res3.text();
    console.log(text3.includes('Register') ? '✓ Register page' : '✗ No register page');

    console.log('\nAll basic tests passed ✓');
  } catch (err) {
    console.error('Test failed:', err.message);
    console.error(err.stack);
    process.exit(1);
  } finally {
    await mf.dispose();
  }
}

runTests();
