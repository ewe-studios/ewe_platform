// Implementation of the C chat-template shim declared in wrapper_chat.h.
//
// Wraps llama.cpp's C++ `common_chat_templates_*` (minja-based Jinja) API so
// Rust can render the Jinja chat templates that modern GGUF models ship.
// All C++ exceptions are caught at the boundary and reported as NULL.

#include "chat.h"  // llama.cpp: common/chat.h

#include "wrapper_chat.h"

#include <cstdlib>
#include <cstring>
#include <string>
#include <utility>

struct ewe_chat_templates {
    common_chat_templates_ptr ptr;
};

extern "C" ewe_chat_templates * ewe_chat_templates_init(const llama_model * model,
                                                        const char *        tmpl_override) {
    try {
        std::string override = tmpl_override != nullptr ? std::string(tmpl_override) : std::string();
        common_chat_templates_ptr tmpls = common_chat_templates_init(model, override);
        if (!tmpls) {
            return nullptr;
        }
        auto * handle = new ewe_chat_templates();
        handle->ptr    = std::move(tmpls);
        return handle;
    } catch (...) {
        return nullptr;
    }
}

extern "C" void ewe_chat_templates_free(ewe_chat_templates * tmpls) {
    delete tmpls;
}

extern "C" char * ewe_chat_templates_apply(const ewe_chat_templates * tmpls,
                                           const char * const *       roles,
                                           const char * const *       contents,
                                           size_t                     n_messages,
                                           bool                       add_generation_prompt) {
    if (tmpls == nullptr || !tmpls->ptr) {
        return nullptr;
    }
    try {
        common_chat_templates_inputs inputs;
        inputs.use_jinja             = true;
        inputs.add_generation_prompt = add_generation_prompt;
        inputs.messages.reserve(n_messages);
        for (size_t i = 0; i < n_messages; ++i) {
            common_chat_msg msg;
            msg.role    = (roles != nullptr && roles[i] != nullptr) ? roles[i] : "";
            msg.content = (contents != nullptr && contents[i] != nullptr) ? contents[i] : "";
            inputs.messages.push_back(std::move(msg));
        }

        common_chat_params  params = common_chat_templates_apply(tmpls->ptr.get(), inputs);
        const std::string & prompt = params.prompt;

        char * out = static_cast<char *>(std::malloc(prompt.size() + 1));
        if (out == nullptr) {
            return nullptr;
        }
        std::memcpy(out, prompt.c_str(), prompt.size() + 1);
        return out;
    } catch (...) {
        return nullptr;
    }
}

extern "C" void ewe_chat_string_free(char * s) {
    std::free(s);
}
