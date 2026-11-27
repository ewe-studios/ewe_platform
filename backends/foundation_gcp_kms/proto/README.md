# Cloud KMS protos

This tree holds the protobuf definitions of the Google Cloud Key Management
Service (KMS) v1 API. The build script compiles them with `buffa-build` into
the message types of `foundation_gcp_kms`.

## Source

The files come from the `googleapis/googleapis` repository at commit
`fe04226e3d76435acbfd3ffb51f6771740e1fb5b` (2026-07-15). They are copied
without a change.

| File | sha256 |
|---|---|
| `google/cloud/kms/v1/service.proto` | `f54956a732d3b3a4698c6f3172a091721b721d5ba0f78dc6f767de03dc9ad5a7` |
| `google/cloud/kms/v1/resources.proto` | `c0463064beeec1ae3644ee688a68b73e0155eeabe3f272444be5ecb6e35cb6c9` |
| `google/api/annotations.proto` | `e79ea741cb605a65e78ca322174764a4af9fde1962c1631e12b84c4934ba9a6c` |
| `google/api/client.proto` | `5e9fc117290fe7d3472791543c979bec7cf4497f3fe6ffaf9de9ae99cc4f1804` |
| `google/api/field_behavior.proto` | `bb7ed77725c95f751ec2c47cd92359d0137c39383b8a62aef41bab7aa7478da5` |
| `google/api/http.proto` | `ead99129aa15dd5f6233942030c72eec33bc0d7b1c7c260dc143293ef66c5b78` |
| `google/api/launch_stage.proto` | `68e22c9821227b4e84c7029569fce1055c21e6ccab991e0986bc75f12ef4dd82` |
| `google/api/resource.proto` | `815d51eada4341bf01672b474b5ec0abbea190e8df9f4aa26a0c96dbdad2993b` |
| `google/longrunning/operations.proto` | `5d2e7f9eeb55b8417b50e0a536a092babd32d26e4fbeb7099c6cdcda6e16db9c` |
| `google/rpc/status.proto` | `f5bfd262e6705c7ae73f32e0ad8ee20ce8c0a2578df8c4f76ebf76b572f295ed` |

`SHA256SUMS` holds the same hashes in the `sha256sum` format. The test
`proto_manifest_matches` recomputes them, so a change to a vendored file
without a note in this README fails.

## The tree is complete and minimal

The tree holds every file that the two Cloud KMS protos import, except the
`google.protobuf` well-known types. Those files (for example
`google/protobuf/timestamp.proto`) come from the `include` directory of
`protoc`, and `buffa-build` maps them to `buffa-types`. A copy here would not
be used.

The service proto also imports `google/longrunning/operations.proto` for the
methods that return a long-running operation, such as `DeleteCryptoKey`. The
build compiles that file and `google/rpc/status.proto` as well, so the
`google.longrunning.Operation` answer decodes.

## License

The files are Apache License 2.0, the license of the `googleapis` repository.
