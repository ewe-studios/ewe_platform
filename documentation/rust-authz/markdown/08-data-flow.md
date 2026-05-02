---
title: End-to-End Data Flows — Check Resolution, Policy Management, REST Queries, and Telemetry Ingestion
---

# Data Flow — End-to-End Flows

This document traces the complete execution paths for the four major workflows across the projects.

## Flow 1: Authorization Check via pgauthz

```mermaid
sequenceDiagram
    participant App as Application (SQL)
    participant PG as PostgreSQL
    participant pgauthz as pgauthz extension
    participant Cache as TypeSystem Cache
    participant SPI as PostgresDatastore (SPI)
    participant Resolver as CoreResolver
    participant Tables as authz.* tables

    App->>PG: SELECT pgauthz_check('document','doc1','viewer','user','alice')
    PG->>pgauthz: pgauthz_check()
    pgauthz->>pgauthz: validate_check_args()
    pgauthz->>Cache: load_typesystem_cached()
    Cache->>SPI: read_latest_authorization_policy()
    SPI->>Tables: SELECT FROM authz.authorization_policy
    Tables->>SPI: policy row
    SPI->>Cache: Policy definition
    Cache->>pgauthz: Arc<TypeSystem>
    pgauthz->>SPI: read_latest_revision()
    SPI->>Tables: SELECT FROM authz.revision
    Tables->>SPI: revision ULID
    SPI->>pgauthz: quantized revision
    pgauthz->>Resolver: CoreResolver::new(datastore, provider)
    Resolver->>Resolver: with_result_cache() + with_tuple_cache()
    pgauthz->>Resolver: LocalDispatcher.dispatch_check(request)
    Resolver->>Resolver: resolve_check() — walk model graph
    loop For each relation in expression
        Resolver->>SPI: read_tuples(filter)
        SPI->>Tables: SELECT FROM authz.tuple
        Tables->>SPI: matching tuples
        SPI->>Resolver: tuples
    end
    Resolver->>pgauthz: CheckResult::Allowed/Denied
    pgauthz->>pgauthz: record metrics
    pgauthz->>PG: return boolean
    PG->>App: true or false
```

Source: `pgauthz/crates/pgauthz/src/check_functions.rs:17-143` (`do_check`).

## Flow 2: Policy Definition and Validation

```mermaid
sequenceDiagram
    participant App as Application (SQL)
    participant PG as PostgreSQL
    participant pgauthz as pgauthz extension
    participant Parser as model_parser
    participant Validator as model_validator
    participant CEL as cel module
    participant SPI as PostgresDatastore
    participant Tables as authz.* tables

    App->>PG: SELECT pgauthz_define_policy('type user {} ...')
    PG->>pgauthz: pgauthz_define_policy()
    pgauthz->>SPI: PostgresDatastore::new()
    pgauthz->>SPI: write_authorization_policy(policy)
    SPI->>Parser: parse_dsl(definition)
    Parser->>Parser: pest parse → ModelFile
    Parser->>SPI: AST
    SPI->>Validator: validate_model(ModelFile)
    Validator->>Validator: check duplicates, undefined refs, cycles
    Validator->>SPI: Ok or Err(ValidationErrors)
    SPI->>CEL: compile each condition expression
    CEL->>CEL: cel::Program::compile()
    CEL->>SPI: Ok or Err(CelError)
    SPI->>SPI: generate ULID
    SPI->>Tables: INSERT INTO authz.authorization_policy
    Tables->>SPI: OK
    SPI->>Tables: INSERT INTO authz.revision
    Tables->>SPI: OK
    SPI->>pgauthz: policy ID
    pgauthz->>PG: return policy ID
    PG->>App: ULID string
```

Source: `authz-datastore-pgx/src/lib.rs:254-313` (PolicyWriter impl).

**Aha:** Policy writing is a three-stage validation gate: parse → validate → CEL compile. If any stage fails, the policy is rejected and nothing is written to the database. This prevents a partially-valid policy from becoming the "latest" and breaking all subsequent checks.

## Flow 3: Relationship Write with Validation

```mermaid
sequenceDiagram
    participant App as Application (SQL)
    participant PG as PostgreSQL
    participant pgauthz as pgauthz extension
    participant SPI as PostgresDatastore
    participant Parser as model_parser
    participant Tables as authz.* tables

    App->>PG: SELECT pgauthz_add_relation('document','doc1','viewer','user','alice')
    PG->>pgauthz: pgauthz_add_relation()
    pgauthz->>pgauthz: write_relationships([tuple], [])
    pgauthz->>SPI: write_tuples(writes, deletes)
    SPI->>SPI: read_latest_authorization_policy()
    SPI->>Parser: parse_dsl(definition)
    Parser->>SPI: ModelFile
    SPI->>SPI: validate tuple against model
    SPI->>Tables: INSERT INTO authz.tuple (ON CONFLICT DO UPDATE)
    Tables->>SPI: OK
    SPI->>Tables: INSERT INTO authz.changelog ('write', ulid)
    Tables->>SPI: OK
    SPI->>Tables: INSERT INTO authz.revision (revision_id)
    Tables->>SPI: OK
    SPI->>pgauthz: revision ULID
    pgauthz->>pgauthz: record tuple write metrics
    pgauthz->>PG: return revision ULID
    PG->>App: ULID string
```

Source: `authz-datastore-pgx/src/lib.rs:557-669` (TupleWriter impl).

## Flow 4: Watch API — Reading Changes

