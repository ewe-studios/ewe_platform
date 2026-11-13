---
feature: "Selectable OCI registry backend: Docker Hub or GHCR"
status: "planned"
priority: "medium"
depends_on: ["01-storage-config-and-manifest"]
crates: []
issues: []
model: "sonnet"
branch: "spec74/f03-registry-backend"
---

# Feature 03: Selectable OCI registry backend

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** continuous integration (CI); Docker Hub (DH); GitHub Actions
(GHA); GitHub Container Registry (GHCR); GitHub token (`GITHUB_TOKEN`); Hypertext Transfer
Protocol (HTTP); Open Container Initiative (OCI); pull request (PR); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Make the OCI registry a configuration choice: Docker Hub or GHCR. Both must support push,
pull, digest pinning and anonymous pull of public images. No workflow may hard-code the
registry name.

## 2. Measured state

1. `ci/ewe-platform-image/config.toml` sets `[registry] host = "docker.io"`, `namespace =
   "ewestudios"`, `username = "ewestudios"`.
2. `ci/platform-images/scripts/images.nu:57` `repository` builds
   `<host>/<namespace>/<name>[-<flavor>]`.
3. `images.nu:52` `registry-username` reads `PLATFORM_IMAGES_REGISTRY_USERNAME`.
4. `images.nu:349` `main exists` retries HTTP 429 and treats a missing image or a rate limit
   as "missing" when there are no credentials.
5. `.github/workflows/platform-images.yaml:129` logs in with `docker/login-action` when a
   password secret exists. The secret name is an input.
6. The images are public today. The VM container images contain no operating system bits, so
   they can stay public.

## 3. Requirements

1. `storage.backend = "dockerhub" | "ghcr"` selects the registry host, the namespace and the
   token source. A single function returns all three. No other code branches on the backend.
2. Docker Hub: host `docker.io`, namespace from the config (`ewestudios`), token from
   `DOCKERHUB_EWESTUDIO_TOKEN`.
3. GHCR: host `ghcr.io`, namespace `ewe-studios` (lower case), token from `GITHUB_TOKEN`.
4. A repository variable `PLATFORM_IMAGES_STORAGE_BACKEND` overrides the config backend, like
   `PLATFORM_IMAGES_REGISTRY_USERNAME` overrides the username today.
5. Pull: an anonymous pull works for a public package on both hosts. With a token, the pull
   is authenticated. A 429 retry is bounded by the existing retry loop.
6. Push: the workflow logs in to the selected host with the selected token. The workflow
   fails early, with a clear message, when the token is missing on an event that must push.
7. Digest references keep their current shape: `<host>/<namespace>/<name>@sha256:<digest>`.
8. The GHCR package must be public. Document the first-push step that links the package to
   the repository and sets public visibility. A private package is a typed error in the
   workflow check, not a silent cost.

## 4. Design

### 4.1 One resolver

```nu
# In storage.nu
export def registry-target [storage: record]: nothing -> record
# { host, namespace, username, token_env, push_allowed }
```

| backend | host | namespace | username | token_env |
| --- | --- | --- | --- | --- |
| `dockerhub` | `docker.io` | config `[registry] namespace` | config `[registry] username` | `DOCKERHUB_EWESTUDIO_TOKEN` |
| `ghcr` | `ghcr.io` | `ewe-studios` | `$env.GITHUB_ACTOR` or `wex` | `GITHUB_TOKEN` |

`images.nu` uses `registry-target` in `repository` and `main prepare`. The plan output gains
`registry_backend`, `registry_host`, `registry_username` and `token_env`. The workflow reads
the plan and logs in with those values.

### 4.2 Workflow change

```yaml
- id: plan
  run: nu ci/platform-images/scripts/images.nu prepare ... --github-output
- name: Log in to ${{ steps.plan.outputs.registry_host }}
  if: steps.plan.outputs.push == 'true'
  uses: docker/login-action@v4.7.0
  with:
    registry: ${{ steps.plan.outputs.registry_host }}
    username: ${{ steps.plan.outputs.registry_username }}
    password: ${{ secrets[steps.plan.outputs.token_env] }}
```

