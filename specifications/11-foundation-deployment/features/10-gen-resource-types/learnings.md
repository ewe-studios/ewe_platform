# Gen Resource Types - Learnings

This document captures key learnings from implementing and fixing the `gen_resource_types` generator for GCP and other cloud provider OpenAPI specs.

---

## 2026-04-05: Rust Type Name Conflicts

### Problem

The `gen_resource_types` generator was producing Rust code that failed to compile due to naming conflicts between generated struct names and Rust built-in types:

1. **`Value` conflict**: APIs defining a schema named `Value` generated `pub struct Value { ... }` which conflicted with `use serde_json::Value;`
2. **`Option` conflict**: APIs defining a schema named `Option` generated `pub struct Option { ... }` which shadowed Rust's `core::option::Option<T>`

### Example Errors

```rust
// Generated code (buggy):
use serde_json::Value;  // Import

pub struct Value {  // Error: Value redefined
    pub field: String,
}

// And:
pub struct Option {  // Error: shadows core::option::Option
    pub default: String,
}

#[derive(Deserialize)]
pub struct SomeType {
    #[serde(default)]
    pub opt: Option,  // Error: Option doesn't implement Default (it's the struct!)
}
```

### Root Cause

The OpenAPI specs for GCP and other providers include schemas with names that overlap with:
- Rust standard library types (`Option`, `Result`, `String`, `Vec`, `Box`)
- Common imported types (`serde_json::Value`)

The generator was naively converting schema names to PascalCase without checking for conflicts.

### Solution

Applied a multi-layered fix:

#### 1. Rename Conflicting Structs

Added a match statement to rename schemas that conflict with Rust built-ins:

```rust
let rust_name = match rust_name.as_str() {
    "Option" => "ApiOption".to_string(),
    "Value" => "ApiValue".to_string(),
    "Result" => "ApiResult".to_string(),
    "Ok" => "ApiOk".to_string(),
    "Err" => "ApiErr".to_string(),
    "Some" => "ApiSome".to_string(),
    "None" => "ApiNone".to_string(),
    "Box" => "ApiBox".to_string(),
    "Vec" => "ApiVec".to_string(),
    "String" => "ApiString".to_string(),
    _ => rust_name,
};
```

This renaming is applied both when generating struct definitions and when resolving `$ref` references.

#### 2. Use Fully Qualified Paths

Changed all standard type references to use fully qualified paths to avoid shadowing:

```rust
// Before (vulnerable to shadowing):
Option<T>
Vec<T>
Box<T>

// After (shadow-proof):
::core::option::Option<T>
::std::vec::Vec<T>
::std::boxed::Box<T>
```

#### 3. Remove Conflicting Imports

Removed `use serde_json::Value;` from the generated file header and always use the fully qualified `serde_json::Value` path.

#### 4. Add Cross-Module Imports

Added `use super::*;` to generated files so types from sibling modules (e.g., `compute.rs` referencing types in `container.rs`) are available.

### Files Changed

- `bin/platform/src/gen_resource_types/mod.rs`:
  - `extract_resource()` - Added struct name renaming
  - `resolve_ref()` - Added type name renaming for references
  - `schema_to_rust_type()` - Use fully qualified paths for `Option`, `Vec`, `Box`
  - `generate_file_header()` - Removed `use serde_json::Value;`, added `use super::*;`
  - `generate_struct()` - Use `::core::option::Option<...>` for optional fields

### APIs Affected

The following GCP APIs had schemas with conflicting names:

| Schema Name | APIs Using It | Renamed To |
|-------------|---------------|------------|
| `Value` | datastore, firestore, fitness, sqladmin, cloudsearch, content | `ApiValue` |
| `Option` | forms | `ApiOption` |

### Verification

After the fix:
- `pub struct ApiValue { ... }` is generated instead of `pub struct Value { ... }`
- `pub struct ApiOption { ... }` is generated instead of `pub struct Option { ... }`
- All field types use fully qualified paths: `::core::option::Option<ApiValue>`
- Compilation succeeds for previously failing modules

---

## 2026-04-05: Forward Reference Handling

### Problem

Rust requires types to be defined before use. When an OpenAPI schema references another schema that appears later in the generated file, Rust reports "cannot find type" errors.

### Example Error

```rust
// Generated code (order matters in Rust):
pub struct Chunk {
    pub metadata: Box<GoogleCloudAiplatformV1Metadata>,  // Error: not defined yet
}

// ... 19,000 lines later ...

pub struct GoogleCloudAiplatformV1UrlMetadata {
    // This exists, but GoogleCloudAiplatformV1Metadata does not
}
```

### Solution: Box Wrapping

Wrapped all custom type references in `::std::boxed::Box<...>` to enable forward references. Rust allows forward references through indirection (pointers/boxes).

```rust
// Generator code:
if !ty.starts_with("serde_json::") && !ty.starts_with("::std::vec::Vec<") {
    format!("::std::boxed::Box<{ty}>")
} else {
    ty
}
```

### Known Limitation: Missing Types

Some OpenAPI specs reference schemas that don't exist in the generated output. For example, `aiplatform.rs` references `GoogleCloudAiplatformV1Metadata` but only `GoogleCloudAiplatformV1MetadataSchema` and `GoogleCloudAiplatformV1MetadataStore` are defined.

This occurs when:
1. The referenced schema has no `properties` field (filtered out by generator)
2. The schema is defined in a different API module
3. The OpenAPI spec has a dangling reference

**Workaround**: These APIs require manual intervention or spec fixes.

---

## Best Practices Established

1. **Always check for Rust keyword/built-in conflicts** - Any generator producing Rust code must handle names like `Option`, `Result`, `String`, etc.

2. **Use fully qualified paths** - Even if imports seem convenient, fully qualified paths (`::core::option::Option`) are immune to shadowing.

3. **Wrap custom types in Box** - This enables forward references and avoids ordering issues in generated files.

4. **Add `use super::*;` for cross-module references** - When generating multiple files in a module hierarchy, this ensures types from sibling modules are available.

5. **Test with real API specs** - Synthetic test cases may not catch edge cases like schemas named `Value` or `Option`.

---

## Related Documentation

- `bin/platform/src/gen_resource_types/mod.rs` - Main generator implementation
- `backends/foundation_deployment/src/providers/gcp/resources/` - Generated GCP resource types
