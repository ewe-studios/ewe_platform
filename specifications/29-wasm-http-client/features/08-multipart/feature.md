---
feature: "Multipart"
description: "Form/Part types for multipart/form-data, FormData construction in JS via ABI, Blob creation, filename and MIME type support"
status: "pending"
priority: "medium"
depends_on: ["body-types"]
estimated_effort: "medium"
created: 2026-05-18
last_updated: 2026-05-18
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Multipart

## Overview

This feature implements multipart/form-data support — the `Form` and `Part` types for building multipart requests, and the JS-side FormData construction that converts them into a `web_sys::FormData`-equivalent via megatron.js. Feature-gated: `http-multipart`.

## Language Stack

| Language | Purpose | Skill Location |
|----------|---------|----------------|
| Rust | Form/Part types, MIME handling | `.agents/skills/rust-clean-code/skill.md` |
| JavaScript | FormData/Blob construction | Follow existing megatron.js patterns |

## Architecture (COMPREHENSIVE)

### File Structure

- `backends/foundation_wasm/src/http/multipart.rs` — Form, Part, PartMetadata, PartProps

### Type Hierarchy

```rust
pub struct Form {
    inner: FormParts<Part>,
}

pub struct Part {
    meta: PartMetadata,
    value: Body,
}

pub(crate) struct FormParts<P> {
    fields: alloc::vec::Vec<(alloc::borrow::Cow<'static, str>, P)>,
}

pub(crate) struct PartMetadata {
    mime: Option<Mime>,
    file_name: Option<alloc::borrow::Cow<'static, str>>,
}
```

**Note**: We don't use the `mime` crate (adds dependency). Instead, MIME types are stored as `&'static str` or `String`.

### Form Methods

```rust
impl Form {
    /// Creates a new empty Form.
    pub fn new() -> Self;