A fork PR has no token. `push` stays false and the image is exported as an artifact, as
today.

### 4.3 Public visibility check

The workflow has a check step:

```nu
nu ci/platform-images/scripts/images.nu check-visibility \
  --config ci/ewe-platform-image/config.toml --ref <ref>
```

For GHCR it requests the anonymous token and the manifest without credentials. A 401 or 403
means the package is private: fail with a message that names the package and the fix. For
Docker Hub, request the manifest anonymously.

### 4.4 Rate-limit measurement

`main exists` already retries 429. Add an output line with the attempt count and the wait
time, so feature 07 can total the retries in a full matrix run. Do not add a cache or a proxy
in this feature.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/storage.nu` | Add `registry-target`. |
| `ci/platform-images/scripts/images.nu` | Use `registry-target`; add `check-visibility`; print the token source. |
| `.github/workflows/platform-images.yaml` | Login from the plan; add the visibility check. |
| `.github/workflows/check.yaml` | Use the plan's login values if it has its own login step. |
| `ci/ewe-platform-image/config.toml` | Document `backend` and the GHCR namespace. |
| `ci/platform-images/README.md` | Section: choose a registry, set the token, make the package public. |
| `ci/platform-images/tests/storage_tests.nu` | Cases for the resolver and ref shapes. |

## 6. Tasks

- [ ] Add `registry-target` and the backend override.
- [ ] Use it in `repository`, `main prepare` and the plan output.
- [ ] Add `check-visibility`.
- [ ] Update both workflows to log in from the plan output.
- [ ] Document the GHCR package setup and the public visibility rule.
- [ ] Add the resolver tests.
- [ ] Measure the Docker Hub retry count in one full matrix run and record it in
      `progress.md` (feature 07 repeats it after the change).

## 7. Tests

| Test | Asserts |
| --- | --- |
| `test registry target dockerhub` | Host, namespace, username and token environment. |
| `test registry target ghcr` | Host `ghcr.io`, namespace `ewe-studios`, token `GITHUB_TOKEN`. |
| `test registry target override` | `PLATFORM_IMAGES_STORAGE_BACKEND=ghcr` wins over the config. |
| `test repository ref shapes` | Default and CUDA refs for both hosts; digest refs keep the host. |
| `test repository rejects unknown backend` | A typed error names the two valid values. |
| `test check visibility` | A local registry fixture shows the public and the private case. |

The visibility test uses a local registry container (`registry:2`) with and without auth. It
is part of `ci/platform-images/tests/docker-compose.yml`.

## 8. Verification

```bash
mise run test:ci-storage -- --case registry
# Both must print the registry record without a network call.
PLATFORM_IMAGES_STORAGE_BACKEND=dockerhub \
  nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml \
  --layer base --flavor default --out /tmp/pi-base
PLATFORM_IMAGES_STORAGE_BACKEND=ghcr \
  nu ci/platform-images/scripts/images.nu prepare --config ci/ewe-platform-image/config.toml \
  --layer base --flavor default --out /tmp/pi-base
```

The full proof is the PR's `check.yaml` run on the selected backend.

## 9. Done when

- The plan and the workflow use one resolver. No workflow string contains a registry host.
- A public image pulls anonymously on the selected backend, and a token pull works.
- A private GHCR package fails the visibility check with a clear message.
- The README explains the setup for both backends.
- `progress.md` and `learnings.md` record the feature.

## 10. Risks

| Risk | Control |
| --- | --- |
| A GHCR package starts private and passes silently. | The visibility check runs before the build push. |
| A token with the wrong scope returns 403 on push. | The workflow fails early with the token environment name. |
| `GITHUB_TOKEN` cannot push a package in another organisation. | The repository must own the package; document the `permissions: packages: write` line. |
| The GHCR namespace differs from the Docker Hub namespace. | The resolver owns both values; tests pin them. |
