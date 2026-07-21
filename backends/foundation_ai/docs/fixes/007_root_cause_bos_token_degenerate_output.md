# 007 — Degenerate output ("." for every prompt) from BOS mishandling

## Symptom

With streaming repaired (docs/fixes/006), the local Gemma 4 E2B model answered
almost every prompt with a single junk token — most often `"."`, sometimes
`"</blockquote>"`, occasionally a correct word. `generate` reported 1–2 output
tokens: the model emitted end-of-turn almost immediately, as if the prompt
already looked complete.

## Investigation

Dumping the exact string fed to the tokenizer (with llama.cpp's own logs now
routed through tracing, spec-60/B) showed the rendered Gemma-4 chat template:

```
<|turn>system
<|think|>
You are a helpful assistant. ...
Available tools:
- shed()<turn|>
<|turn>user
Name one color.<turn|>
<|turn>model
```

Two things stood out:

1. **No `<bos>` in the rendered prompt.** Gemma-4's GGUF chat template starts
   straight at `<|turn>system` — it does not emit `{{ bos_token }}`.
2. Tokenization used `str_to_token(prompt, AddBos::Always)`, i.e. auto-prepend
   BOS, with `parse_special = true`.

An earlier hypothesis was a *doubled* BOS (template emits one, we add another).
That is real for templates that do emit `{{ bos_token }}` — but the opposite for
Gemma-4, whose template emits none. The first fix attempt (unconditional
`AddBos::Never` for templated prompts) therefore made Gemma-4 *worse*: it went
from one BOS (correct) to zero, changing the junk token from `"."` to
`"</blockquote>"` — proof that BOS handling, not doubling specifically, was the
axis that mattered.

## Root cause

BOS handling was unconditional, and no single policy is correct across models:

| Template | `AddBos::Always` | `AddBos::Never` |
|----------|------------------|-----------------|
| emits `{{ bos_token }}` (many) | **doubled BOS** | correct |
| omits it (Gemma-4) | correct | **no BOS** |

Either extreme corrupts the prompt for half of all models, and a corrupt BOS is
a known cause of degenerate first tokens.

## Fix

`tokenize_prompt(model, prompt, templated)` ensures **exactly one** leading BOS:

- Raw prompt (no chat template): `AddBos::Always`, as before.
- Templated prompt: tokenize with `AddBos::Never` and `parse_special = true`
  (so any `<bos>` already in the string becomes the BOS token, not literal
  text), then prepend `model.token_bos()` only if the first token is not
  already BOS.

Applied at all three tokenization sites — the streaming path, `generate_text`,
and threaded through `Model::generate`. The embedding path is unchanged (raw,
no template).

## Result

The model answers coherently:

```
Name one color.            -> Blue
What is the capital of ... -> The capital of France is Paris.
```

## Left for follow-up (separate defects, not this bug)

The France answer exposed two more, both visible in the prompt dump above:

- **Phantom `shed()` tool.** `ToolShed::default()` sets `shed: Some(..)`, so
  every prompt — even from an app with no tools — injects a `shed()` meta-tool
  and tool-calling instructions. The model visibly wasted reasoning on it
  ("the shed() tool is presumably for shedding something ... irrelevant").
- **Thinking channel leaks.** Gemma-4 emits `<|channel>thought ... <channel|>`
  reasoning that is passed through raw instead of being parsed out.

Neither causes the degenerate-output bug; both are quality issues to address on
their own.
