---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/07-templates"
this_file: "specifications/11-foundation-deployment/features/07-templates/feature.md"

status: pending
priority: high
created: 2026-03-26

depends_on: ["04-cloudflare-provider", "05-gcp-cloud-run-provider", "06-aws-lambda-provider"]

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Composable Templates

## Overview

Create composable project templates organized by two dimensions:
- **Language/build type**: How the project is built (Rust worker, Rust WASM, Rust container)
- **Provider**: Where it deploys (Cloudflare, GCP, AWS)

Templates are generated via `ewe_platform generate --lang rust --target cloudflare` and produce a complete project with build setup, provider config file, mise.toml, and CI/CD workflow.

## Dependencies

Depends on:
- `04-cloudflare-provider` - wrangler.toml format and build patterns
- `05-gcp-cloud-run-provider` - service.yaml format and container patterns
- `06-aws-lambda-provider` - template.yaml format and Lambda patterns

Required by:
- `08-mise-integration` - Templates include mise.toml
- `09-examples-documentation` - Examples derived from templates

## Requirements

### Template Matrix

| | Cloudflare | GCP Cloud Run | AWS Lambda |
|--|-----------|--------------|-----------|
| **Rust Worker** | `cf-rust-app` | - | - |
| **Rust WASM** | `cf-rust-wasm-app` | - | - |
| **Rust Container** | - | `gcp-rust-app` | `aws-rust-app` |
| **Rust Lambda** | - | - | `aws-rust-lambda` |

Each template generates:

```
{project-name}/
|-- Cargo.toml                  # (Rust templates)
|-- Dockerfile                  # (Container templates)
|-- {provider-config}           # wrangler.toml / service.yaml / template.yaml
|-- mise.toml                   # Tooling and tasks
|-- README.md                   # Setup and usage
|-- .gitignore
|-- src/
|   +-- {entry-point}           # main.rs / lib.rs
+-- .github/
    +-- workflows/
        +-- deploy.yml          # CI/CD
```

### Template Variables

All templates use `ewe_temple` (MiniJinja) variable substitution:

| Variable | Description | Example |
|----------|-------------|---------|
| `{{project_name}}` | Project name | `my-api` |
| `{{project_name_snake}}` | Snake case name | `my_api` |
| `{{author}}` | Author name | `darkvoid` |
| `{{github_url}}` | GitHub repo base | `github.com/user` |
| `{{target}}` | Deployment target | `cloudflare` |
| `{{region}}` | Cloud region (GCP/AWS) | `us-central1` |
| `{{account_id}}` | Provider account ID | `abc123` |

### Cloudflare Rust Worker Template (cf-rust-app)

**wrangler.toml:**
```toml
name = "{{project_name}}"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[env.staging]
name = "{{project_name}}-staging"
vars = { ENVIRONMENT = "staging" }

[env.production]
name = "{{project_name}}"
vars = { ENVIRONMENT = "production" }
```

**src/main.rs:**
```rust
use worker::*;
use serde::Serialize;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get("/", |_, _| async { Response::ok("Hello from {{project_name}}!") })
        .get("/api/health", |_, _| async {
            Response::from_json(&Health { status: "ok".into() })
        })
        .run(req, env)
        .await
}

#[derive(Serialize)]
struct Health { status: String }
```

### GCP Cloud Run Rust Template (gcp-rust-app)

**service.yaml:**
```yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: {{project_name}}
  labels:
    cloud.googleapis.com/location: {{region}}
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "0"
        autoscaling.knative.dev/maxScale: "10"
    spec:
      containerConcurrency: 80
      timeoutSeconds: 300
      containers:
        - image: IMAGE_PLACEHOLDER
          ports:
            - containerPort: 8080
          resources:
            limits:
              cpu: "1"
              memory: 512Mi
          env:
            - name: ENVIRONMENT
              value: production
```

**Dockerfile:**
```dockerfile
FROM rust:1.87-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/{{project_name_snake}} /app
EXPOSE 8080
ENTRYPOINT ["/app"]
```

**src/main.rs:**
```rust
use axum::{routing::get, Router, Json};
use serde::Serialize;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(|| async { "Hello from {{project_name}}!" }))
        .route("/health", get(health));

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok".into() })
}

#[derive(Serialize)]
struct Health { status: String }
```

### AWS Lambda Rust Template (aws-rust-lambda)

