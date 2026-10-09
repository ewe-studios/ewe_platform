# Start: Feature 11 - Request Batching

## Workflow

1. Read `features.md` for this feature
2. Implement `RequestQueue`, `WSBatchQueue`, `WorkerBatchQueue` in `assets/batching.js`
3. Implement `probeBatching()` HEAD probe
4. Wire into Transport classes (F06)
5. Ensure server exposes `HEAD /primal/messages` (F10)
6. Test: probe detection, HTTP batching, WebSocket buffering, worker batching
