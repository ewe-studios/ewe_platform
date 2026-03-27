# Provider OpenAPI Specifications for API Based Deployment

1. [FlyIO](https://docs.machines.dev/spec/openapi3.json)
2. [PlanetScale OpenAPI specification](https://api-docs.planetscale.com/)
3. [PlanetScale V1 Specification](https://api.planetscale.com/v1/openapi-spec)
4. [Cloudflare OpenAPI specification](https://github.com/cloudflare/api-schemas)
5. [GCP OpenAPI Specifications](https://developers.google.com/discovery)
6. [Prisma Postgres](https://api.prisma.io/v1/doc)
7. [SupaBase](https://api.supabase.com/api/v1-json)
8. [MongoDB Atlas](https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json)
9. [Neon API](https://neon.com/api_spec/release/v2.json)
10. [Stripe](https://docs.stripe.com/api)

See the `distilled-*` projects  here:

1. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled
2. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-cloudflare
3. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-planetscale
4. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-fly-io
5. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-gcp
6. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-mongodb-atlas
7. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-neon
8. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-planetscale
9. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-prisma-postgres
10. /home/darkvoid/Boxxed/@formulas/src.rust/src.deployAnywhere/distilled-spec-supabase

Lets also create with valtron (similar to /home/darkvoid/Boxxed/@dev/ewe_platform/bin/platform/src/gen_model_descriptors/mod.rs), a new command under `platform` that re-implements the different processes (see the directories listed above to pull these JSON OpenAPI specs for each above).
