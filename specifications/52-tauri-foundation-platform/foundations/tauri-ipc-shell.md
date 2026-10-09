0. Tauri has the tuari.conf.js ( "withGlobalTauri": false) to enable injecting a tuari global, why are we not using it, its neeed for the webview to send commands and events to tauri.
1. Tauri invoke: has a invoke() method, so frontend can get a context in window (webview) and call it to invoke a command to tauri - tauri gets it and calls rust on our side. We register the ewe:// scheme, so we can also add a shim

We get to be able to register commands like below (tauri requires the command to not be pub has it has issues and names must be unique):

```rust

#[tauri::command]
fn my_custom_command() {
    println!("I was invoked from JavaScript!");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
    .invoke_handler(tauri::generate_handler![my_custom_command])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
```

We register it by adding it to the `invoke_handler` of the tuari builder.

Wasm specifically built for tauri can add the following to lock in the bindings:

```rust
#[wasm_bindgen]
extern "C" {
    // invoke without arguments
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke_without_args(cmd: &str) -> JsValue;

    // invoke with arguments (default)
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    // They need to have different names!
}

```

Functions can take arguments and our commands can return values which will be used to respond with a reply via serde, for larger response type return tauri Response object with an array buffer.

```rust
use tauri::ipc::Response;
#[tauri::command]
fn read_file() -> Response {
  let data = std::fs::read("/path/to/file").unwrap();
  tauri::ipc::Response::new(data)
}

```

To handle handlers that can fail we can return a `Result`: (As mentioned above, everything returned from commands must implement serde::Serialize)

````rust

#[tauri::command]
fn login(user: String, password: String) -> Result<String, String> {
  if user == "tauri" && password == "tauri" {
    // resolve
    Ok("logged_in".to_string())
  } else {
    // reject
    Err("invalid credentials".to_string())
  }
}


async commands are supported but has specific requirements - like not borrowing values and they are automatcially ran on an async task using `async_ruyntime::spawn` (tauri assync system):

Currently, you cannot simply include borrowed arguments in the signature of an asynchronous function. Some common examples of types like this are &str and State<'_, Data>

```rust
// Return a Result<String, ()> to bypass the borrowing issue
#[tauri::command]
async fn my_custom_command(value: &str) -> Result<String, ()> {
  // Call another async function and wait for it to finish
  some_async_function().await;
  // Note that the return value must be wrapped in `Ok()` now.
  Ok(format!(value))
}
````

We can invoke commands from javascript:

```javascript
invoke("my_custom_command", { value: "Hello, Async!" }).then(() =>
  console.log("Completed!"),
);
```

2. Tuari has Channels for data streaming in a command to the frontend

```rust
use tokio::io::AsyncReadExt;

#[tauri::command]
async fn load_image(path: std::path::PathBuf, reader: tauri::ipc::Channel<&[u8]>) {
  // for simplicity this example does not include error handling
  let mut file = tokio::fs::File::open(path).await.unwrap();

  let mut chunk = vec![0; 4096];

  loop {
    let len = file.read(&mut chunk).await.unwrap();
    if len == 0 {
      // Length of zero means end of file.
      break;
    }
    reader.send(&chunk).unwrap();
  }
}

```

You can get a handle to the webview window and app handle instance in commands too:

```rust
#[tauri::command]
async fn my_custom_command(webview_window: tauri::WebviewWindow) {
  println!("WebviewWindow: {}", webview_window.label());
}

#[tauri::command]
async fn my_custom_command(app_handle: tauri::AppHandle) {
  let app_dir = app_handle.path().app_dir();
  use tauri::GlobalShortcutManager;
  app_handle.global_shortcut_manager().register("CTRL + U", move || {});
}

```

We can store and access state in the tauri builder and in the command functions:

```rust
struct MyState(String);

