# Executive AI Assistant -- Email Triage Agent

AI agent that monitors Gmail inbox, triages emails, drafts responses, schedules meetings, and sends calendar invites.

## Documents

- [00 Architecture](00-architecture.md) — Two graphs (main + cron), email triage → human-in-loop → action, deterministic thread IDs from Gmail thread_id

## Workflow

```
Cron: Fetch Gmail --> triage_input --> route
                           |
            +--------------+--------------+
            |              |              |
       draft_response    "no"          "notify"
            |              |              |
       human_node     mark_as_read     notify_node
            |
       action: send_email / send_cal_invite / rewrite
```
