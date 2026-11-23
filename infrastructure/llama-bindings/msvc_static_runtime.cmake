# Force the static MSVC runtime for llama.cpp.
#
# WHY: the Rust static library and the `esaxx-rs` objects use the static C
# runtime (`/MT`). llama.cpp's objects must match, or the link fails with
# LNK2038.
#
# HOW: this file runs through `CMAKE_PROJECT_TOP_LEVEL_INCLUDES` after
# `project()`, before any target exists. llama.cpp declares
# `cmake_minimum_required` below 3.15, so policy CMP0091 is OLD there; setting
# the policy and the runtime variable here makes them take effect.
if(MSVC)
  cmake_policy(SET CMP0091 NEW)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()