#[tauri::command]
fn my_custom_command(state: tauri::State<MyState>) {
  assert_eq!(state.0 == "some state value", true);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(MyState("some state value".into()))
    .invoke_handler(tauri::generate_handler![my_custom_command])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
```

You can also access the raw request object:

```rust

#[derive(Debug, thiserror::Error)]
enum Error {
  #[error("unexpected request body")]
  RequestBodyMustBeRaw,
  #[error("missing `{0}` header")]
  MissingHeader(&'static str),
}

impl serde::Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::ser::Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}

#[tauri::command]
fn upload(request: tauri::ipc::Request) -> Result<(), Error> {
  let tauri::ipc::InvokeBody::Raw(upload_data) = request.body() else {
    return Err(Error::RequestBodyMustBeRaw);
  };
  let Some(authorization_header) = request.headers().get("Authorization") else {
    return Err(Error::MissingHeader("Authorization"));
  };

  // upload...

  Ok(())
}

```

on the frontend side you can send raw request body (ArrayBuffer or Uint8Array) as the payload argument and include headers like below:

```js
const data = new Uint8Array([1, 2, 3]);
await __TAURI__.core.invoke("upload", data, {
  headers: {
    Authorization: "apikey",
  },
});
```

2. Tauri events: tauri has events via Rust::emit() and js:listen()

The event system is a simpler communication mechanism between your frontend and the Rust. Unlike commands, events are not type safe, are always async, cannot return values and only supports JSON payloads.

Using npm style imports: you can get the emit object from tauri and get the webview window and both can emit events.

```js
import { emit } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// emit(eventName, payload)
emit("file-selected", "/path/to/file");

const appWebview = getCurrentWebviewWindow();
appWebview.emit("route-changed", { url: window.location.href });
```

You can also selectively send an event to a specific webview:

```js
import { emitTo } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// emitTo(webviewLabel, eventName, payload)
emitTo("settings", "settings-update-requested", {
  key: "notification",
  value: "all",
});

const appWebview = getCurrentWebviewWindow();
appWebview.emitTo("editor", "file-changed", {
  path: "/path/to/file",
  contents: "file contents",
});
```

a. To trigger a global event you can use the event.emit or the WebviewWindow#emit functions, Global events are delivered to all listeners
b. Webview target events are not sent to global event listeners, so you must specifically listen for them in the webview and provide the target { target: { kind: 'Any' } } option to event.listen.

```js
import { listen } from "@tauri-apps/api/event";
listen(
  "state-changed",
  (event) => {
    console.log("got state changed event", event);
  },
  {
    target: { kind: "Any" },
  },
);
```

You can lisen to global events:events

```js

import { listen } from '@tauri-apps/api/event';

type DownloadStarted = {
  url: string;
  downloadId: number;
  contentLength: number;
};

listen<DownloadStarted>('download-started', (event) => {
  console.log(
    `downloading ${event.payload.contentLength} bytes from ${event.payload.url}`
  );
});

```

You can listen to webview specific events:

```js
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

const appWebview = getCurrentWebviewWindow();
appWebview.listen <
  string >
  ("logged-in",
  (event) => {
    localStorage.setItem("session-token", event.payload);
  });
```

The listen function keeps the event listener registered for the entire lifetime of the application. To stop listening on an event you can use the unlisten function which is returned by the listen function:

```js
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen("download-started", (event) => {});
unlisten();
```

Always use the unlisten function when your execution context goes out of scope such as when a component is unmounted.

When the page is reloaded, or you navigate to another URL the listeners are unregistered automatically. This does not apply to a Single Page Application (SPA) router though.

On the rust side you can listen for events via the app instance from the builder:

a. Listening to global events

```rust
use tauri::Listener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      app.listen("download-started", |event| {
        if let Ok(payload) = serde_json::from_str::<DownloadStarted>(&event.payload()) {
          println!("downloading {}", payload.url);
        }
      });
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

```

b. Listening to webview specific events

The event payload can be any serializable type that also implements Clone. Let’s enhance the download event example by using an object to emit more information in each event:

```rust

use tauri::{AppHandle, Emitter};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadStarted<'a> {
  url: &'a str,
  download_id: usize,
  content_length: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
  download_id: usize,
  chunk_length: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadFinished {
  download_id: usize,
}

#[tauri::command]
fn download(app: AppHandle, url: String) {
  let content_length = 1000;
  let download_id = 1;

  app.emit("download-started", DownloadStarted {
    url: &url,
    download_id,
    content_length
  }).unwrap();

  for chunk_length in [15, 150, 35, 500, 300] {
    app.emit("download-progress", DownloadProgress {
      download_id,
      chunk_length,
    }).unwrap();
  }

  app.emit("download-finished", DownloadFinished { download_id }).unwrap();
}
```

```rust

use tauri::{Listener, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      let webview = app.get_webview_window("main").unwrap();
      webview.listen("logged-in", |event| {
        let session_token = event.data;
        // save token..
      });
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

```

The listen function keeps the event listener registered for the entire lifetime of the application. To stop listening on an event you can use the unlisten function:

```rust
// unlisten outside of the event handler scope:
let event_id = app.listen("download-started", |event| {});
app.unlisten(event_id);

// unlisten when some event criteria is matched
let handle = app.handle().clone();
app.listen("status-changed", |event| {
  if event.data == "ready" {
    handle.unlisten(event.id);
  }
});

```

You can also do one time listen to an event:

````rust

app.once("ready", |event| {
  println!("app is ready");
});


### Calling the frontend from rust

Global events can be emiited from rust via the emitter from the app handle:


```rust
use tauri::{AppHandle, Emitter};

#[tauri::command]
fn download(app: AppHandle, url: String) {
  app.emit("download-started", &url).unwrap();
  for progress in [1, 15, 50, 80, 100] {
    app.emit("download-progress", progress).unwrap();
  }
  app.emit("download-finished", &url).unwrap();
}

```


Webview specific event can also be done in similar via the `emit_to` method:

```rust
use tauri::{AppHandle, Emitter};

#[tauri::command]
fn login(app: AppHandle, user: String, password: String) {
  let authenticated = user == "tauri-apps" && password == "tauri";
  let result = if authenticated { "loggedIn" } else { "invalidCredentials" };
  app.emit_to("login", "login-result", result).unwrap();
}

```

You can also mass deliver events to multiple webivews:

It is also possible to trigger an event to a list of webviews by calling Emitter#emit_filter. In the following example we emit a open-file event to the main and file-viewer webviews:

```rust
use tauri::{AppHandle, Emitter, EventTarget};

#[tauri::command]
fn open_file(app: AppHandle, path: std::path::PathBuf) {
  app.emit_filter("open-file", path, |target| match target {
    EventTarget::WebviewWindow { label } => label == "main" || label == "file-viewer",
    _ => false,
  }).unwrap();
}
```

Webview-specific events are not triggered to regular global event listeners. To listen to any event you must use the listen_any function instead of listen, which defines the listener to act as a catch-all for emitted events.


### Notes

#### Don’t call unlisten() before the listener resolves

The listen function returns a Promise that resolves to the unlisten handle. If you call unlisten synchronously before the Promise resolves, the handler will be removed immediately and you won’t receive any events:

#### Timing in setup hooks

In frameworks like React, Vue, and Svelte, the setup or mount hook runs before the component is fully rendered. If you listen for events during setup, make sure the event handler does not depend on DOM elements that haven’t been rendered yet, or defer the listener registration to an effect/hook that runs after mount.

#### Event ordering and async listeners

Event listeners are called in the order they are registered, but if a listener is async and the event emitter sends multiple events in rapid succession, the listeners may process events out of order. For ordered, high-throughput data delivery, consider using Channels instead of the event system.
````

## Channels

Channels are designed to be fast and deliver ordered data. They are used internally for streaming operations such as download progress, child process output and WebSocket messages

```rust

use tauri::{AppHandle, ipc::Channel};
use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "event", content = "data")]
enum DownloadEvent<'a> {
  Started {
    url: &'a str,
    download_id: usize,
    content_length: usize,
  },
  Progress {
    download_id: usize,
    chunk_length: usize,
  },
  Finished {
    download_id: usize,
  },
}

