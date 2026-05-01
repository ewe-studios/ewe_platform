---
title: Executive AI Assistant -- Email Triage Agent
---

# Executive AI Assistant -- Email Triage Agent

## Purpose

An AI agent that acts as an Executive Assistant — monitors a Gmail inbox, triages emails, drafts responses, schedules meetings, and sends calendar invites. Uses LangChain Auth for Google OAuth.

Source: `/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AgenticLibraries/src.langchain/executive-ai-assistant/`

## Aha Moments

**Aha: Deterministic thread IDs from email thread_id MD5.** The cron graph creates/reuses threads by hashing the Gmail thread_id — same email always maps to the same LangGraph thread, enabling stateful multi-turn handling.

**Aha: Two graphs — main and cron.** The main graph handles per-email processing (triage → human → action). The cron graph periodically fetches new emails and kicks off runs against the main graph with `multitask_strategy="rollback"`.

## Main Graph (`eaia/main/graph.py:1-191`)

```
START --> triage_input --> route_after_triage
                                |
         +----------------------+----------------------+
         |              |              |               |
    draft_response    "no"         "notify"        "question"
         |              |              |               |
    human_node      mark_as_read    notify_node    send_email_node
         |
    enter_after_human
         |
    +----+----+----------+
    |         |          |
send_email  send_cal   rewrite
  invite    invite
```

### Nodes

| Node | File | Purpose |
|------|------|---------|
| `triage_input` | `triage.py:46` | Classifies email as: "email" (draft response), "no" (mark as read), "notify" (alert user), "question" (draft response) |
| `draft_response` | `draft_response.py` | Drafts email response using LLM |
| `take_action` | Conditional edge | Routes to appropriate action based on agent decision |
| `human_node` | `human_inbox.py:153` | Human-in-the-loop pause point |
| `enter_after_human` | After approval | Routes to action node |
| Action nodes | Various | `send_email_node`, `send_cal_invite_node`, `rewrite`, `notify`, `mark_as_read_node` |

## Cron Graph (`eaia/cron_graph.py:1-54`)

1. Fetches group emails via Gmail (last N minutes)
2. Creates/reuses threads (deterministic thread IDs from email thread_id MD5 hash, line 23)
3. Kicks off runs against "main" graph with `multitask_strategy="rollback"`

## langgraph.json

Defines 4 graphs: `main`, `cron`, `general_reflection_graph`, `multi_reflection_graph`. Uses OpenAI text-embedding-3-small for store indexing.

## Key Files

- `eaia/main/graph.py` — Main agent graph
- `eaia/main/triage.py` — Email triage classification
- `eaia/main/draft_response.py` — Email drafting
- `eaia/main/rewrite.py` — Tone/style rewriting
- `eaia/main/find_meeting_time.py` — Calendar scheduling
- `eaia/main/human_inbox.py` — HITL action dispatch
- `eaia/cron_graph.py` — Email ingestion cron
- `eaia/gmail.py` — Gmail API interactions
- `eaia/reflection_graphs.py` — Memory/reflection graphs
- `eaia/main/config.yaml` — User configuration

[Back to main index → ../README.md](../README.md)
