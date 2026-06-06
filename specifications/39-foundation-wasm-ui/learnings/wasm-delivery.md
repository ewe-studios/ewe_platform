# How we could delivery wasm to the web

For a full-stack, reactive Rust framework like Leptos, delivery splits neatly into two distinct strategies depending on whether you choose Server-Side Rendering (SSR) or a Client-Side Single Page App (CSR). [1, 2] 
Here is exactly how Leptos applications deliver Wasm in production today: [2, 3] 
## 1. The Full-Stack Way: SSR with Hydration (Most Common) [4] 
In a standard Leptos setup, your code compiles into two distinct parts: a native backend binary (usually powered by Axum or Actix-web) and a client-side Wasm bundle. [2, 3, 5] 

* The Build Tool: Developers use cargo-leptos. This specialized build tool compiles the server binary for your target architecture (e.g., Linux x86_64) and automatically compiles the frontend into a highly optimized .wasm asset alongside a JavaScript loader. [2, 5, 6, 7] 
* Delivery Flow:
1. The user requests a page. The native Linux server renders raw HTML immediately and streams it to the user browser.
   2. Along with the HTML, the server serves the .wasm file as a static asset out of a directory like /target/site/pkg/.
   3. The browser fetches the Wasm binary, downloads it, and executes it to "hydrate" the page—turning the static HTML into a fully interactive, reactive Rust app. [1, 2, 3, 7, 8] 
* Where it's deployed: This setup requires a standard server environment rather than a serverless Wasm runtime. It is typically packaged into a minimal Docker container and deployed to platforms like Fly.io, [Railway](https://railway.com/), or standard cloud platforms (AWS EC2, Google Cloud Run). [3, 9, 10, 11, 12] 

## 2. The Static Way: Client-Side Rendering (CSR)
If you do not want a Rust backend and prefer to use Leptos strictly as a frontend alternative to React or SolidJS, you build it as a Single Page Application (SPA). [2, 4, 13] 

* The Build Tool: Developers use Trunk, a Wasm web application bundler for Rust. Trunk processes your index.html, compiles your Rust into Wasm, hashes the filenames for caching, and spits out a folder of entirely static files. [2, 14, 15] 
* Delivery Flow: There is no server running Rust. The .wasm file, HTML, and JS glue files are simply tossed onto an edge network or object storage bucket. The Wasm is served directly to the client browser via a CDN. [3] 
* Where it's deployed: Standard static hosting providers like Cloudflare Pages, Vercel, [Netlify](https://www.netlify.com/), or an AWS S3 bucket hooked up to CloudFront. [3] 

------------------------------
## Comparison Matrix

| Deployment Strategy [1, 2, 3, 9, 11] | Build Engine | Delivery Mechanism | Hosting Targets |
|---|---|---|---|
| Full-Stack (SSR) | cargo-leptos | Server streams HTML; Wasm is downloaded via background static asset requests to hydrate the UI. | Docker, Linux VMs, Fly.io, AWS Lambda. |
| Static SPA (CSR) | Trunk | Entire application (HTML, JS glue, and Wasm bundle) is pulled straight from an edge proxy cache. | CDNs, Cloudflare Pages, GitHub Pages, AWS S3. |

## Emerging Wasm-Native Server Alternatives
While most full-stack Leptos apps compile the server to native machine code (not Wasm), ecosystems like wasmCloud allow developers to compile the entire full-stack Leptos app (both server and client components) into an optimized component matching the WASI 0.2 Component Model specifications. This lets you run the server side inside a Wasm runtime as well! [1, 16] 
Are you starting a brand new Leptos project from scratch, or are you trying to dockerize and deploy an existing one? Let me know and I can supply the exact Dockerfile or configuration patterns you need!