#[tauri::command]
fn download(app: AppHandle, url: String, on_event: Channel<DownloadEvent>) {
  let content_length = 1000;
  let download_id = 1;

  on_event.send(DownloadEvent::Started {
    url: &url,
    download_id,
    content_length,
  }).unwrap();

  for chunk_length in [15, 150, 35, 500, 300] {
    on_event.send(DownloadEvent::Progress {
      download_id,
      chunk_length,
    }).unwrap();
  }

  on_event.send(DownloadEvent::Finished { download_id }).unwrap();
}

```

then we ensure to provide the channel to the invoke function to receive the messages:

```typescript
import { invoke, Channel } from "@tauri-apps/api/core";

type DownloadEvent =
  | {
      event: "started";
      data: {
        url: string;
        downloadId: number;
        contentLength: number;
      };
    }
  | {
      event: "progress";
      data: {
        downloadId: number;
        chunkLength: number;
      };
    }
  | {
      event: "finished";
      data: {
        downloadId: number;
      };
    };

const onEvent = new Channel<DownloadEvent>();
onEvent.onmessage = (message) => {
  console.log(`got download event ${message.event}`);
};

await invoke("download", {
  url: "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-schema-generator/schemas/config.schema.json",
  onEvent,
});
```

### Evaluating Javascript

To directly execute any JavaScript code on the webview context you can use the WebviewWindow#eval function:

```rust
use tauri::Manager;

