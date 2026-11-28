# Run an Ewe application

Install the command from the repository root. Keep the Cargo binary directory on your executable search path.

```sh
cargo install --locked --path bin/ewe_ui
ewe_ui --help
```

## Create a project

Run `ewe_ui new` in a terminal for the wizard. To create the same project in a script, give all required arguments.

```sh
ewe_ui new --name FieldNotes --target web --target android --target ios --app-id com.example.fieldnotes --host server --output .
cd FieldNotes
ewe_ui task list
```

Repeat `--target` for each platform. Give a mobile application identifier for Android or iOS.
A web-only project needs a name and `--target web`.
The command creates a new directory and stops if the destination exists.
It records the targets and host settings in `ewe-ui.toml`.
The default `server` host connects to a service. The `on-device` host runs a shared Rust library in the mobile application.
[Generator options](../../src/main.rs) · [Project manifest and defaults](../../../../backends/foundation_ui_project/src/application.rs)

## Start a mobile application

Install the tools in [Building](building.md). Run the following commands from the generated project directory.
Copy the identifier from the first column of the device list.

```sh
ewe_ui devices list --target android
ewe_ui run --target android --device DEVICE_ID
ewe_ui devices list --target ios
ewe_ui run --target ios --device SIMULATOR_ID
```

Android lists emulators and connected devices. iOS uses simulators on macOS.
The run command waits for the device, builds a debug application, installs it, and launches it.
Use `ewe_ui devices start --target ios --device SIMULATOR_ID` to start a simulator without building the app.
Use `--timeout-seconds` to change the startup wait. Use `--poll-milliseconds` to change the Android readiness interval.
For a launch error, check the device state and application identifier.
For a server host, also check that the device can reach the service address in the generated host.
[Command implementation](../../src/operations.rs) · [Device discovery and startup](../../../../backends/foundation_ui_native/src/tasks/project/devices.rs)

## Serve a generated web build

Install Python 3. From the generated project root, build the web target and start a loopback preview server.

```sh
ewe_ui build --target web
python3 -m http.server 8080 --bind 127.0.0.1 --directory web/dist
```

On Windows, use `py -3` in place of `python3`.
Open the preview in your browser. [Local web preview](http://127.0.0.1:8080/)

## Run the shared Todo example

From the repository root, start the example server on the loopback interface with a separate database.
Use the first block in Bash or Zsh. Use the second block in PowerShell.

```sh
todo_run_dir=$(mktemp -d)
cargo run --locked --profile uat -p ui_todo --bin ui_todo -- \
  127.0.0.1:0 --store "$todo_run_dir/todo.sqlite3"
```

```powershell
$todoRunDir = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $todoRunDir | Out-Null
cargo run --locked --profile uat -p ui_todo --bin ui_todo -- 127.0.0.1:0 --store (Join-Path $todoRunDir 'todo.sqlite3')
```

Open the printed address in a browser. Keep the server running while you use the page.
For the mobile examples, follow the host setup for the platform.
[Todo server and browser](../../../../examples/ui_todo/README.md) · [Android hosts](../../../../examples/ui_todo/android/README.md) · [iPhone server host](../../../../examples/ui_todo/ios/todo_server/README.md) · [iPhone on-device host](../../../../examples/ui_todo/ios/todo_on_device/README.md)

Run application tasks from `examples/ui_todo`. The command reads the task declarations in its manifest.

```sh
ewe_ui task list
ewe_ui task run todo:prepare-ios --help
ewe_ui task run todo:test-ios --help
ewe_ui task run todo:test-android --help
```

Use `ewe_ui help --llm` for the complete command inventory from the current directory.
[Todo task declarations](../../../../examples/ui_todo/ewe-ui.toml) · [Todo task implementation](../../../../examples/ui_todo/tools/tasks/src/lib.rs)

## Open the Todo desktop window

Install the Tauri prerequisites for your operating system. Keep the Todo server running.
In a second shell at the repository root, run the desktop shell with the address printed by the server.
Replace the example port with the one your server prints.

```sh
cargo run --locked --manifest-path examples/ui_todo/src-tauri/Cargo.toml -- http://127.0.0.1:57841/
```

The desktop shell displays the page from the shared server.
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) · [Todo desktop example](../../../../examples/ui_todo/README.md#tauri-desktop-check)
