# Native todo registry

This crate builds the example's Rust action registry. Its `create_todo_core` function accepts a native platform and a transport. Swift and Kotlin use the generated bindings to call it.

The host handles `open_add_dialog`, `close_dialog`, `open_delete_dialog` and `filter_todos`. The server handles `add_todo`, `toggle_todo` and `confirm_delete`. The example signs presentation state. Business actions read the server's store and reject attached presentation state.

The compiler links the domain actions and native core into one library. Swift and Kotlin refer to that same library for callback handles and Rust buffers. The independent starter uses the package's remote factory.

The transport sends one EWF1 frame per request. Strip the four-byte HTTP length before returning a response to the core. The renderer returns each trigger's signed bytes unchanged. See the parent README for server routes. See the wire crate README for hand-written frames and fixture generation.