tauri::Builder::default()
  .setup(|app| {
    let webview = app.get_webview_window("main").unwrap();
    webview.eval("console.log('hello from Rust')")?;
    Ok(())
  })

```

If the script to be evaluated is not so simple and must use input from Rust objects we recommend using the serialize-to-javascript crate.

## Bundling Additional files

You may need to include additional files in your application bundle that aren’t part of your frontend (your frontendDist) directly or which are too big to be inlined into the binary. We call these files resources.

### Configuration

To bundle the files of your choice, add the resources property to the bundle object in your tauri.conf.json file.

```json
{
  "bundle": {
    "resources": [
      // Will be placed to `$RESOURCE/path/to/some-file.txt`
      "./path/to/some-file.txt",

      // The root in an absolute path will be replaced by `_root_`,
      // so `textfile.txt` will be placed to `$RESOURCE/_root_/absolute/path/to/textfile.txt`
      "/absolute/path/to/textfile.txt",

      // `..` in a relative path will be replaced by `_up_`,
      // so `jsonfile.json` will be placed to `$RESOURCE/_up_/relative/path/to/textfile.txt`,
      "../relative/path/to/jsonfile.json",

      // If the path is a directory, the entire directory will be copied to the `$RESOURCE` directory,
      // preserving the original structures, for example:
      //   - `some-folder/file.txt`                   -> `$RESOURCE/some-folder/file.txt`
      //   - `some-folder/another-folder/config.json` -> `$RESOURCE/some-folder/another-folder/config.json`
      // This is the same as `some-folder/**/*`
      "some-folder/",

      // You can also include multiple files at once through glob patterns.
      // All the `.md` files inside `resources` will be placed to `$RESOURCE/resources/`,
      // preserving their original directory structures, for example:
      //   - `resources/index.md`      -> `$RESOURCE/resources/index.md`
      //   - `resources/docs/setup.md` -> `$RESOURCE/resources/docs/setup.md`
      "resources/**/*.md"
    ]
  }
}
```

The bundled files will be in $RESOURCES/ with the original directory structure preserved, for example: ./path/to/some-file.txt -> $RESOURCE/path/to/some-file.txt

To fine control where the files will get copied to, use a map instead:

```json
{
  "bundle": {
    "resources": {
      // `textfile.txt` will be placed to `$RESOURCE/resources/textfile.txt`
      "/absolute/path/to/textfile.txt": "resources/textfile.txt",

      // `jsonfile.json` will be placed to `$RESOURCE/resources/jsonfile.json`
      "relative/path/to/jsonfile.json": "resources/jsonfile.json",

      // Copy the entire directory to `$RESOURCE`, preserving the original structures,
      // the target is "" which means it will be placed directly in the resource directory `$RESOURCE`, for example:
      //   - `resources/file.txt`                -> `$RESOURCE/file.txt`
      //   - `resources/some-folder/config.json` -> `$RESOURCE/some-folder/config.json`
      "resources/": "",

      // When using glob patterns, the behavior is different from the list one,
      // all the matching files will be placed to the target directory without preserving the original file structures
      // for example:
      //   - `docs/index.md`         -> `$RESOURCE/website-docs/index.md`
      //   - `docs/plugins/setup.md` -> `$RESOURCE/website-docs/setup.md`
      "docs/**/*md": "website-docs/"
    }
  }
}
```

The path in the API calls can be either a normal relative path like folder/json*file.json that resolves to $RESOURCE/folder/json_file.json, or a paths like ../relative/folder/toml_file.toml that resolves to $RESOURCE/\_up*/relative/folder/toml_file.toml, these APIs use the same rules as you write tauri.conf.json > bundle > resources, for example:

```json
{
  "bundle": {
    "resources": ["folder/json_file.json", "../relative/folder/toml_file.toml"]
  }
}
```

```rust
let json_path = app.path().resolve("folder/json_file.json", BaseDirectory::Resource)?;
let toml_path = app.path().resolve("../relative/folder/toml_file.toml", BaseDirectory::Resource)?;

