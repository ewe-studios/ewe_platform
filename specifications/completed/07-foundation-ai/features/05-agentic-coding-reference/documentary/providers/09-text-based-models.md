# Text-Based Models -- Deep Dive

## Reference

This section covers open-source models that do NOT have structured tool calling APIs. Tool calls are embedded in text output and must be parsed.

## Model Families and Formats

### Hermes / Qwen / Longcat Format

- **Pattern**: Tool name followed by JSON object in backticks
- **Format**: `` `tool_name { "arg": "value" }` ``
- **Parse**: Extract JSON between backticks, parse, validate against schema

### Llama 3/4 Format

- **Pattern**: Raw JSON objects with `name` and `args` keys
- **Format**: `{ "name": "tool_name", "args": { "arg": "value" } }`
- **Parse**: Detect JSON objects in text, look for `name` + `args` structure
- **Challenge**: No wrapping markers -- must detect JSON objects within text

### Mistral Open-Source Format

- **Pattern**: Sentinel token followed by tool name and JSON object
- **Format**: `[TOOL] tool_name { json }`
- **Parse**: Look for `[TOOL]` sentinel, extract tool name and JSON

### DeepSeek V3/V3.1 Format

- **Pattern**: Special unicode tokens with markdown-wrapped JSON
- **Parse**: Detect unicode sentinels, extract markdown, parse JSON

### Kimi K2 Format

- **Pattern**: Section begin/end tokens wrapping individual tool call blocks
- **Parse**: Look for section markers, extract tool call blocks within

### GLM 4.5/4.7 Format

- **Pattern**: Backtick-arg_key/arg_value pairs instead of JSON
- **Format**: `` `arg_key` key_name `arg_value` value ``
- **Parse**: Extract key-value pairs, assemble into JSON object
- **Challenge**: Not JSON -- requires key-value assembly

### Qwen3-Coder Format

- **Pattern**: XML-style nested function/parameter tags
- **Parse**: `<function name="tool_name">