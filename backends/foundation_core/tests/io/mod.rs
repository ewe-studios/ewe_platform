mod buffer_pool_tests;
mod byte_buffer_pointer_read_tests;
mod byte_buffer_pointer_tests;
mod incremental_decoder_tests;
mod ioutils_buffered_tests;
mod mem;
#[cfg(not(target_family = "wasm"))]
mod readers_tests;
mod shared_buffer_tests;
mod shared_byte_buffer_stream_probe;
mod shared_write_reader_tests;
mod stream_ext_tests;
mod stringpointer_tests;
mod ubytes_tests;
