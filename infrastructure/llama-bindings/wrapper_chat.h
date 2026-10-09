// C shim over llama.cpp's C++ chat-template API (common/chat.h).
//
// WHY: the legacy `llama_chat_apply_template` C API only handles a hardcoded
// set of templates and returns < 0 for the Jinja templates shipped by modern
// models (Gemma 4, Qwen3-Next, GLM, DeepSeek, ...). The capable path is
// `common_chat_templates_*` in common/chat.h, but that is a C++ API
// (std::string / std::unique_ptr) that bindgen cannot bind directly. This
// header exposes a minimal `extern "C"` surface that bindgen can consume; the
// implementation lives in wrapper_chat.cpp.

#pragma once

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

struct llama_model;

// Opaque owner of a `common_chat_templates` instance.
typedef struct ewe_chat_templates ewe_chat_templates;

// Build chat templates from a model. The model's embedded Jinja chat template
// (and bos/eos tokens) are used. `tmpl_override` may be NULL or "" to use the
// model default, or a template string/name to override it.
//
// Returns NULL on failure (e.g. the template cannot be parsed).
ewe_chat_templates * ewe_chat_templates_init(const struct llama_model * model,
                                             const char *               tmpl_override);

// Free a handle from `ewe_chat_templates_init`. NULL-safe.
void ewe_chat_templates_free(ewe_chat_templates * tmpls);

// Render a chat into a prompt. `roles` and `contents` are parallel arrays of
// `n_messages` NUL-terminated UTF-8 strings. `add_generation_prompt` appends
// the assistant generation prefix.
//
// Returns a newly heap-allocated NUL-terminated string that the caller must
// release with `ewe_chat_string_free`, or NULL on error.
char * ewe_chat_templates_apply(const ewe_chat_templates * tmpls,
                                const char * const *       roles,
                                const char * const *       contents,
                                size_t                     n_messages,
                                bool                       add_generation_prompt);

// Free a string returned by `ewe_chat_templates_apply`. NULL-safe.
void ewe_chat_string_free(char * s);

#ifdef __cplusplus
}
#endif
