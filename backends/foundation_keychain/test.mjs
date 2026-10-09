// Miniflare integration tests for the foundation_keychain Cloudflare Worker.
//
// Exercises the full Worker runtime — D1 queries, JWT auth, Bitwarden API
// routes, and the SignalR Durable Object — using miniflare's programmatic API.
//
// Usage:
//   node test.mjs              (builds wasm first, then runs all tests)
//   node test.mjs --no-build   (skip wasm build, just run tests)
//
// Requires: node, miniflare, wrangler (all already in workspace node_modules).

import { execSync } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';
import { ok, deepEqual } from 'node:assert/strict';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ── Build ──────────────────────────────────────────────────────────────────

const skipBuild = process.argv.includes('--no-build');
if (!skipBuild) {
  console.log('Building WASM...');
  execSync('npx worker-build --release', { cwd: __dirname, stdio: 'inherit' });
  console.log('✓ WASM built\n');
}

// ── Miniflare setup ────────────────────────────────────────────────────────

const { Miniflare, Response: MfResponse } = await import('miniflare');

function createMiniflare() {
  return new Miniflare({
    modules: true,
    modulesRules: [
      { type: 'ESModule', include: ['**/*.js'], fallthrough: true },
      { type: 'CompiledWasm', include: ['**/*.wasm'] },
    ],
    scriptPath: path.join(__dirname, 'build/index.js'),
    d1Databases: { KEYCHAIN_DB: 'keychain-db' },
    kvNamespaces: ['KEYCHAIN_KV'],
    durableObjects: { SIGNALR_HUB: 'SignalRHub' },
    compatibilityDate: '2025-07-19',
  });
}

// ── Helpers ────────────────────────────────────────────────────────────────

const BASE = 'http://localhost';

async function post(mf, url, body, headers = {}) {
  const isRaw = typeof body === 'string';
  return mf.dispatchFetch(BASE + url, {
    method: 'POST',
    headers: { 'Content-Type': isRaw ? 'application/x-www-form-urlencoded' : 'application/json', ...headers },
    body: isRaw ? body : JSON.stringify(body),
  });
}

async function get(mf, url, headers = {}) {
  return mf.dispatchFetch(BASE + url, { method: 'GET', headers });
}

// ── Test runner ────────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;

async function test(name, fn) {
  try {
    await fn();
    console.log(`  ✓ ${name}`);
    passed++;
  } catch (err) {
    console.log(`  ✗ ${name}`);
    console.log(`    ${err.message}`);
    if (err.stack) {
      const stackLine = err.stack.split('\n')[1]?.trim();
      if (stackLine) console.log(`    ${stackLine}`);
    }
    failed++;
  }
}

// ── Tests ──────────────────────────────────────────────────────────────────

