# web_gen — TypeScript-to-Rust Binding Generation

> **Source**: `@formulas/src.rust/src.wasm/src.webrs/web.rs/crates/web_gen/`
> **What it does**: Parses a custom TypeScript-like definition language into an AST, then generates Rust FFI stub files — one `.rs` file per namespace.
> **What it does NOT do**: It does NOT read standard `.d.ts` files. It does NOT generate TypeScript from Rust. It does NOT produce working implementations — only type signatures.

---

## Table of Contents

1. [End-to-End Flow](#1-end-to-end-flow)
2. [The Custom Definition Language (DSL)](#2-the-custom-definition-language-dsl)
3. [The Parser — nom-Based Recursive Descent](#3-the-parser--nom-based-recursive-descent)
4. [The AST — Data Model](#4-the-ast--data-model)
5. [The Code Generator — AST → Rust Source](#5-the-code-generator--ast--rust-source)
6. [Type Mapping Table](#6-type-mapping-table)
7. [Example: Input → Output Walkthrough](#7-example-input--output-walkthrough)
8. [What the Generated Code Actually Looks Like](#8-what-the-generated-code-actually-looks-like)
9. [The CLI Tool](#9-the-cli-tool)
10. [Limitations and Gaps](#10-limitations-and-gaps)
11. [Lessons for Our Project](#11-lessons-for-our-project)

---

## 1. End-to-End Flow

```
┌──────────────────────────────┐
│  Custom .d.ts-like file      │
│  (hand-written DSL)          │
│                              │
│  declare namespace WebGPU {  │
│    interface GPUAdapter {    │
│      requestDevice(): ...    │
│    }                         │
│    var navigator: Navigator  │
│  }                           │
└──────────────┬───────────────┘
               │
               │  CLI: web_gen input.dsl --output-dir ./out/
               ▼
┌──────────────────────────────┐
│  PARSE (typescript.rs)       │
│  nom-based recursive parser  │
│                              │
│  bytes → str → parse_file()  │
│         → TypescriptDefinitionFile (AST)
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  GENERATE (generation.rs)    │
│                              │
│  1. Collect all interfaces    │
│     into a global map         │
│  2. For each namespace:       │
│     a. Create {name}.rs file  │
│     b. Write use js::*;       │
│     c. For each declaration:  │
│        → emit getter fns      │
│        → emit method stubs    │
└──────────────┬───────────────┘
               │
               ▼
┌──────────────────────────────┐
│  Output: {namespace}.rs      │
│                              │
│  use js::*;                   │
│  pub fn GPUAdapter_request...│
│  pub fn field_get(...) -> .. │
│  (all bodies = unimplemented)│
└──────────────────────────────┘
```

---

## 2. The Custom Definition Language (DSL)

This is **not** standard TypeScript. It's a simplified, purpose-built DSL that looks like TypeScript declarations but has its own grammar rules.

### 2.1 Grammar (EBNF-like)

```
file         → { part }
part         → namespace | interface | declaration | comment

namespace    → "declare" "namespace" IDENTIFIER "{" { part } "}"
interface    → "interface" IDENTIFIER "{" { interface_member } "}"
declaration  → "var" IDENTIFIER ":" type_name ";"
comment      → "//" { any_char_except_newline }

interface_member → function | field
function     → IDENTIFIER "(" [ parameters ] ")" ":" type_name ";"
field        → IDENTIFIER ":" type_name ";"
parameters   → parameter { "," parameter }
parameter    → IDENTIFIER ":" type_name

type_name    → "void" | "string" | "number" | "boolean" | IDENTIFIER
IDENTIFIER   → alpha1  (one or more alphabetic chars, NO digits/underscores/hyphens)
```

### 2.2 Concrete Example

```typescript
// WebGPU API definitions
declare namespace WebGPU {
    interface GPUAdapter {
        requestDevice(): GPUDevice;
    }

    interface GPUDevice {
        createBuffer(size: number, usage: number, mappedAtCreation: boolean): GPUBuffer;
        createShaderModule(source: string): GPUShaderModule;
        getQueue(): GPUQueue;
    }

    interface GPUBuffer {
        setFromF32Array(data: number): void;
        unmapped(): void;
    }

    interface GPUQueue {
        submit(commandBuffers: GPUCommandBuffer): void;
    }

    var adapter: GPUAdapter;
    var device: GPUDevice;
}
```

### 2.3 Key DSL Constraints

| Constraint | Detail |
|-----------|--------|
| **Identifiers** | Only `[a-zA-Z]+` — no digits, no underscores, no hyphens. Parsed via `nom::alpha1`. |
| **Types** | Exactly 5: `void`, `string`, `number`, `boolean`, or "any other identifier" = interface reference |
| **No arrays** | Array type parsing is commented out in both parser and generator |
| **No generics** | No `<T>` syntax, no type parameters |
| **No optionals** | No `?` suffix, no `| null`, no `| undefined` unions |
| **No nested namespaces** | The parser supports `declare namespace`, but the generator only iterates one level |
| **Comments ignored** | `//` comments are parsed into the AST but discarded during generation |
| **Semicolons required** | Every declaration, field, and function must end with `;` |

---

## 3. The Parser — nom-Based Recursive Descent

**File**: `crates/web_gen/src/typescript.rs`
**Parser combinators used**: `tag`, `alpha1`, `char`, `multispace0`, `multispace1`, `opt`, `many0`, `separated_list0`, `alt`, `map`, `take_while`

### 3.1 Parser Hierarchy

```
parse_file(buffer: &[u8])
  └── parse_file(str)                    ← wraps bytes as str
        └── parse_part()* (many0)         ← top-level parts
              ├── parse_interface
              ├── parse_comment
              ├── parse_declaration
              └── parse_namespace
                    └── parse_part()*     ← recursive: parts inside namespace
```

### 3.2 Each Parser Combinator

#### `parse_file` — Entry Point
```rust
fn parse_file(input: &str) -> IResult<&str, TypescriptDefinitionFile> {
    map(opt(many0(parse_part)), |nodes| TypescriptDefinitionFile {
        parts: nodes.unwrap_or(vec![])
    })(input)
}
```
Accepts zero or more `parse_part` results, wraps in the file struct. The `opt` makes empty files valid.

#### `parse_part` — Dispatch
```rust
fn parse_part(input: &str) -> IResult<&str, TypescriptDefinitionFilePart> {
    alt((parse_interface, parse_comment, parse_declaration, parse_namespace))(input)
}
```
Tries each parser in order. `alt` returns the first match. **Order matters**: `parse_interface` is tried before `parse_declaration` because `interface` starts with alphabetic chars that could match a variable name.

#### `parse_namespace` — Namespace Block
```rust
fn parse_namespace(input: &str) -> IResult<&str, TypescriptDefinitionFilePart> {
    let (input, _) = multispace0(input)?;
    let (input, _) = tag("declare")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, _) = tag("namespace")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('{')(input)?;
    let (input, parts) = map(opt(many0(parse_part)), |nodes| nodes.unwrap_or(vec![]))(input)?;
    let (input, _) = multispace0(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, TypescriptDefinitionFilePart::NameSpace(Namespace {
        name: String::from(name),
        parts: parts,
    })))
}
```

Key detail: **recursive parsing** — `parse_namespace` calls `many0(parse_part)`, which can match another namespace, creating arbitrary nesting in the AST. But the generator only handles one level.

#### `parse_interface` — Interface Block
```rust
fn parse_interface(input: &str) -> IResult<&str, TypescriptDefinitionFilePart> {
    let (input, _) = tag("interface")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = char('{')(input)?;
    let (input, members) = map(opt(many0(parse_interface_member)), |nodes| nodes.unwrap_or(vec![]))(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, TypescriptDefinitionFilePart::Interface(Interface {
        name: String::from(name),
        members,
    })))
}
```

#### `parse_interface_member` — Function or Field
```rust
fn parse_interface_member(input: &str) -> IResult<&str, InterfaceMember> {
    alt((parse_function, parse_field))(input)
}
```
Tries function first because both start with an identifier. If the identifier is followed by `(`, it's a function; if by `:`, it's a field.

#### `parse_function` — Method Signature
```rust
fn parse_function(input: &str) -> IResult<&str, InterfaceMember> {
    let (input, name) = alpha1(input)?;
    let (input, _) = char('(')(input)?;
    let (input, parameters) = map(opt(parse_parameters), |args| args.unwrap_or(vec![]))(input)?;
    let (input, _) = char(')')(input)?;
    let (input, _) = char(':')(input)?;
    let (input, return_type) = alpha1(input)?;
    let (input, _) = char(';')(input)?;
    Ok((input, InterfaceMember::Function(Function {
        name: String::from(name),
        parameters,
        return_type: match return_type {
            "void" => ValueType::Void,
            "string" => ValueType::String,
            "number" => ValueType::Number,
            "boolean" => ValueType::Boolean,
            _ => ValueType::Interface(String::from(return_type)),
        },
    })))
}
```

#### `parse_parameters` — Comma-Separated List
```rust
fn parse_parameters(input: &str) -> IResult<&str, Vec<Parameter>> {
    separated_list0(char(','), parse_parameter)(input)
}
```

#### `parse_parameter` — Single Parameter
```rust
fn parse_parameter(input: &str) -> IResult<&str, Parameter> {
    let (input, name) = alpha1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, value_type) = alpha1(input)?;
    Ok((input, Parameter {
        name: String::from(name),
        value_type: match value_type {
            "void" => panic!("void is not a valid parameter type"),
            "string" => ValueType::String,
            "number" => ValueType::Number,
            "boolean" => ValueType::Boolean,
            _ => ValueType::Interface(String::from(value_type)),
        },
    }))
}
```
Note: `void` as a parameter type causes a **panic** at parse time.

#### `parse_field` — Property Declaration
```rust
fn parse_field(input: &str) -> IResult<&str, InterfaceMember> {
    let (input, name) = alpha1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, return_type) = alpha1(input)?;
    let (input, _) = char(';')(input)?;
    Ok((input, InterfaceMember::Field(Declaration {
        name: String::from(name),
        value_type: match return_type {
            "void" => panic!("Field cannot have void type"),
            "string" => ValueType::String,
            "number" => ValueType::Number,
            "boolean" => ValueType::Boolean,
            _ => ValueType::Interface(String::from(return_type)),
        },
    })))
}
```

#### `parse_declaration` — Global Variable
```rust
fn parse_declaration(input: &str) -> IResult<&str, TypescriptDefinitionFilePart> {
    let (input, _) = tag("var")(input)?;
    let (input, _) = multispace1(input)?;
    let (input, name) = alpha1(input)?;
    let (input, _) = char(':')(input)?;
    let (input, _) = multispace1(input)?;
    let (input, value_type) = alpha1(input)?;
    let (input, _) = char(';')(input)?;
    Ok((input, TypescriptDefinitionFilePart::Declaration(Declaration { ... })))
}
```

#### `parse_comment` — Line Comment
```rust
fn parse_comment(input: &str) -> IResult<&str, TypescriptDefinitionFilePart> {
    let (input, _) = char('/')(input)?;
    let (input, _) = char('/')(input)?;
    let (input, comment) = take_while(|c: char| c != '\n' && c != '\r')(input)?;
    Ok((input, TypescriptDefinitionFilePart::Comment(Comment(comment.to_string()))))
}
```

#### `parse` — Public Entry
```rust
pub fn parse(buffer: &[u8]) -> Result<TypescriptDefinitionFile, &'static str> {
    let buffer_str = std::str::from_utf8(buffer).map_err(|_| "Invalid UTF-8")?;
    let r = parse_file(buffer_str);
    match r {
        Ok((_, usd)) => Ok(usd),
        Err(e) => { println!("{:?}", e); Err("Parse error") }
    }
}
```

### 3.3 Whitespace Handling

`multispace0` and `multispace1` are used liberally between every token. This means the DSL is **whitespace-insensitive** — you can put extra spaces anywhere and it still parses. However, there's no support for inline comments or block comments (`/* */`).

---

## 4. The AST — Data Model

```rust
pub enum ValueType {
    Void,
    Interface(String),   // any identifier that isn't void/string/number/boolean
    String,
    Number,
    Boolean,
}

pub struct Parameter {
    pub name: String,
    pub value_type: ValueType,
}

pub struct Function {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub return_type: ValueType,
}

pub struct Declaration {
    pub name: String,
    pub value_type: ValueType,
}

pub enum InterfaceMember {
    Function(Function),
    Field(Declaration),
}

pub struct Interface {
    pub name: String,
    pub members: Vec<InterfaceMember>,
}

pub struct Namespace {
    pub name: String,
    pub parts: Vec<TypescriptDefinitionFilePart>,
}

pub enum TypescriptDefinitionFilePart {
    Comment(Comment),
    Interface(Interface),
    Declaration(Declaration),
    NameSpace(Namespace),
}

pub struct TypescriptDefinitionFile {
    pub parts: Vec<TypescriptDefinitionFilePart>,
}
```

**Key observation**: The AST is flat. There's no type hierarchy, no inheritance, no method overloading, no visibility modifiers. Everything is `pub` by default. This matches the FFI nature of the generated code — these are extern signatures, not a type system.

---

## 5. The Code Generator — AST → Rust Source

**File**: `crates/web_gen/src/generation.rs`

### 5.1 Two-Pass Generation

**Pass 1 — Collect all interfaces into a global lookup map**:

```rust
let mut all_known_interfaces = HashMap::new();
for parts in definition_file.parts.iter() {
    match parts {
        TypescriptDefinitionFilePart::Interface(interface) => {
            all_known_interfaces.insert(interface.name.clone(), interface.clone());
        }
        TypescriptDefinitionFilePart::NameSpace(namespace) => {
            for parts in namespace.parts.iter() {
                if let TypescriptDefinitionFilePart::Interface(interface) = parts {
                    all_known_interfaces.insert(interface.name.clone(), interface.clone());
                }
            }
        }
        _ => {}
    }
}
```

This is needed because when generating code for a declaration like `var adapter: GPUAdapter`, the generator needs to look up the `GPUAdapter` interface to emit its members. The map is keyed by interface name.

**Pass 2 — Generate one .rs file per namespace**:

```rust
for parts in definition_file.parts.iter() {
    if let TypescriptDefinitionFilePart::NameSpace(namespace) = parts {
        let file_name = output_base_dir.join(format!("{}.rs", pascal_case_to_snake_case(&namespace.name)));
        let mut file = std::fs::File::create(file_name).unwrap();
        let mut writer = std::io::BufWriter::new(&mut file);
        writer.write_all(b"use js::*;\r\n").unwrap();
        // ... emit declarations ...
        writer.flush().unwrap();
    }
}
```

### 5.2 Declaration Code Generation

For each `Declaration` inside a namespace whose type is an interface:

```rust
TypescriptDefinitionFilePart::Declaration(declaration) => {
    let declaration_name = &declaration.name;  // e.g., "adapter"
    if let ValueType::Interface(interface_name) = &declaration.value_type {
        let interface = all_known_interfaces.get(&interface_name).unwrap();
        for interface_member in interface.members.iter() {
            match interface_member {
                InterfaceMember::Field(field) => { ... }
                InterfaceMember::Function(function) => { ... }
            }
        }
    }
}
```

The generator iterates the members of the referenced interface and emits a function for each.

### 5.3 Field → Getter Function

```rust
InterfaceMember::Field(field) => {
    // Write: pub fn field_name(this: &InterfaceName) -> Type {
    writer.write_all(b"pub fn ").unwrap();
    writer.write_all(field.name.as_bytes()).unwrap();
    writer.write_all(b"(").unwrap();
    writer.write_all(b"this: &").unwrap();
    writer.write_all(interface.name.as_bytes()).unwrap();
    writer.write_all(b") -> ").unwrap();

    // Write return type based on field type
    match &field.value_type {
        ValueType::Boolean => writer.write_all(b"bool"),
        ValueType::Number => writer.write_all(b"f64"),
        ValueType::String => writer.write_all(b"&str"),
        ValueType::Interface(name) => {
            writer.write_all(b"&");
            writer.write_all(name.as_bytes());
        }
        _ => {}
    }

    // Write body: js_unwrap!(this.field_name)
    writer.write_all(b" {\r\n    js_unwrap!(this.");
    writer.write_all(field.name.as_bytes());
    writer.write_all(b")\r\n}\r\n");
}
```

**Generated output for a field**:
```rust
pub fn width(this: &GPUTexture) -> f64 {
    js_unwrap!(this.width)
}
```

Note: `js_unwrap!` is NOT defined anywhere in the codebase. It appears to be a placeholder for a future macro. This code would not compile as-is.

### 5.4 Function → Method Stub

```rust
InterfaceMember::Function(function) => {
    // Write: pub fn declarationName_functionName(params...) -> ReturnType {
    writer.write_all(b"pub fn ").unwrap();
    writer.write_all(declaration_name.as_bytes()).unwrap();  // e.g., "adapter"
    writer.write_all(b"_").unwrap();
    writer.write_all(function.name.as_bytes()).unwrap();     // e.g., "requestDevice"
    writer.write_all(b"(").unwrap();

    // Write parameters
    for (index, parameter) in function.parameters.iter().enumerate() {
        if index > 0 { writer.write_all(b", "); }
        writer.write_all(parameter.name.as_bytes()).unwrap();
        writer.write_all(b": ").unwrap();
        match &parameter.value_type {
            ValueType::Boolean => writer.write_all(b"bool"),
            ValueType::Number => writer.write_all(b"f64"),
            ValueType::String => writer.write_all(b"&str"),
            ValueType::Interface(name) => {
                writer.write_all(b"&");
                writer.write_all(name.as_bytes());
            }
            _ => {}
        }
    }

    // Write return type
    writer.write_all(b") -> ").unwrap();
    match &function.return_type {
        ValueType::Boolean => writer.write_all(b"bool"),
        ValueType::Number => writer.write_all(b"f64"),
        ValueType::String => writer.write_all(b"String"),    // Note: String for returns, &str for params
        ValueType::Interface(name) => { ... }
        _ => writer.write_all(b"()"),                        // void → ()
    }

    // Write body: unimplemented!()
    writer.write_all(b" {\r\n    unimplemented!()\r\n}\r\n");
}
```

**Generated output for a function**:
```rust
pub fn adapter_requestDevice() -> &GPUDevice {
    unimplemented!()
}
```

### 5.5 Name Conversion

```rust
fn pascal_case_to_snake_case(input: &str) -> String {
    let mut result = String::new();
    for (index, c) in input.chars().enumerate() {
        if c.is_uppercase() {
            if index > 0 { result.push('_'); }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}
```

| Input | Output |
|-------|--------|
| `WebGPU` | `web_g_p_u` |
| `MyNamespace` | `my_namespace` |
| `XMLParser` | `x_m_l_parser` |

This is a naive converter — it treats each uppercase letter as a word boundary. `WebGPU` becomes `web_g_p_u` rather than `web_gpu`.

### 5.6 File Output

Each namespace gets its own file. The file starts with `use js::*;` and uses `\r\n` line endings (Windows-style, hardcoded). The `BufWriter` is flushed at the end but the `File` is never explicitly closed (dropped on function return, which flushes).

---

## 6. Type Mapping Table

| DSL Type | As Parameter | As Return Type | As Field Type |
|----------|-------------|----------------|---------------|
| `void` | ❌ panic | `()` (unit) | ❌ panic |
| `string` | `&str` | `String` | `&str` |
| `number` | `f64` | `f64` | `f64` |
| `boolean` | `bool` | `bool` | `bool` |
| `SomeInterface` | `&SomeInterface` | `&SomeInterface` | `&SomeInterface` |
| Array types | ❌ commented out | ❌ commented out | ❌ commented out |

**Notable asymmetry**: `string` maps to `&str` for parameters/fields but `String` for return types. This is correct Rust idiom (borrowed input, owned output) but it's the only type with this asymmetry in the generator.

---

## 7. Example: Input → Output Walkthrough

### 7.1 Input DSL File

```
declare namespace WebGPU {
    interface GPUAdapter {
        requestDevice(): GPUDevice;
    }

    interface GPUDevice {
        createBuffer(size: number, usage: number): GPUBuffer;
        queue: GPUQueue;
    }

    interface GPUBuffer {
        unmapped(): void;
    }

    var adapter: GPUAdapter;
    var device: GPUDevice;
}
```

### 7.2 Parser Output (AST)

```rust
TypescriptDefinitionFile {
    parts: [
        NameSpace(Namespace {
            name: "WebGPU",
            parts: [
                Interface(Interface {
                    name: "GPUAdapter",
                    members: [
                        Function(Function {
                            name: "requestDevice",
                            parameters: [],
                            return_type: Interface("GPUDevice"),
                        }),
                    ],
                }),
                Interface(Interface {
                    name: "GPUDevice",
                    members: [
                        Function(Function {
                            name: "createBuffer",
                            parameters: [
                                Parameter { name: "size", value_type: Number },
                                Parameter { name: "usage", value_type: Number },
                            ],
                            return_type: Interface("GPUBuffer"),
                        }),
                        Field(Declaration {
                            name: "queue",
                            value_type: Interface("GPUQueue"),
                        }),
                    ],
                }),
                Interface(Interface {
                    name: "GPUBuffer",
                    members: [
                        Function(Function {
                            name: "unmapped",
                            parameters: [],
                            return_type: Void,
                        }),
                    ],
                }),
                Declaration(Declaration { name: "adapter", value_type: Interface("GPUAdapter") }),
                Declaration(Declaration { name: "device", value_type: Interface("GPUDevice") }),
            ],
        }),
    ],
}
```

### 7.3 Generator Output (`web_g_p_u.rs`)

```rust
use js::*;

pub fn adapter_requestDevice() -> &GPUDevice {
    unimplemented!()
}

pub fn device_createBuffer(size: f64, usage: f64) -> &GPUBuffer {
    unimplemented!()
}

pub fn queue(this: &GPUDevice) -> &GPUQueue {
    js_unwrap!(this.queue)
}

pub fn device_unmapped() -> () {
    unimplemented!()
}
```

**What's missing from the output**: The `GPUAdapter`, `GPUDevice`, `GPUBuffer`, `GPUQueue` types themselves are not defined. They're referenced but never declared. The generated file would not compile without the types being defined elsewhere (presumably by hand).

---

## 8. What the Generated Code Actually Looks Like

The generated code has three fundamental problems:

### 8.1 `js_unwrap!` is Undefined

Field getters emit `js_unwrap!(this.field_name)` but this macro does not exist in the codebase. There's `js!` in the `js` crate, but no `js_unwrap!`. This means field generation produces non-compiling code.

### 8.2 Types Are Never Defined

The generator emits `&GPUDevice`, `&GPUBuffer`, etc. as parameter and return types, but never emits `struct GPUDevice(ExternRef);` or any definition. These types must be hand-written elsewhere.

### 8.3 Bodies Are `unimplemented!()`

Function stubs have `unimplemented!()` bodies. They compile (as long as the types exist) but do nothing at runtime. The developer must manually replace each `unimplemented!()` with actual `js!(...).invoke_and_return_*()` calls.

**In practice**: The generator produces a file with the correct function signatures but zero working code. Its value is as a **header file** — it tells you what functions exist and what their types are, so you don't have to type the signatures by hand.

---

## 9. The CLI Tool

**File**: `crates/web_gen/src/main.rs`
**Binary name**: `web_gen`
**Args**: `web_gen <input_file> [--output-dir <dir>]`

```rust
fn main() {
    let args = Args::parse();
    let mut file = File::open(args.input_file).unwrap();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();

    let output_dir: PathBuf = args.output_dir.unwrap_or(".".into()).into();

    let result = parse(&buffer);
    if let Ok(definition_file) = result {
        generate_bindings(&definition_file, output_dir);
        println!("Done!");
    } else {
        println!("Error generating bindings: {:?}", result);
    }
}
```

No error recovery, no line/column reporting on parse errors (the nom error is printed with `{:?}` which shows the remaining input but not a useful position).

---

## 10. Limitations and Gaps

### 10.1 Parser Limitations

| Limitation | Impact |
|-----------|--------|
| `alpha1` identifiers only | No `getBoundingClientRect`, no `data-*`, no `aria-*`, no `_private`, no `xmlHttp` (digits) |
| No array types | Cannot express `HTMLElement[]`, `Uint8Array`, etc. — commented out in both parser and generator |
| No union types | Cannot express `string \| null`, `number \| undefined` |
| No optional parameters | Cannot express `(x: number, y?: number)` |
| No generics | Cannot express `Promise<T>`, `Map<K, V>` |
| No block comments | Only `//` line comments, no `/* */` |
| No error positioning | Parse errors print remaining input, not line/column |
| Panic on `void` params | `void` in parameter position crashes the parser, returns a user-friendly error |

### 10.2 Generator Limitations

| Limitation | Impact |
|-----------|--------|
| `js_unwrap!` undefined | Field getters produce non-compiling code |
| Types not generated | Interface types must be hand-defined elsewhere |
| `unimplemented!()` bodies | Zero working code produced |
| `\r\n` hardcoded | Windows line endings even on Linux/macOS |
| `WebGPU` → `web_g_p_u` | Naive PascalCase converter produces ugly filenames |
| Only namespaces generate files | Top-level interfaces (outside namespace) are collected to the map but never generate files |
| No deduplication | If two namespaces declare the same interface, the HashMap silently overwrites |
| `.unwrap()` everywhere | Any missing interface reference in the DSL causes a panic |
| No array support in output | Commented-out array handling in both field and function generation |

### 10.3 Design Limitations

| Limitation | Impact |
|-----------|--------|
| Custom DSL, not `.d.ts` | Can't consume existing TypeScript definitions from DefinitelyTyped or web libraries |
| One-way only | Generates Rust from DSL, not vice versa |
| No wasm-bindgen output | Produces raw Rust stubs, not `#[wasm_bindgen]` annotated code |
| Tied to `js` crate conventions | Generated code assumes `js::*` is in scope and uses `ExternRef`-based types |
| No versioning | No way to track which version of an API the DSL represents |

---

## 11. Lessons for Our Project

### 11.1 What's Worth Borrowing

1. **The two-pass architecture** — Collecting all types into a global lookup before generation is the right pattern. You need the full type context to resolve interface references.

2. **The nom parser approach** — Hand-written recursive descent with nom combinators is lightweight and has zero dependencies beyond `nom`. For a custom DSL, this is better than bringing in a full parser generator.

3. **The type-tagged wire protocol** — While not part of the codegen per se, the binary protocol that the generated code would use (if implemented) is worth understanding for any WASM FFI work.

4. **Interface-as-declaration pattern** — The DSL's approach of declaring `var name: InterfaceType` and then expanding its members into functions is a form of "facade generation." This is a valid pattern for generating typed wrappers.

### 11.2 What to Do Differently

1. **Parse real `.d.ts` files** — Instead of a custom DSL, use an existing TypeScript definition parser. There are crates like `typescript-parser` or you can use the TypeScript compiler API via Node. The value of codegen comes from consuming existing definitions, not writing your own.

2. **Generate `#[wasm_bindgen]` code** — If targeting WASM, generate code annotated with `#[wasm_bindgen]` rather than raw `js!` macro calls. This gives you error handling, type safety, and integration with the `web-sys` ecosystem.

3. **Generate implementations, not stubs** — The biggest gap is that `unimplemented!()` bodies provide zero value. A better generator would produce actual FFI calls with the correct `js!` or `wasm_bindgen` invocation patterns.

4. **Proper name conversion** — Use a real PascalCase→snake_case converter (e.g., `convert_case` crate) that handles acronyms correctly: `WebGPU` → `web_gpu`, not `web_g_p_u`.

5. **Generate type definitions too** — Emit `struct TypeName(ExternRef);` for every interface, not just method signatures. The generated file should be self-contained and compilable.

6. **Error handling in the generator** — Replace `.unwrap()` with proper `Result` propagation. Provide line/column error messages from the parser.

7. **Support arrays and optionals** — These are ubiquitous in real Web APIs. The generator must handle `[T]`, `Option<T>`, and `Vec<T>` types.

8. **Output format** — Use `syn` and `quote` to generate Rust code programmatically rather than raw byte writing. This ensures correct formatting, proper handling of edge cases, and the ability to round-trip the AST.

### 11.3 The Bigger Picture

The `web_gen` approach represents a **minimal viable codegen strategy**: define your API surface in a simple language, generate stubs, fill in by hand. It's honest about its limitations — the generated code is clearly meant to be a starting point, not a finished product.

For our `foundation-wasm-ui` project, if we need TypeScript ↔ Rust binding generation, we should either:

- **Use `wasm-bindgen` + `web-sys`** — The ecosystem standard. Handles 99% of Web APIs already.
- **Build on `ts-rs` or `schemars`** — For generating TypeScript types FROM Rust (the reverse direction, which is more commonly needed for API contracts).
- **Custom DSL only if** — We have a truly novel abstraction that existing tools can't express. The `web_gen` DSL doesn't offer enough over standard `.d.ts` to justify the custom parser.
