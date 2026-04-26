---
title: Paperclip -- Sub-Projects
---

# Paperclip -- Sub-Projects

Paperclip is distributed across multiple sub-projects, each serving a distinct purpose in the ecosystem.

## Overview

| Project | Stack | Purpose | Location |
|---------|-------|---------|----------|
| [Clipmart](#clipmart) | Next.js | Company templates marketplace | `clipmart/` |
| [PR Reviewer](#pr-reviewer) | Cloudflare Workers (Hono) | PR triage dashboard | `pr-reviewer/` |
| [Companies Tool](#companies-tool) | CLI (TypeScript) | Import/export agent companies | `companies-tool/` |
| [Paperclip Website](#paperclip-website) | Astro | Marketing site | `paperclip-website/` |

## Clipmart

**Company templates marketplace.** Browse pre-built company templates -- full org structures, agent configs, and skills -- and import them into your Paperclip instance in seconds.

### What It Does

- Browse available company templates
- Preview org structures, agent roles, and configurations
- One-click import into your Paperclip instance
- Template versioning and updates

### Status

Coming soon. The infrastructure for company import/export already exists via the companies-tool CLI and the Paperclip import/export API. Clipmart adds a web-based marketplace layer on top.

### Architecture

Clipmart is built with Next.js and serves as a frontend for discovering and installing company templates. Templates are stored as Agent Companies packages (markdown-first packages following the open standard at [companies.io](https://companies.io)).

## PR Reviewer

**PR triage dashboard** for the paperclipai/paperclip repository. It syncs open (and recent closed/merged) pull requests from GitHub, scores them, and presents a ranked list to help maintainers decide what to review next.

**Live dashboard:** https://pr-triage.bippadotta.workers.dev

### How Scoring Works

Every PR receives a **composite score from 0 to 180**, built from ten signals:

#### Base Signals (0-115 points)

| Signal | Points | How It Works |
|--------|--------|--------------|
| **Greptile confidence** | 0-40 | Greptile bot confidence score (1-5) multiplied by 8 |
| **CI status** | 0-25 | Passing = 25, pending = 12, unknown = 8, failing = 0 |
| **Merge conflicts** | -15 to +15 | No conflicts = +15, has conflicts = -15 |
| **Human comments** | 0-20 | 1 comment = 10, 2+ comments = 20 (bot comments excluded) |
| **Lines of code** | 0-15 | Smaller PRs score higher (logarithmic decay) |

#### Contributor Priority (-25 to +25 points)

Each author gets an internal priority score (0-100) based on their history, mapped to -25 to +25 composite points:

- **First-time contributors** get a +15 bonus
- **Track record** (0-10): based on merged PR count
- **Merge rate**: 80%+ = +10, 60-79% = +5, 40-59% = 0, 20-39% = -15, <20% = -30
- **Open PR load** (0-10): authors with many open PRs get a small boost

#### Bonus Signals (0-40 points)

| Signal | Points | How It Works |
|--------|--------|--------------|
| **Includes tests** | +10 | PR touches test files |
| **Thinking Path** | +10 | PR description contains "Thinking Path" |
| **Issue link** | +10 | PR description links to a GitHub issue |
| **Freshness** | 0-10 | Newer PRs score higher (<1 day = 10, older = 0) |

### Architecture

- Runs as a **Cloudflare Worker** with a D1 database
- Syncs PR data from GitHub on a schedule
- All scoring logic lives in `src/scoring.ts`
- Merges to `master` are automatically deployed via CI

### Deployment

```bash
npm install
# Create .dev.vars with CLOUDFLARE_API_TOKEN and CLOUDFLARE_ACCOUNT_ID
npx wrangler deploy
```

## Companies Tool

**The CLI for the Agent Companies open standard.** Browse installable companies at [companies.sh](https://companies.sh).

### What It Does

- Install agent companies from GitHub, URLs, or local paths
- Import into Paperclip or other orchestrators
- List available companies in the provider

### Install Commands

```bash
# GitHub shorthand (owner/repo/path)
npx companies.sh add paperclipai/companies/gstack

# GitHub tree URL
npx companies.sh add https://github.com/paperclipai/companies/tree/main/gstack

# Local path
npx companies.sh add ./my-company
```

### Options

| Option | Description |
|--------|-------------|
| `--target <mode>` | Import into a `new` or `existing` company |
| `-C, --company-id <id>` | Target company id for existing |
| `--include <values>` | Comma-separated: `company,agents,projects,tasks,skills` |
| `--agents <list>` | Comma-separated agent slugs, or `all` |
| `--collision <mode>` | Collision strategy: `rename`, `skip`, or `replace` |
| `--dry-run` | Preview the import without applying |
| `-y, --yes` | Skip interactive prompts |
| `-p, --provider <provider>` | Destination orchestrator (default: `paperclip`) |

### Company Package Format

An Agent Company is a portable, markdown-first package:

| File | Purpose |
|------|---------|
| `COMPANY.md` | Company metadata and configuration (YAML frontmatter) |
| `AGENTS.md` | Agent definitions, roles, and reporting structure |
| `PROJECT.md` | Project definitions and workspace bindings |
| `TASK.md` | Pre-loaded tasks and assignments |
| `SKILL.md` | Reusable skills available to agents |

### Connection Modes

| Mode | Behavior |
|------|----------|
| `auto` | Checks local Paperclip config, falls back to localhost, runs onboard if no config exists |
| `custom-url` | Skips local bootstrap; expects a reachable instance at `--api-base` |

### Provider Architecture

`companies.sh` is provider-based. Paperclip is the first supported provider, but the architecture accepts additional agent orchestrators. To add a provider, open a PR with the orchestrator name, documentation link, and CLI/SDK for the import flow.

## Paperclip Website

**Marketing site** for Paperclip, built with Astro.

### Purpose

- Project overview and feature descriptions
- Documentation links and quickstart guide
- Community links (Discord, GitHub)
- Landing page for new users

### Architecture

- **Astro** static site generator
- Deployed to Cloudflare Pages
- Source in `paperclip-website/`
- Content in `paperclip-website/src/` and `paperclip-website/doc/`

### Development

```bash
npm install
# Configure wrangler.jsonc for deployment
make deploy
```
