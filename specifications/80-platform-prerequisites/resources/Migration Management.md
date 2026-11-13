# Database Migrations Architecture Discussion

## Topic 1: Migration Granularity (Per Schema vs. Per Table)
**User:** should migrations be per schema or per table what if you want to utilize a change in one schema for a migration in another ?

**AI:** Database migrations should be managed **per database (or per database instance)** rather than strictly per schema or per table. Your migration tracking table (such as `schema_migrations`) acts as a single timeline for the entire physical database engine instance. 

### Per Database vs. Per Schema vs. Per Table

| Granularity | Approach | Best Used For |
| :--- | :--- | :--- |
| **Per Table** | **Avoid.** Tracking migrations per table creates immense complexity, fragmented timelines, and makes managing foreign keys nearly impossible. | N/A |
| **Per Schema** | **Use only if isolated.** Maintain separate migration timelines per schema *only* if the schemas represent completely decoupled, independent microservices or tenants. | Multi-tenant apps, microservices sharing one hardware instance. |
| **Per Database** | **Recommended.** Keep a single, chronological timeline of files for the entire database. A migration file can contain changes for multiple tables across different schemas. | Monoliths, tightly coupled schemas, standard applications. |

### Handling Cross-Schema Dependencies
If **Schema B** needs to utilize a change (like a new column, a new table, or a changed data type) introduced in **Schema A**, your approach depends on how your codebase is structured.

#### Scenario 1: Both schemas are in the same code repository (Tightly Coupled)
If a single application owns both schemas, manage them in a **single database migration timeline**. 
* **How it works:** You write sequential migration scripts using fully qualified names (`schema_name.table_name`). 
* **Execution:**
  1. `Migration 001_create_table_in_schema_a.sql`: Creates `schema_a.users`.
  2. `Migration 002_add_foreign_key_in_schema_b.sql`: Alters `schema_b.profiles` to reference `schema_a.users`.
* **Why it works:** The migration tool executes these linearly, ensuring the dependency in Schema A always exists before Schema B tries to look for it.

#### Scenario 2: The schemas belong to different applications/repositories (Decoupled)
If Team A owns Schema A and Team B owns Schema B, you cannot rely on a single migration timeline. You must treat Schema A as an external API.
* **Step 1: Schema A Deploys Backward-Compatible Change.** Team A deploys their migration to add the new table or column. It must be non-breaking (e.g., nullable or containing default values) so it doesn't break Team B's existing code.
* **Step 2: Schema B Deploys Dependency.** Team B runs a migration that checks for the existence of Schema A's change or simply executes assuming it is there, using explicit error handling if it fails.
* **Alternative (Best Practice):** Avoid cross-schema foreign keys or tight dependencies between separate apps. Instead, have Team B request data from Team A via an API or an asynchronous event broker rather than reading directly from Schema A's database tables.

### Key Rules for Cross-Schema Migrations
1. **Always use fully qualified names:** In your SQL scripts, never write `ALTER TABLE users`. Write `ALTER TABLE schema_a.users` to avoid accidental execution in the default schema.
2. **Order matters:** If using separate migration folders per schema, you must orchestrate your deployment pipeline to run Schema A's migrations completely before starting Schema B's migrations.

---

## Topic 2: PostgreSQL Specific Approach
**User:** say for postgresql whats the recommended approach

**AI:** In PostgreSQL, the highly recommended approach is to manage migrations **per database**, using a single chronological timeline for the entire database instance while utilizing **fully qualified identifiers** (`schema_name.table_name`).

PostgreSQL’s architecture is uniquely optimized for this because **schemas are just namespaces** within a single physical database. They share the same transaction log, boundaries, and catalog tables.

### The Recommended PostgreSQL Blueprint
1. **A Single Migration Timeline:** Maintain one single folder of chronological migration files (e.g., `0001_init.sql`, `0002_add_billing.sql`) for the entire database. Your migration tracking table (like `public.schema_migrations`) lives in a central schema (usually `public`) and acts as the single source of truth for the entire cluster.
2. **Explicit Namespacing (Fully Qualified Names):** Never rely on PostgreSQL's default `search_path`. Inside your migration SQL files, explicitly declare the schema for every single command.
3. **Leverage Transactional DDL:** PostgreSQL supports **Transactional DDL**. This means you can wrap your cross-schema migrations in a single transaction (`BEGIN; ... COMMIT;`). If the change in `schema_a` succeeds but the dependent change in `schema_b` fails, Postgres will **roll back the entire migration**.

### When should you deviate and go "Per Schema"?
The only exception to the "Per Database" rule in PostgreSQL is if you are building an **isolated Multi-Tenant SaaS application** using a *Schema-per-Tenant* strategy. If you have 500 tenants, each with their own identical schema (`tenant_1`, `tenant_2`, etc.), you should use a **Per Schema** runner where you write **one** generic migration blueprint template and loop through all tenants to run it.

---

## Topic 3: Hybrid Structure (Shared + Tenant Migrations)
**User:** ok, what if i have a shared set of migration 0 .. 100 but i have tenant specific sets, how do we make such a structure work?

**AI:** To make a hybrid structure work—where you have a **shared base set** of migrations alongside **tenant-specific sets**—you must separate your migrations into **isolated, independent pipelines** with their own distinct tracking tables. In PostgreSQL, this is typically structured as a **Shared/Global Pipeline** and a **Dynamic Tenant Loop**.

### 1. The Directory Structure
Separate your migration files by scope:
```text
migrations/
├── shared/                  # Applied ONCE globally to the database
│   ├── 0001_init_shared.sql
│   └── 0002_add_global_settings.sql
└── tenants/                 # Applied ONLY to individual tenant schemas
    ├── 0001_init_tenant.sql # Template run against tenant_a, tenant_b, etc.
    └── 0002_add_tenant_billing.sql
```

### 2. How the Tracking Tables Work
PostgreSQL isolates tracking by placing the migration history tables exactly where the changes are happening:
* **Shared Tracking:** A single table lives in your global namespace, usually `public.shared_schema_migrations`.
* **Tenant Tracking:** Every single tenant schema gets its *own private copy* of the tracking table, for example, `tenant_a.tenant_schema_migrations` and `tenant_b.tenant_schema_migrations`.

### 3. Execution Order & Cross-Schema Links
1. **Step 1: Run the Global Pipeline:** Deploy the `migrations/shared/` folder first to target global tables or core lookups.
2. **Step 2: Loop the Tenant Pipeline:** Your migration runner fetches a list of active tenant schemas, overrides Postgres's `search_path` to target that tenant, then executes the `migrations/tenants/` scripts.

### 4. How to Implement This in Common Tools
* **Flyway / Liquibase:** Execute the tool **twice** in your deployment script. Once for `migrations/shared`, and a second time inside a loop altering the `-schemas` and `-table` flags for every tenant.
* **Atlas (by Ariga):** Uses "Multi-tenant" declarative patterns where you define a desired tenant schema state and apply it dynamically.
* **Prisma / Rails / Django:** Often require handling the tenant-looping layer manually via custom runner scripts or specific multi-tenancy library extensions (like `django-tenants` or `ros-apartment`).