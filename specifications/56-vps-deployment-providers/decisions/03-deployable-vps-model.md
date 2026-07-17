# 03 — The `Deployable` VPS model

**Date:** 2026-07-17
**Status:** Open

## The question

The owner's requirement: **"we setup deployables for these to use the crates or
API each provider provides to deploy, setup and harden these VPS instances for
use."**

So each provider crate exposes a `Deployable`. What does `deploy` guarantee, what
does it persist, and what happens when it is called twice?

## Prior art in this tree

`foundation_deployment_docker::ContainerDeployment` is the model and it is close
to what we want (spec-54, decision 05):

- `deploy` creates + starts a container and **persists its id** through the
  `Deployable::store` (a namespaced state store on the `ProviderClient`).
- `destroy` reads the id back and stops + removes it.
- Both are plain async returning `BoxFuture`.
- It builds its **own** client rather than using the `ProviderClient`'s HTTP
  client, because Docker speaks over a Unix socket — "the canonical unique
  underlying mechanics case".

A VPS deployment is the same shape with a different mechanism: an HTTPS API
instead of a Unix socket, and an instance id instead of a container id.

## What to decide

### 1. What does `deploy` guarantee on return?

A VPS that exists is not a VPS you can use. Proposal — `deploy` returns only when
**all** of:

1. the instance exists and the vendor reports it running;
2. it has a public IP;
3. sshd answers on 22 with our key (the vendor's "running" precedes sshd by
   many seconds);
4. hardening is applied (decision 04);
5. Docker is installed and `docker version` answers over SSH.

Anything less and the caller has to re-implement the waiting, which is where the
subtle bugs live. Counter-argument: that is a lot for one call — should
`deploy` be split (`create` / `harden` / `bootstrap`), with the `Deployable`
composing them?

### 2. What is persisted?

At minimum the instance id, so `destroy` works after a process restart. Probably
also the public IP and the provider name — a caller that wants to
`connect_ssh` after a restart should not have to call the API again. The store is
namespaced per deployable.

### 3. Is `deploy` idempotent?

If state holds an id and that instance still exists, does `deploy` return it, or
create a second box? Proposal: **return the existing one** (create-or-find, as
`NetworkHandle::create_or_find` does in spec-53) — a duplicate VPS costs real
money, which makes an accidental second create worse than a no-op.

But: what if the recorded instance is *gone* (deleted out of band)? Recreate
silently, or fail and make the caller clear the state?

### 4. What does `destroy` do about the SSH key?

If `deploy` uploaded a key to the vendor's account, does `destroy` remove it?
Shared keys across deployments make this dangerous — deleting a key another
instance still uses would lock it out.

### 5. Rollback on partial failure

If create succeeds and hardening fails, do we destroy the instance or leave it
for inspection? Leaving it costs money silently; destroying it discards the
evidence. Spec-53 hit the mirror of this and chose cleanup: `ContainerHandle::start_async`
now removes a container when a later step fails, because a stranded container
"kept holding its ports and made every later run fail". A stranded VPS bills.

Proposal: **destroy on failure by default**, with an opt-out for debugging —
matching the `force_rm` shape landed in spec-53 F04.

## To resolve

1. Does `deploy` do everything (create→harden→bootstrap), or compose smaller steps?
2. What is persisted beyond the id?
3. Idempotent create-or-find? And what if the recorded instance is gone?
4. Does `destroy` touch vendor-side SSH keys?
5. Destroy-on-partial-failure by default, with an opt-out?