```

To resolve the resource file paths:

On the Rust side, you need an instance of the PathResolver which you can get from App and AppHandle, then call PathResolver::resolve:

```rust
tauri::Builder::default()
  .setup(|app| {
    let resource_path = app.path().resolve("lang/de.json", BaseDirectory::Resource)?;
    Ok(())
  })

  #[tauri::command]
fn hello(handle: tauri::AppHandle) {
  let resource_path = handle.path().resolve("lang/de.json", BaseDirectory::Resource)?;
}

```

````

On the JS side:

```js
import { resolveResource } from '@tauri-apps/api/path';
const resourcePath = await resolveResource('lang/de.json');

````

Under Android: Currently the resources are stored in the APK as assets so the return value of those APIs are not normal file system paths, we use a special URI prefix asset://localhost/ here that can be used with the fs plugin, with that, you can read the files through FsExt::fs like this:

On the Rust side, you need an instance of the PathResolver which you can get from App and AppHandle:

```rust
let resource_path = app.path().resolve("lang/de.json", BaseDirectory::Resource).unwrap();
let json = app.fs().read_to_string(&resource_path);

```

```rust
tauri::Builder::default()
  .setup(|app| {
    // The path specified must follow the same syntax as defined in
    // `tauri.conf.json > bundle > resources`
    let resource_path = app.path().resolve("lang/de.json", BaseDirectory::Resource)?;

    let json = std::fs::read_to_string(&resource_path).unwrap();
    // Or when dealing with Android, use the file system plugin instead
    // let json = app.fs().read_to_string(&resource_path);

    let lang_de: serde_json::Value = serde_json::from_str(json).unwrap();

    // This will print 'Guten Tag!' to the terminal
    println!("{}", lang_de.get("hello").unwrap());

    Ok(())
  })


#[tauri::command]
fn hello(handle: tauri::AppHandle) -> String {
    let resource_path = handle.path().resolve("lang/de.json", BaseDirectory::Resource)?;

    let json = std::fs::read_to_string(&resource_path).unwrap();
    // Or when dealing with Android, use the file system plugin instead
    // let json = handle.fs().read_to_string(&resource_path);

    let lang_de: serde_json::Value = serde_json::from_str(json).unwrap();

    lang_de.get("hello").unwrap()
}


```

On the js side:

For the JavaScript side, you can either use a command like the one above and call it through await invoke('hello') or access the files using the fs plugin.

When using the fs plugin, in addition to the basic setup, you’ll also need to configure the access control list to enable any plugin APIs you need as well as the permissions to access the $RESOURCE folder:

Here we use fs:allow-resource-read-recursive to allow for full recursive read access to the complete $RESOURCE folder, files, and subdirectories. For more information, read Scope Permissions for other options, or Scopes for more fine-grained control.

```js

{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "main-capability",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "fs:allow-read-text-file",
    "fs:allow-resource-read-recursive"
  ]
}
```

```js
import { resolveResource } from "@tauri-apps/api/path";
import { readTextFile } from "@tauri-apps/plugin-fs";

const resourcePath = await resolveResource("lang/de.json");
const langDe = JSON.parse(await readTextFile(resourcePath));
console.log(langDe.hello); // This will print 'Guten Tag!' to the devtools console
```

### Embedding External binaries

You may need to embed external binaries to add additional functionality to your application or prevent users from installing additional dependencies (e.g., Node.js or Python). We call this binary a sidecar.

Binaries are executables written in any programming language. Common use cases are Python CLI applications or API servers bundled using pyinstaller.

To bundle the binaries of your choice, you can add the externalBin property to the bundle object in your tauri.conf.json. The externalBin configuration expects a list of strings targeting binaries either with absolute or relative paths.

Here is a Tauri configuration snippet to illustrate a sidecar configuration:

```json
{
  "bundle": {
    "externalBin": [
      "/absolute/path/to/sidecar",
      "../relative/path/to/binary",
      "binaries/my-sidecar"
    ]
  }
}
```

The relative paths are relative to the tauri.conf.json file which is in the src-tauri directory. So binaries/my-sidecar would represent <PROJECT ROOT>/src-tauri/binaries/my-sidecar.

