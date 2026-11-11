# Vendored sqlite-vec source

These files build the sqlite-vec loadable extension from source.

- `sqlite-vec.c`, `sqlite-vec.h` — sqlite-vec v0.1.9
  (https://github.com/asg017/sqlite-vec, tag `v0.1.9`). The header is generated
  from `sqlite-vec.h.tmpl` with `VERSION=0.1.9`.
- `sqlite3ext.h` — the standard SQLite extension API header, from the SQLite
  3.45.1 amalgamation (https://sqlite.org/2024/sqlite-amalgamation-3450100.zip).
  Public domain. 3.45.1 matches the SQLite version bundled with `libsql-ffi`
  0.5.0 (the version `libsql-sys` 0.8.0 uses).
- `LICENSE-MIT`, `LICENSE-APACHE` — the sqlite-vec licenses.

`build.rs` compiles `sqlite-vec.c` into a loadable extension for the current
target and sets `VEC0_SO` to its path. The extension is built only when the
`sqlite-vec` feature is on.