**template.yaml:**
```yaml
AWSTemplateFormatVersion: '2010-09-09'
Transform: AWS::Serverless-2016-10-31
Description: {{project_name}}

Globals:
  Function:
    Timeout: 30
    Runtime: provided.al2023
    Architectures:
      - arm64

Resources:
  ApiFunction:
    Type: AWS::Serverless::Function
    Properties:
      Handler: bootstrap
      CodeUri: target/lambda/{{project_name_snake}}/
      Events:
        Api:
          Type: HttpApi
          Properties:
            Path: /{proxy+}
            Method: ANY
      Environment:
        Variables:
          ENVIRONMENT: production

Outputs:
  ApiUrl:
    Description: API Gateway URL
    Value: !Sub "https://${ServerlessHttpApi}.execute-api.${AWS::Region}.amazonaws.com"
```

**src/main.rs:**
```rust
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use serde::Serialize;

#[tokio::main]
async fn main() -> Result<(), Error> {
    run(service_fn(handler)).await
}

async fn handler(event: Request) -> Result<Response<Body>, Error> {
    let path = event.uri().path();
    match path {
        "/" => Ok(Response::builder()
            .status(200)
            .body("Hello from {{project_name}}!".into())?),
        "/health" => {
            let health = Health { status: "ok".into() };
            Ok(Response::builder()
                .status(200)
                .header("content-type", "application/json")
                .body(serde_json::to_string(&health)?.into())?)
        }
        _ => Ok(Response::builder().status(404).body("Not Found".into())?),
    }
}

#[derive(Serialize)]
struct Health { status: String }
```

### Template Generation CLI

```
ewe_platform generate --lang rust --target cloudflare -p my-worker -o .
ewe_platform generate --lang rust --target gcp -p my-service -o . --region us-central1
ewe_platform generate --lang rust-lambda --target aws -p my-function -o . --region us-east-1
ewe_platform generate --lang rust-wasm --target cloudflare -p my-wasm-worker -o .
```

## Tasks

1. **Define template composition logic**
   - [ ] Create `src/template/mod.rs` and `compose.rs`
   - [ ] Define `TemplateLanguage` and `TemplateTarget` enums
   - [ ] Implement matrix validation (which combos are valid)
   - [ ] Implement `generate()` function that composes language + provider files

2. **Create Cloudflare templates**
   - [ ] `templates/cf-rust-app/` - Rust worker (worker crate)
   - [ ] `templates/cf-rust-wasm-app/` - Rust WASM (wasm-pack)
   - [ ] Include wrangler.toml, mise.toml, src/main.rs, .gitignore, README.md, deploy.yml

3. **Create GCP Cloud Run templates**
   - [ ] `templates/gcp-rust-app/` - Rust container (axum + Docker)
   - [ ] Include service.yaml, Dockerfile, mise.toml, src/main.rs, .gitignore, README.md, deploy.yml

4. **Create AWS Lambda templates**
   - [ ] `templates/aws-rust-lambda/` - Rust Lambda (cargo-lambda)
   - [ ] Include template.yaml, mise.toml, src/main.rs, .gitignore, README.md, deploy.yml

5. **Integrate with ewe_temple**
   - [ ] Register templates with `PackageDirectorate`
   - [ ] Implement MiniJinja variable substitution
   - [ ] Handle file renaming (e.g., `gitignore` -> `.gitignore`)

6. **Write tests**
   - [ ] Test template generation for each combo
   - [ ] Verify generated projects have correct config files
   - [ ] Verify template variables are substituted
   - [ ] Test invalid combo rejection (e.g., `rust-lambda --target gcp`)

## Success Criteria

- [ ] All 6 tasks completed
- [ ] `ewe_platform generate --lang rust --target cloudflare` produces working project
- [ ] `ewe_platform generate --lang rust --target gcp` produces working project
- [ ] `ewe_platform generate --lang rust-lambda --target aws` produces working project
- [ ] Generated projects build successfully
- [ ] Invalid combinations produce clear error messages

## Verification

```bash
# Generate and build each template
for combo in "rust cloudflare" "rust gcp" "rust-lambda aws"; do
  set -- $combo
  ewe_platform generate --lang $1 --target $2 -p test-$2 -o /tmp/test-$2
  cd /tmp/test-$2
  mise install
  mise run build
done
```

---

_Created: 2026-03-26_
