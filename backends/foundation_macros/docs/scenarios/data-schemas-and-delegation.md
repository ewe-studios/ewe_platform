# Derive data schemas and delegate methods

Run these commands from the repository root.

```sh
cargo test --locked --profile uat -p foundation_macros --test foundation_macros_tests
```

```sh
cargo test --locked --profile uat -p foundation_core --test type_uuid_tests
```

```sh
cargo test --locked --profile uat -p foundation_nativeapis --features ipc --test vfs_async heterogeneous_message_types
```

The following fixtures give the setup and assertions.

- [Check Arrow and JSON schema fields](../../tests/units/foundation_macros_arrow_schema_tests.rs)
- [Round-trip Arrow records](../../tests/units/foundation_macros_arrow_serialize_tests.rs)
- [Delegate public methods and keep overrides](../../tests/units/foundation_macros_scaffold_tests.rs)
- [Match UUIDs for owned and external types](../../../foundation_core/tests/type_uuid_tests.rs)
- [Send and receive distinct message types](../../../foundation_nativeapis/tests/ipc/mod.rs)