[1] [https://github.com](https://github.com/leptos-rs/leptos)
[2] [https://book.leptos.dev](https://book.leptos.dev/getting_started/index.html)
[3] [https://rustify.rs](https://rustify.rs/articles/leptos-vs-dioxus-rust-frontend-2026)
[4] [https://rustify.rs](https://rustify.rs/articles/leptos-vs-dioxus-rust-frontend-2026)
[5] [https://www.youtube.com](https://www.youtube.com/watch?v=wm7BRRUdDt4&t=39)
[6] [https://www.youtube.com](https://www.youtube.com/watch?v=PRBK1BeXYJc)
[7] [https://www.rustadventure.dev](https://www.rustadventure.dev/building-wasm-web-ui-with-rust-and-leptos)
[8] [https://github.com](https://github.com/leptos-rs)
[9] [https://www.reddit.com](https://www.reddit.com/r/learnrust/comments/1d6r9bu/best_way_to_deploy_a_leptos_singlepage_app/)
[10] [https://www.youtube.com](https://www.youtube.com/watch?v=6rICMHd7as0&t=224)
[11] [https://book.leptos.dev](https://book.leptos.dev/deployment/ssr.html)
[12] [https://book.leptos.dev](https://book.leptos.dev/deployment/ssr.html)
[13] [https://reintech.io](https://reintech.io/blog/building-web-applications-with-leptos-complete-guide-2026)
[14] [https://www.youtube.com](https://www.youtube.com/watch?v=PZoZwbrJ16A&t=47)
[15] [https://thenewstack.io](https://thenewstack.io/want-a-web-framework-for-rust-not-javascript-try-leptos/)
[16] [https://www.nikl.me](https://www.nikl.me/blog/2024/bevy_ecs_as_data_layer_in_leptos_ssg/)


Yes, absolutely. Inlining WebAssembly binaries directly into a JavaScript file as a Base64-encoded string is a widely used deployment strategy, particularly for standalone utility libraries, widget embeds, and edge worker scripts. [1, 2, 3, 4] 
It reduces the application to a single file, eliminating secondary network fetches and completely bypassing complex CORS or routing configurations on your CDN. [1, 2, 5] 
------------------------------
## Popular Real-World Examples

* Node.js Core Utilities: Node.js implements its experimental native TypeScript execution engine (amaro) using a Rust-compiled Wasm binary that is [intentionally inlined inside their JavaScript source code](https://www.reddit.com/r/programming/comments/1ebtt4s/nodejs_adds_experimental_support_for_typescript/) to avoid multi-file filesystem dependencies. [6, 7, 8] 
* Database & Heavy Client Libraries: Tools like [sql.js](https://webreflection.medium.com/how-to-embed-your-wasm-blob-c29692119039) (SQLite for the browser) distribute an alternative single-file variant. Third-party widgets and canvas/charting SDKs often use this method so customers can paste a solitary script tag onto their site without hosting a separate .wasm file. [1, 9, 10] 

------------------------------
## How Bundlers Automate This Today
Modern build systems handle the conversion seamlessly without requiring manual string copying. [11] 
## 1. Vite
By default, [Vite](https://v4.vite.dev/guide/features) automatically inlines .wasm files as Base64 strings during a production build if the file is smaller than your configured assetsInlineLimit (usually 4KB). If you want to force Vite to inline a large file regardless of its size, you can import it explicitly or use custom query parameters: [12] 

import init from './my_project_bg.wasm?init' 
// Vite compiles this into a base64 string automatically, // allowing you to instantiate it directly without an external fetch!
init().then((instance) => {
  // Use your Rust/Wasm functions here
});

## 2. Webpack 5 & Rollup
You can configure Webpack using asset/inline modules to force Wasm into data URIs, or pull in plugins like @rollup/plugin-url for Rollup setups. [13] 
------------------------------
## Underlying Implementation: What the Output Looks Like
When compiled, the bundler transforms the binary file into a plain JavaScript utility structure: [14] 

// The bundler encodes the raw compiled binary bytes directly into textconst base64Wasm = "AGFzbQEAAAABBwFgAn9/AX8DAgEABwcBA2FkZAAACgkBBwAgACABags=";
function base64ToUint8Array(base64) {
  const binaryString = atob(base64); // Decode text back to standard bytes
  const bytes = new Uint8Array(binaryString.length);
  for (let i = 0; i < binaryString.length; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}
export async function loadWasm() {
  const wasmBuffer = base64ToUint8Array(base64Wasm);
  // Uses native instantiate instead of instantiateStreaming since we already have the bytes
  const { instance } = await WebAssembly.instantiate(wasmBuffer, {}); 
  return instance.exports;
}

------------------------------
## The Trade-offs You Need to Know
While single-file CDN caching is extremely convenient, you pay a tax for embedding: [2] 

* 🟢 The Pros:
* Zero-Config Deployment: Simply push one file to an AWS S3 bucket, Cloudflare Pages, or Netlify.
   * No Network Race Conditions: The application code and Wasm logic arrive at the exact same moment, avoiding "flash of uninitialized state" errors.
   * No CORS or MIME issues: You do not have to configure your CDN to explicitly serve the application/wasm header. [1, 2, 15] 
* 🔴 The Cons:
* 33% Larger Payload: Base64 text encoding increases the raw file size by roughly one-third compared to native binary bytes. Note: Gzip or Brotli compression on your CDN shrinks this back down drastically, but it remains slightly heavier than pure binary formats.
   * Slower Startup Time: The browser's JS engine must expend CPU cycles decoding the Base64 string into binary array buffers before compiling it. You lose the capability to use WebAssembly.instantiateStreaming(), which compiles Wasm in the background while the file is still downloading. [1, 2, 9, 16, 17] 

If you are using Leptos, let me know if you would like me to show you how to configure a Trunk or Vite config to automatically bake your output into a single-file artifact!

[1] [https://nickb.dev](https://nickb.dev/blog/recommendations-when-publishing-a-wasm-library/)
[2] [https://guido.io](https://guido.io/posts/embedding-webassembly-in-javascript/)
[3] [https://jsr.io](https://jsr.io/@doctor/wasm-bundler)
[4] [https://rahuulmiishra.medium.com](https://rahuulmiishra.medium.com/how-does-base64-work-9cbf8bd743a9)
[5] [https://medium.com](https://medium.com/@s0l0ist/c-to-webassembly-using-bazel-and-emscripten-ae797c119bef)
[6] [https://www.reddit.com](https://www.reddit.com/r/programming/comments/1ebtt4s/nodejs_adds_experimental_support_for_typescript/)
[7] [https://www.youtube.com](https://www.youtube.com/watch?v=4_YfWCyjra4)
[8] [https://elvisbrevi.hashnode.dev](https://elvisbrevi.hashnode.dev/improving-nodejs-with-rust-wasm-library)
[9] [https://webreflection.medium.com](https://webreflection.medium.com/how-to-embed-your-wasm-blob-c29692119039)
[10] [https://news.ycombinator.com](https://news.ycombinator.com/item?id=41963996)
[11] [https://guido.io](https://guido.io/posts/embedding-webassembly-in-javascript-without-a-bundler/)
[12] [https://v4.vite.dev](https://v4.vite.dev/guide/features)
[13] [https://webpack.js.org](https://webpack.js.org/blog/2020-10-10-webpack-5-release/)
[14] [https://guido.io](https://guido.io/posts/embedding-webassembly-in-javascript/)
[15] [https://medium.com](https://medium.com/@philip.mutua/the-best-cloud-platforms-to-deploy-frontend-apps-fast-easy-3391d237c94b)
[16] [https://serengetitech.com](https://serengetitech.com/tech/webassembly-a-light-at-the-end-of-the-tunnel/)
[17] [https://discussions.unity.com](https://discussions.unity.com/t/webgl-both-async-and-sync-fetching-of-the-wasm-failed-build-and-runs-works-fails-when-uploading/802944)


To bypass the size and CPU penalty of Base64 embedding, developers use a technique called Binary String / Array Mapping combined with native HTTP-level compression (Brotli or Zstandard). [1, 2, 3, 4] 
Modern browsers and CDNs handle this automatically, achieving near-instant decoding with maximum compression. [5] 
------------------------------
## The Strategy: Raw Binary JS + Server-Side Compression
Instead of encoding your Wasm file into a bloated Base64 string, you compile it directly into a JavaScript Uint8Array (raw binary byte values) or store it as an unencoded binary chunk.
## 1. The Super Fast Browser-Side Code
Your JavaScript file looks like this:

// Raw binary byte representation of Wasm (no base64 overhead!)const wasmBytes = new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0, 1, 133, 128, ...]);
export async function initWasm() {
  // Directly instantiate from the byte array. 
  // No CPU cycles spent running `atob()` or decoding text strings!
  const { instance } = await WebAssembly.instantiate(wasmBytes, {});
  return instance.exports;
}

## 2. The Server-Side Compression Algorithms
Because the JavaScript file now contains raw binary data, you rely on the CDN/Server to compress the entire .js file during flight. Two modern, server-supported options handle this beautifully: [6] 

* Brotli (br): Supported by default on virtually all servers and browsers. For static assets, you compress the file once at build time using the highest setting (Brotli Level 11). It shrinks binary data remarkably well and decompresses incredibly fast on the user's device. [5, 6, 7] 
* Zstandard (zstd): The modern speed king. Major edge networks (like [Cloudflare](https://developers.cloudflare.com/speed/optimization/content/compression/)) compress traffic using Zstd by default. It matches Brotli’s file reduction ratios but compresses and decompresses up to 40%+ faster, resulting in a significantly lower Time-to-First-Byte (TTFB). [1, 5, 6, 8, 9] 

------------------------------
## Why this is vastly superior to Base64

| Metric [8, 10, 11, 12] | Base64 String Embedding | Raw Binary JS + Brotli/Zstd |
|---|---|---|
| Payload Bloat | ❌ Adds 33% extra size to the source text before compression. | 0% bloat. The source code uses compact 8-bit integer bytes. |
| Browser Decoding Speed | ❌ Slow. The browser has to waste CPU loops converting the text string back to bytes. | ⚡ Instant. The browser passes the byte array directly into the Wasm engine. |
| Network Caching | ❌ Highly reliant on the browser's script cache. | ⚡ Automated perfectly by the CDN’s native compression negotiation (Accept-Encoding: zstd, br). |

## How to use this with your tooling
If you are using Vite, you can drop the Base64 method and use plugins like vite-plugin-wasm or standard ArrayBuffer bundling configurations. If you are using Trunk for a Leptos SPA, Trunk naturally outputs optimized static binary .wasm and .js chunks designed to be dropped onto a CDN that has Brotli or Zstd compression flipped on. [6, 13] 
Would you like help looking at your build tool configuration (Vite, Webpack, or Trunk) to ensure your bundle outputs are optimized for server-side Brotli/Zstd?

[1] [https://blog.cloudflare.com](https://blog.cloudflare.com/new-standards/)
[2] [https://www.reddit.com](https://www.reddit.com/r/HTML/comments/dn8wyu/alternative_to_base64/)
[3] [https://www.testmuai.com](https://www.testmuai.com/learning-hub/brotli-browser-support/)
[4] [https://mpetroff.net](https://mpetroff.net/2021/02/space-efficient-embedding-of-webassembly-in-javascript/)
[5] [https://www.vodien.com](https://www.vodien.com/learn/zstandard-vs-brotli/)
[6] [https://mykolaaleksandrov.dev](https://mykolaaleksandrov.dev/posts/2025/11/brotli-on-cdn-cloudflare-with-iis/)
[7] [https://pkg.go.dev](https://pkg.go.dev/go.pyspa.org/brbundle)
[8] [https://developers.cloudflare.com](https://developers.cloudflare.com/speed/optimization/content/compression/)
[9] [https://paulcalvano.com](https://paulcalvano.com/2024-03-19-choosing-between-gzip-brotli-and-zstandard-compression/)
[10] [https://codefarm0.medium.com](https://codefarm0.medium.com/understanding-base62-base64-and-other-encodings-a-developers-guide-e9eeabbaabbf)
[11] [https://nickb.dev](https://nickb.dev/blog/recommendations-when-publishing-a-wasm-library/)
[12] [https://www.testmuai.com](https://www.testmuai.com/learning-hub/zstd-browser-support/)
[13] [https://medium.com](https://medium.com/@jickpatel611/7-js-compression-choices-for-faster-pages-5165730c874f)


Yes, there are critical constraints where Base64 encoding remains the only viable mechanism for delivering embedded WebAssembly within a single JavaScript file.
The primary reasons you are forced into Base64 are source-code parser limits, network-free environment rules, and strict security sandboxing.
------------------------------
## 1. File Size Crashing the JavaScript Engine (V8/JSC limits)
If you try to write a large WebAssembly binary as a raw byte array literal (new Uint8Array([0, 97, 115...])), you will crash the browser or the build tool.

* The Problem: Every single comma and number inside [0, 12, 255...] is parsed by the JavaScript engine as an individual token/Node in the Abstract Syntax Tree (AST).
* The Failure: If your Wasm module is larger than a few hundred kilobytes, the JS parser will run out of memory, error out with Maximum call stack size exceeded, or freeze the browser during parsing.
* The Base64 Fix: A Base64 string is parsed as a single, unified string literal token. The JS engine processes it instantly as text without building an AST node for every single byte.

## 2. Network-Isolated & No-Disk RunTimes
Some execution environments completely block access to external HTTP fetches, local file systems (fs), and modern module loading features.

* Adobe CEP / UXP Plugins: Building extensions inside Creative Cloud applications (like Photoshop or Illustrator) forces you into strict, heavily locked-down JS environments. You cannot fetch local .wasm files easily due to security sandbox boundaries.
* Legacy Enterprise Containers: Old WebView wrappers or embedded IoT JavaScript runtimes (like JerryScript or Duktape) frequently lack full network stack exposure or support for advanced typed array manipulation syntax.

## 3. Copy-and-Paste Code Distributions
If you are distributing a script that developers must manually copy and paste into a single configuration field, Base64 text is your only choice.

* Google Tag Manager (GTM) Custom HTML: GTM allow users to inject custom scripts via a web UI. It only accepts text inputs and does not support binary uploads or external asset hosting for that specific container.
* User Scripts (Tampermonkey/Violentmonkey): If you are building a browser extension script that runs entirely inside a browser user-script manager, distributing a single .user.js text file is mandatory.

## 4. Content Security Policies (CSP) Blocking unsafe-eval
On highly secure websites, the Content Security Policy header might explicitly block creating WebAssembly out of raw memory arrays to prevent memory-injection attacks.

* The Policy Restriction: A policy like script-src 'self' with no wasm-unsafe-eval flag prevents WebAssembly.instantiate(Uint8Array) from executing.
* Some historical variations of Edge and Safari allowed Base64-encoded data URIs (data:application/wasm;base64,...) combined with WebAssembly.instantiateStreaming(), completely bypassing the restrictions applied to raw byte array evaluation.

------------------------------
## Summary Checklist

| If your deployment has... | Can you use Raw JS Byte Arrays? | Can you use External .wasm Fetch? | Winner |
|---|---|---|---|
| Large Wasm Module (>1MB) | ❌ No (Crashes JS Parser) | Yes | External Fetch |
| Strict No-Network Sandbox | Yes (If small) | ❌ No | Base64 String |
| Copy-Paste Input UI (e.g. GTM) | ❌ No (Text only input) | Yes (If hosted elsewhere) | Base64 String |

If you are dealing with one of these strict environments, let me know which runtime or platform you are targetting. I can help you write a highly optimized custom macro or build step to safely compress the Base64 string before it reaches the parser!