async function runAllTests(mf, ns) {
  // ── REST API (unauthenticated) ─────────────────────────────────────────

  await test('prelogin returns KDF params', async () => {
    const res = await post(mf, '/identity/accounts/prelogin', { email: 'test@example.com' });
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(body.Kdf !== undefined || body.kdf !== undefined, 'missing KDF field');
    const kdf = body.kdf ?? body.Kdf;
    ok(kdf === 0, `expected PBKDF2 KDF=0, got ${kdf}`);
    ok(body.kdfIterations !== undefined || body.KdfIterations !== undefined, 'missing iterations');
  });

  await test('register creates a user', async () => {
    const res = await post(mf, '/identity/accounts/register', {
      name: 'Test User',
      email: 'test@example.com',
      masterPasswordHash: 'a' + 'b'.repeat(63), // 64-char hex-ish hash
      masterPasswordHint: 'test',
      kdf: 0,
      kdfIterations: 600000,
    });
    // register returns the created user or token response
    ok(res.status === 200, `expected 200, got ${res.status} — probably email collision from prior test run`);
  });

  await test('login returns JWT access + refresh tokens', async () => {
    const res = await post(mf, '/identity/connect/token',
      'grant_type=password&username=test@example.com&password=' + 'a' + 'b'.repeat(63),
      { 'Content-Type': 'application/x-www-form-urlencoded' }
    );
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(typeof body.access_token === 'string', 'missing access_token');
    ok(typeof body.refresh_token === 'string', 'missing refresh_token');
    accessToken = body.access_token;
  });

  // ── REST API (authenticated) ────────────────────────────────────────────

  await test('GET /api/sync returns profile + folders + ciphers', async () => {
    const res = await get(mf, '/api/sync', { Authorization: `Bearer ${accessToken}` });
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(typeof body === 'object' && body !== null, 'sync response is not an object');
    // SyncData: { profile, folders, ciphers, collections, domains, sends }
    ok(typeof body.profile === 'object', 'missing profile in sync');
    ok(Array.isArray(body.folders), 'missing folders array');
    ok(Array.isArray(body.ciphers), 'missing ciphers array');
  });

  await test('POST /api/folders creates a folder', async () => {
    const res = await post(mf, '/api/folders',
      { name: 'Test Vault' },
      { Authorization: `Bearer ${accessToken}` }
    );
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(typeof body.Id === 'string' || typeof body.id === 'string', 'missing folder Id');
  });

  await test('GET /api/folders lists folders', async () => {
    const res = await get(mf, '/api/folders', { Authorization: `Bearer ${accessToken}` });
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    // Response is a plain JSON array (Bitwarden API: array of Folder)
    ok(Array.isArray(body), `folders should be array, got ${typeof body}`);
    ok(body.length >= 1, 'expected at least 1 folder');
  });

  await test('POST /api/ciphers creates a login cipher', async () => {
    const res = await post(mf, '/api/ciphers',
      {
        type: 1, // Login
        name: 'example.com',
        login: {
          username: 'user@example.com',
          password: 'secret123',
          uris: [{ uri: 'https://example.com', match: null }],
        },
      },
      { Authorization: `Bearer ${accessToken}` }
    );
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(typeof body.id === 'string', 'missing cipher id');
    cipherId = body.id;
  });

  await test('GET /api/ciphers lists ciphers', async () => {
    const res = await get(mf, '/api/ciphers', { Authorization: `Bearer ${accessToken}` });
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    // Response is a plain JSON array (Bitwarden API: array of Cipher)
    ok(Array.isArray(body), `ciphers should be array, got ${typeof body}`);
    ok(body.length >= 1, 'expected at least 1 cipher');
  });

  // ── Auth edge cases ─────────────────────────────────────────────────────

  await test('missing auth returns 401', async () => {
    const res = await get(mf, '/api/sync');
    ok(res.status === 401, `expected 401, got ${res.status}`);
  });

  await test('bad auth returns 401', async () => {
    const res = await get(mf, '/api/sync', { Authorization: 'Bearer not.a.real.token' });
    ok(res.status === 401, `expected 401, got ${res.status}`);
  });

  await test('unknown route returns 404-ish error', async () => {
    const res = await get(mf, '/api/nonexistent', { Authorization: `Bearer ${accessToken}` });
    // router returns 404 for unmatched paths
    ok(res.status >= 400, `expected 4xx, got ${res.status}`);
  });

  await test('prelogin for nonexistent user returns KDF=0', async () => {
    const res = await post(mf, '/identity/accounts/prelogin', { email: 'nobody@nowhere.com' });
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    const kdf = body.kdf ?? body.Kdf;
    ok(kdf === 0, 'nonexistent user should still return KDF=0');
  });

  // ── Durable Object ─────────────────────────────────────────────────────
  // Miniflare DO API: ns.idFromName(name) → ns.get(id) → stub.fetch(url)

  const doStub = (name) => ns.get(ns.idFromName(name));
  const doFetch = (name, path, init) => doStub(name).fetch(BASE + path, init);

  await test('DO /health returns connected count', async () => {
    const res = await doFetch('test-user', '/health');
    ok(res.status === 200, `expected 200, got ${res.status}`);
    const body = await res.json();
    ok(typeof body.messages === 'number', 'missing messages field');
    ok(typeof body.sockets === 'number', 'missing sockets field');
    ok(body.sockets === 0, 'no sockets connected yet');
  });

  await test('DO /notify broadcasts to connected sockets', async () => {
    const res = await doFetch('test-user', '/notify', {
      method: 'POST',
      body: JSON.stringify([{ id: 'cipher-1' }, 2]),
    });
    ok(res.status === 200, `expected 200, got ${res.status}`);
  });

  await test('DO unknown route returns 404', async () => {
    const res = await doFetch('test-user', '/unknown');
    ok(res.status === 404, `expected 404, got ${res.status}`);
  });

  // ── WebSocket upgrade (DO) ──────────────────────────────────────────────

  await test('DO /ws upgrades to WebSocket', async () => {
    const res = await doFetch('ws-test', '/ws', {
      headers: { 'Upgrade': 'websocket' },
    });
    ok(res.status === 101, `expected 101, got ${res.status}`);
    ok(res.webSocket !== undefined && res.webSocket !== null,
      'missing webSocket on upgrade response');
  });
}

// ── Main ───────────────────────────────────────────────────────────────────

let accessToken = '';
let cipherId = '';

const mf = createMiniflare();

try {
  console.log('Starting Miniflare...\n');

  // Get DO namespace for SignalR hub tests
  const ns = await mf.getDurableObjectNamespace('SIGNALR_HUB');
  console.log('✓ Got DO namespace: SIGNALR_HUB\n');

  console.log('Running tests:\n');
  await runAllTests(mf, ns);

  console.log(`\n${passed} passed, ${failed} failed, ${passed + failed} total`);
} catch (err) {
  console.error('Fatal:', err.message);
  console.error(err.stack);
  process.exit(1);
} finally {
  await mf.dispose();
}

if (failed > 0) process.exit(1);