```mermaid
sequenceDiagram
    participant App as Application (SQL)
    participant PG as PostgreSQL
    participant pgauthz as pgauthz extension
    participant SPI as PostgresDatastore
    participant Tables as authz.changelog

    App->>PG: SELECT * FROM pgauthz_read_changes('document', NULL, 100)
    PG->>pgauthz: pgauthz_read_changes()
    pgauthz->>pgauthz: validate_read_changes_args()
    pgauthz->>SPI: read_changes('document', None, 100)
    SPI->>Tables: SELECT FROM authz.changelog WHERE object_type='document' ORDER BY ulid LIMIT 100
    Tables->>SPI: changelog rows
    SPI->>pgauthz: Vec<ChangelogEntry>
    pgauthz->>PG: TableIterator rows
    PG->>App: (object_type, object_id, relation, subject_type, subject_id, operation, ulid)
```

Source: `pgauthz/crates/pgauthz/src/lib.rs:308-354` and `authz-datastore-pgx/src/lib.rs:674-725`.

The Watch API enables clients to poll for changes since a known ULID cursor, implementing a simplified version of Zanzibar's consistency model.

## Flow 5: dbrest REST Query

```mermaid
sequenceDiagram
    participant Client as HTTP Client
    participant axum as axum Router
    participant Auth as JWT middleware
    participant Req as ApiRequest
    participant Plan as action_plan
    participant Query as query generator
    participant Schema as SchemaCache
    participant Backend as DatabaseBackend
    participant DB as PostgreSQL/SQLite

    Client->>axum: GET /documents?select=*&status=eq.active
    axum->>Auth: JWT middleware
    Auth->>Auth: validate token (cached)
    Auth->>axum: AuthResult
    axum->>Req: Parse request
    Req->>Req: Extract: method, path, params, headers
    Req->>Schema: lookup table schema
    Schema->>Req: columns, types, PKs
    Req->>Plan: Build action plan
    Plan->>Query: Generate SQL
    Query->>Backend: execute(sql, params)
    Backend->>DB: Run prepared statement
    DB->>Backend: Result rows
    Backend->>Query: Raw values
    Query->>axum: JSON response body
    axum->>Client: 200 OK + JSON
```

Source: `dbrest/src/main.rs` (server startup), `dbrest-core/src/api_request/` (request parsing), `dbrest-core/src/plan/` (action planning).

## Flow 6: OTLP Telemetry Ingestion

```mermaid
sequenceDiagram
    participant SDK as OTel SDK
    participant tonic as tonic gRPC server
    participant api_optel as api-optel service
    participant Models as zradar-models
    participant Writer as TelemetryWriter
    participant Plugin as plugin-clickhouse
    participant CH[(ClickHouse)]

    SDK->>tonic: ExportTraceServiceRequest (gRPC)
    tonic->>api_optel: Route to OTLP handler
    api_optel->>Models: Parse OTLP protobuf
    Models->>Models: Convert to internal types
    Models->>Writer: write_spans(batch)
    Writer->>Plugin: ClickHouse INSERT batch
    Plugin->>CH: Batch insert (LZ4 compressed)
    CH->>Plugin: OK
    Plugin->>Writer: OK
    Writer->>Models: OK
    Models->>api_optel: OK
    api_optel->>tonic: ExportTraceServiceResponse
    tonic->>SDK: OK
```

Source: `zradar/crates/services/api-optel/` (OTLP service), `zradar/crates/plugins/zradar-plugin-clickhouse/` (ClickHouse writer).

## Flow 7: Revision-Based Cache Invalidation

```mermaid
sequenceDiagram
    participant Write as Tuple/Policy Write
    participant SPI as PostgresDatastore
    participant Revision as authz.revision
    participant Check as pgauthz_check
    participant Cache as Result/Tuple Cache

    Write->>SPI: write_tuples(...)
    SPI->>Revision: INSERT revision (new ULID)
    Revision->>SPI: new_revision_id
    Note over Check,Cache: Next check uses NEW revision
    Check->>SPI: read_latest_revision()
    SPI->>Revision: SELECT latest revision
    Revision->>SPI: new_revision_id
    SPI->>Check: quantize(new_revision_id)
    Check->>Cache: lookup key = new_revision:...
    Note over Cache: Old keys (old_revision:...) are ORPHANED
    Cache->>Check: MISS (new revision)
    Check->>SPI: read from datastore
    SPI->>Cache: insert key = new_revision:...
```

**Aha:** Cache invalidation is achieved without explicit invalidation. Each write creates a new revision. Each check reads the latest revision and includes it in the cache key. Old cache entries (keyed by previous revisions) are never looked up again — they naturally expire. This is simpler than invalidation and immune to race conditions.

## Flow 8: zradar Plugin Loading

```mermaid
sequenceDiagram
    participant Server as zradar-server
    participant Registry as zradar-plugins
    participant FS as Filesystem
    participant Postgres as plugin-postgres
    participant ClickHouse as plugin-clickhouse
    participant Config as Configuration

    Server->>Config: Load config (which plugins to enable)
    Config->>Server: plugin list
    Server->>Registry: load_plugins(plugins)
    alt Static linking (rlib)
        Registry->>Registry: Initialize plugin crates directly
    else Dynamic loading (cdylib)
        Registry->>FS: dlopen("libzradar_plugin_postgres.so")
        FS->>Registry: Library handle
        Registry->>Registry: Call plugin init function
    end
    Registry->>Postgres: Initialize (connect pool)
    Registry->>ClickHouse: Initialize (connect pool)
    Postgres->>Registry: Register as TelemetryWriter
    ClickHouse->>Registry: Register as TelemetryWriter
    Registry->>Server: Plugin registry ready
```

## What to Read Next

Continue with [09-cross-cutting.md](09-cross-cutting.md) for error handling, observability, and testing patterns shared across all four projects.
