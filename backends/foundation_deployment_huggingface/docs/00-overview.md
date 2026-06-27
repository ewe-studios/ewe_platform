# foundation_deployment_huggingface — Hugging Face model deployment

## What it is
Hugging Face Hub integration: model downloading, caching, and format conversion.

## Key modules
- **`hub/`** — Hugging Face Hub API client (model listing, file downloads).
- **`cache/`** — Model cache with LRU eviction and deduplication.
- **`convert/`** — Format conversion (GGUF, safetensors, ONNX).

## Integration
- Used by `foundation_ai`'s `llamacpp` feature to download GGUF models.
- Used by `foundation_ai`'s `candle` feature to download safetensors models.