To make the external binary work on each supported architecture, a binary with the same name and a -$TARGET_TRIPLE suffix must exist on the specified path. For instance, "externalBin": ["binaries/my-sidecar"] requires a src-tauri/binaries/my-sidecar-x86_64-unknown-linux-gnu executable on Linux or src-tauri/binaries/my-sidecar-aarch64-apple-darwin on Mac OS with Apple Silicon.

You can find your current platform’s -$TARGET_TRIPLE suffix by running the following command:

```bash
rustc --print host-tuple
```

On the Rust side, import the tauri_plugin_shell::ShellExt trait and call the shell().sidecar() function on the AppHandle:

```rust
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;
use tauri::Emitter;

let sidecar_command = app.shell().sidecar("my-sidecar").unwrap();
let (mut rx, mut child) = sidecar_command
  .spawn()
  .expect("Failed to spawn sidecar");

tauri::async_runtime::spawn(async move {
  // read events such as stdout
  while let Some(event) = rx.recv().await {
    if let CommandEvent::Stdout(line_bytes) = event {
      let line = String::from_utf8_lossy(&line_bytes);
      app
        .emit("message", Some(format!("'{}'", line)))
        .expect("failed to emit event");
      // write to stdin
      child.write("message from Rust\n".as_bytes()).unwrap();
    }
  }
});

```

The sidecar() function expects just the filename, NOT the whole path configured in the externalBin array.

```json
{
  "bundle": {
    "externalBin": ["binaries/app", "my-sidecar", "../scripts/sidecar"]
  }
}
```

The appropriate way to execute the sidecar is by calling app.shell().sidecar(name) where name is either "app", "my-sidecar" or "sidecar" instead of "binaries/app" for instance.

#### Running it from javascript

When running the sidecar, Tauri requires you to give the sidecar permission to run the execute or spawn method on the child process. To grant this permission, go to the file <PROJECT ROOT>/src-tauri/capabilities/default.json and add the section below to the permissions array. Don’t forget to name your sidecar according to the relative path mentioned earlier.

```json
{
  "permissions": [
    "core:default",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        {
          "name": "binaries/app",
          "sidecar": true
        }
      ]
    }
  ]
}
```

The shell:allow-execute identifier is used because the sidecar’s child process will be started using the command.execute() method. To run it with command.spawn(), you need to change the identifier to shell:allow-spawn or add another entry to the array with the same structure as the one above, but with the identifier set to shell:allow-spawn. In the JavaScript code, import the Command class from the @tauri-apps/plugin-shell module and use the sidecar static method. The string provided to Command.sidecar must match one of the strings defined in the externalBin configuration array.

```js
import { Command } from "@tauri-apps/plugin-shell";
const command = Command.sidecar("binaries/my-sidecar");
const output = await command.execute();
```

#### Passing arguments

You can pass arguments to Sidecar commands just like you would for running normal Command.

Arguments can be either static (e.g. -o or serve) or dynamic (e.g. <file_path> or localhost:<PORT>). A value of true will allow any arguments to be passed to the command. false will disable all arguments. If neither true or false is set, you define the arguments in the exact order in which you’d call them. Static arguments are defined as-is, while dynamic arguments can be defined using a regular expression.

First, define the arguments that need to be passed to the sidecar command in src-tauri/capabilities/default.json:

```

{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        {
          "args": [
            "arg1",
            "-a",
            "--arg2",
            {
              "validator": "\\S+"
            }
          ],
          "name": "binaries/my-sidecar",
          "sidecar": true
        }
      ]
    }
  ]
}

```

Then, to call the sidecar command, simply pass in all the arguments as an array.

```rust
use tauri_plugin_shell::ShellExt;
#[tauri::command]
async fn call_my_sidecar(app: tauri::AppHandle) {
  let sidecar_command = app
    .shell()
    .sidecar("my-sidecar")
    .unwrap()
    .args(["arg1", "-a", "--arg2", "any-string-that-matches-the-validator"]);
  let (mut _rx, mut _child) = sidecar_command.spawn().unwrap();
}

```

In Javascript:

```js
import { Command } from "@tauri-apps/plugin-shell";
// notice that the args array matches EXACTLY what is specified in `capabilities/default.json`.
const command = Command.sidecar("binaries/my-sidecar", [
  "arg1",
  "-a",
  "--arg2",
  "any-string-that-matches-the-validator",
]);
const output = await command.execute();
```

## State Management

