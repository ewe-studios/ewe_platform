# Foundation Testbed — Agent Workflow

## Steps

1. **Read `requirements.md`** — understand the high-level architecture and feature index
2. **Read `LEARNINGS.md`** — understand design decisions and past learnings
3. **Identify the feature to implement** from the Feature Index in requirements.md
4. **Navigate to the feature directory** — e.g., `features/01-qemu-backend/`
5. **Read the feature's `start.md`** — follow the feature-level workflow
6. **Implement the feature** — follow tasks in `feature.md` one at a time
7. **Verify each task** before marking complete
8. **Update `LEARNINGS.md`** in the spec root after each milestone
9. **Report to Main Agent** when the feature is complete

## Feature Order

Features have dependencies. Implement in this order:

1. `01-qemu-backend` — no dependencies (start here)
2. `02-vm-communication` — depends on 01
3. `03-bootstrap-build-pipeline` — depends on 02
4. `04-runner-utilities` — depends on 03
5. `05-cli-state-management` — depends on 04
6. `06-bin-integration` — depends on 05

## Reminders

- **No coding without reading the spec first**
- **Update LEARNINGS.md after each milestone**
- **Keep feature.md as the single source of truth — never split architecture into separate files**
