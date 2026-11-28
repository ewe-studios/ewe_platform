# Embed files and list mobile assets

Run these commands from the repository root.

```sh
cargo test --locked --profile uat -p foundation_macros --test test_macros_embeddeders
```

The following fixtures give the setup and assertions.

- [Use file and directory placeholders](../../tests/test_macros_embeddeders.rs)
- [Read generated mobile file metadata](../../tests/test_macros_embeddeders.rs)
