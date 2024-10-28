# Binaries
Within this directories are different binaries the platform project provides for differnt needs and usecases.

## Binaries

### [generate](./generate)

Create a new projects from ready made templates and geared towards making
setup super quick and easy.

For now the goal is to support Rust, WebAssemly + Rust and HTML projects,
soon Tauri templates will be added.

```bash
ewe_platform generate --project_name supercrate --template_name http-app --github_url 'https://github.com/ewestudios'`

2024-10-28T05:35:02.673898Z  INFO ewe_temple::files: Creating file: "/home/darkvoid/Labs/ewestudios/ewe_platform/supercrate/Cargo.toml"
2024-10-28T05:35:02.674052Z  INFO ewe_temple::files: Creating file: "/home/darkvoid/Labs/ewestudios/ewe_platform/supercrate/src/main.rs"

 pwd
/home/darkvoid/Labs/ewestudios/ewe_platform/supercrate
 tree
.
├── Cargo.toml
└── src
    └── main.rs

```