    /// Adds a text field to the form.
    pub fn text(self, name: impl Into<Cow<'static, str>>, value: impl Into<String>) -> Self;

    /// Adds a part to the form.
    pub fn part(self, name: impl Into<Cow<'static, str>>, part: Part) -> Self;

    /// Converts the form to a FormData ExternalPointer via JS-side construction.
    /// Returns an ExternalPointer to a JS FormData object.
    pub(crate) fn into_form_data(&self) -> HttpResult<ExternalPointer>;
}
```

### Part Methods

```rust
impl Part {
    /// Creates a text part.
    pub fn text(value: impl Into<String>) -> Self;

    /// Creates a bytes part.
    pub fn bytes(value: impl Into<Vec<u8>>) -> Self;

    /// Sets the MIME type for this part.
    pub fn mime_str(self, mime: impl Into<String>) -> Self;

    /// Sets the filename for this part.
    pub fn file_name(self, filename: impl Into<String>) -> Self;
}
```

### FormData Construction via ABI

The Form is transferred to JS via shared memory, then converted to FormData:

```rust
impl Form {
    pub(crate) fn into_form_data(&self) -> HttpResult<ExternalPointer> {
        // Encode form fields into shared memory:
        // [num_fields(u32)][field1_name(str)][field1_body(mem_id)][field1_mime(str)][field1_filename(str)]...

        let mem_id = self.write_form_to_memory()?;

        // Call JS-side FormData constructor
        let fd_uid = host_runtime::web::create_form_data_from_memory(mem_id)?;

        Ok(ExternalPointer::from_u64(fd_uid))
    }

    fn write_form_to_memory(&self) -> HttpResult<MemoryId> {
        // Serialize field data into shared memory
        // Each field: name (string) + body (MemoryId) + mime (string, optional) + filename (string, optional)
        let mut encoder = FormEncoder::new();
        encoder.write_u32(self.inner.fields.len() as u32);

        for (name, part) in &self.inner.fields {
            // Write body to memory, get MemoryId
            let body_mem_id = part.value.write_to_memory()?;

            encoder.write_str(name);
            encoder.write_u64(body_mem_id.0 as u64);
            encoder.write_str(part.meta.mime.as_deref().unwrap_or(""));
            encoder.write_str(part.meta.file_name.as_deref().unwrap_or(""));
        }

        encoder.finish()
    }
}
```

### Megatron.js FormData Construction

```javascript
/**
 * Create a FormData object from shared memory.
 * Memory layout: [num_fields(u32)][field1_name(str)][field1_body_mem_id(u64)]
 *                [field1_mime(str)][field1_filename(str)]...
 */
createFormDataFromMemory(memoryId) {
  const logger = LOGGER.scoped("createFormDataFromMemory:");
  const alloc = this.allocations.get(memoryId);
  const data = new DataView(alloc.buffer);
  let offset = 0;

  const formData = new FormData();
  const numFields = data.getUint32(offset);
  offset += 4;

  for (let i = 0; i < numFields; i++) {
    // Read field name
    const { value: name, offset: newOffset } = this.readString(alloc.buffer, offset);
    offset = newOffset;

    // Read body MemoryId
    const bodyMemId = data.getBigUint64(offset);
    offset += 8;
    const bodyAlloc = this.allocations.get(bodyMemId);
    const bodyBytes = new Uint8Array(bodyAlloc.buffer);

    // Read MIME type
    const { value: mime, offset: newOffset2 } = this.readString(alloc.buffer, offset);
    offset = newOffset2;

    // Read filename
    const { value: filename, offset: newOffset3 } = this.readString(alloc.buffer, offset);
    offset = newOffset3;

    // Append to FormData
    if (mime && filename) {
      // Create a Blob with the specified MIME type and filename
      const blob = new Blob([bodyBytes], { type: mime });
      const file = new File([blob], filename, { type: mime });
      formData.append(name, file);
    } else if (mime) {
      const blob = new Blob([bodyBytes], { type: mime });
      formData.append(name, blob);
    } else if (filename) {
      const blob = new Blob([bodyBytes]);
      const file = new File([blob], filename);
      formData.append(name, file);
    } else {
      // Simple text field
      const text = new TextDecoder().decode(bodyBytes);
      formData.append(name, text);
    }

    logger.debug("FormData field:", name, "mime:", mime, "filename:", filename);
  }

  // Store FormData as ExternalPointer
  const uid = this.storeExternalObject(formData);
  return uid;
}
```

### Using FormData as Request Body

When a Form is set as the request body, the bridge module passes the FormData ExternalPointer UID to the JS fetch executor instead of a body MemoryId:

```rust
// In bridge.rs, when request.body is MultipartForm:
Inner::MultipartForm(form) => {
    let fd_uid = form.into_form_data()?;
    // Pass fd_uid as body reference instead of body MemoryId
    encode_request_with_form_data(&request, fd_uid)
}
```

The JS fetch executor detects the FormData UID and uses it directly as `init.body`:

```javascript
// In fetchRequest:
if (params.isFormData) {
  const formData = this.getExternalObject(params.formDataUid);
  init.body = formData;
} else if (bodyData) {
  init.body = bodyData;
}
```

### Trade-offs and Decisions

| Decision | Rationale | Alternatives Considered |
|----------|-----------|------------------------|
| FormData constructed in JS | Can't create FormData through ABI directly | Encode as binary multipart — complex boundary management |
| Form serialized to memory, then JS reads | Consistent with other ABI transfers | Pass field-by-field via multiple host calls — more round trips |
| MIME as string, not mime crate | Avoids dependency | Use `mime` crate — adds 10+ transitive deps |
| FormData stored as ExternalPointer | Consistent with Response handling | Return directly — can't hold reference across calls |

## Implementation

### Files to Create/Modify

- `backends/foundation_wasm/src/http/multipart.rs` — New file: Form, Part, metadata
- `backends/foundation_wasm/src/http/body.rs` — Add `MultipartForm` variant to `Inner`
- `backends/foundation_wasm/sdk/jsruntime/megatron.js` — Add `createFormDataFromMemory()`

### Tasks

- [ ] T1: Create `PartMetadata` struct with mime and filename fields
- [ ] T2: Create `Part` struct with text/bytes constructors and builder methods
- [ ] T3: Create `FormParts<P>` generic collection
- [ ] T4: Create `Form` struct with text/part methods
- [ ] T5: Implement `Form::write_form_to_memory()` for ABI serialization
- [ ] T6: Implement `Form::into_form_data()` calling JS-side FormData constructor
- [ ] T7: Add `MultipartForm(Form)` variant to `Body::Inner`
- [ ] T8: Add `createFormDataFromMemory()` to megatron.js
- [ ] T9: Wire FormData as request body in bridge.rs
- [ ] T10: Export from `http/mod.rs` (feature-gated: `http-multipart`)

## Testing

### Test Cases

1. **Text-only form**: Form with text fields correctly converts to FormData
2. **Binary part**: Form with bytes part creates Blob with correct content
3. **MIME type**: Part with MIME type creates Blob with correct type
4. **Filename**: Part with filename creates File with correct name
5. **Mixed form**: Form with text, binary, and file fields all append correctly
6. **Request integration**: Multipart request sends correctly with FormData body

## Success Criteria

- [ ] All tasks completed
- [ ] Form correctly serializes to shared memory
- [ ] megatron.js correctly constructs FormData from memory
- [ ] FormData used as request body in fetch
- [ ] No regressions on native target
- [ ] Multipart body variant integrates with existing Body type

---

_Created: 2026-05-18_
