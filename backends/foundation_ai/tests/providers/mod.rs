mod anthropic_messages_provider;
mod embedding_provider_tests;
mod huggingface_gguf_provider;
mod llamacpp_config_tests;
mod llamacpp_fixture;
mod model_trait_surface_tests;
mod openai_message_shapes_tests;
mod openai_provider;
mod provider_catalog_tests;
mod provider_credentials_tests;
mod provider_fault_injection_tests;

#[cfg(feature = "llamacpp")]
mod llamacpp_provider;

// Also compiled under `cuda`: the GPU integration test uses a committed fixture,
// so it must be reachable with just the `cuda` feature. The other integration
// tests here self-skip when their downloaded models are absent, so pulling them
// in under `cuda` costs nothing.
#[cfg(any(feature = "live-model-tests", feature = "cuda"))]
mod integrations;

#[cfg(feature = "external-service-tests")]
mod external;