In a Tauri application, you often need to keep track of the current state of your application or manage the lifecycle of things associated with it. Tauri provides an easy way to manage the state of your application using the Manager API, and read it when commands are called.

```rust
use tauri::{Builder, Manager};

struct AppData {
  welcome_message: &'static str,
}

fn main() {
  Builder::default()
    .setup(|app| {
      app.manage(AppData {
        welcome_message: "Welcome to Tauri!",
      });
      Ok(())
    })
    .run(tauri::generate_context!())
    .unwrap();
}

```

You can later access your state with any type that implements the Manager trait, for example the App instance:

```rust
let data = app.state::<AppData>();

```

### Mutability

In Rust, you cannot directly mutate values which are shared between multiple threads or when ownership is controlled through a shared pointer such as Arc (or Tauri’s State). Doing so could cause data races (for example, two writes happening simultaneously).

To work around this, you can use a concept known as interior mutability. For example, the standard library’s Mutex can be used to wrap your state. This allows you to lock the value when you need to modify it, and unlock it when you are done.

```rust
use std::sync::Mutex;

use tauri::{Builder, Manager};

#[derive(Default)]
struct AppState {
  counter: u32,
}

fn main() {
  Builder::default()
    .setup(|app| {
      app.manage(Mutex::new(AppState::default()));
      Ok(())
    })
    .run(tauri::generate_context!())
    .unwrap();
}

```

The state can now be modified by locking the mutex:

```rust
let state = app.state::<Mutex<AppState>>();

// Lock the mutex to get mutable access:
let mut state = state.lock().unwrap();

// Modify the state:
state.counter += 1;

```

At the end of the scope, or when the MutexGuard is otherwise dropped, the mutex is unlocked automatically so that other parts of your application can access and mutate the data within.

To access state in commands:

```rust
#[tauri::command]
fn increase_counter(state: State<'_, Mutex<AppState>>) -> u32 {
  let mut state = state.lock().unwrap();
  state.counter += 1;
  state.counter
}

#[tauri::command]
async fn increase_counter(state: State<'_, Mutex<AppState>>) -> Result<u32, ()> {
  let mut state = state.lock().await;
  state.counter += 1;
  Ok(state.counter)
}

```

If you are using async commands and want to use Tokio’s async Mutex, you can set it up the same way and access the state like this. Note that the return type must be Result if you use asynchronous commands.

#### Do you need Arc?

It’s common to see Arc used in Rust to share ownership of a value across multiple threads (usually paired with a Mutex in the form of Arc<Mutex<T>>). However, you don’t need to use Arc for things stored in State because Tauri will do this for you.

In case State’s lifetime requirements prevent you from moving your state into a new thread you can instead move an AppHandle into the thread and then retrieve your state as shown below in the “Access state with the Manager trait” section. AppHandles are deliberately cheap to clone for use-cases like this.

#### when to use an async mutex ?

To quote the Tokio documentation, it’s often fine to use the standard library’s Mutex instead of an async mutex such as the one Tokio provides:

Contrary to popular belief, it is ok and often preferred to use the ordinary Mutex from the standard library in asynchronous code … The primary use case for the async mutex is to provide shared mutable access to IO resources such as a database connection.

It’s a good idea to read the linked documentation fully to understand the trade-offs between the two. One reason you would need an async mutex is if you need to hold the MutexGuard across await points.

#### Access state with the Manager trait?

Sometimes you may need to access the state outside of commands, such as in a different thread or in an event handler like on_window_event. In such cases, you can use the state() method of types that implement the Manager trait (such as the AppHandle) to get the state:

```rust

use std::sync::Mutex;
use tauri::{Builder, Window, WindowEvent, Manager};

#[derive(Default)]
struct AppState {
  counter: u32,
}

// In an event handler:
fn on_window_event(window: &Window, _event: &WindowEvent) {
    // Get a handle to the app so we can get the global state.
    let app_handle = window.app_handle();
    let state = app_handle.state::<Mutex<AppState>>();

    // Lock the mutex to mutably access the state.
    let mut state = state.lock().unwrap();
    state.counter += 1;
}

fn main() {
  Builder::default()
    .setup(|app| {
      app.manage(Mutex::new(AppState::default()));
      Ok(())
    })
    .on_window_event(on_window_event)
    .run(tauri::generate_context!())
    .unwrap();
}
```
