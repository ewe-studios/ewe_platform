---
title: "Pi Extensions -- pi-web-access"
---

# pi-web-access

**Web search, content extraction, and video understanding for Pi.**

pi-web-access gives Pi the ability to search the web, extract content from URLs, and understand videos. Zero config with a supported Chromium-based browser, or bring your own API keys.

## Features

### Zero Config

Works out of the box with Exa MCP (no API key needed). Or sign into Google in Chrome, Arc, Helium, or Chromium for Gemini Web. Add API keys for Exa, Perplexity, or Gemini API for more control.

### Video Understanding

Point it at a YouTube video or local screen recording and ask questions about what's on screen. Full transcripts, visual descriptions, and frame extraction at exact timestamps.

### Content Extraction

Give it any URL and it extracts the content in a format the agent can understand -- articles, documentation, API references, error messages.

## Search Providers

| Provider | Config Required | Notes |
|----------|----------------|-------|
| Exa MCP | No API key | Works out of the box |
| Gemini Web | Google sign-in | Via Chrome/Arc/Helium |
| Exa API | API key | More control |
| Perplexity | API key | Alternative search |
| Gemini API | API key | Google's API |

## Tools

| Tool | Purpose |
|------|---------|
| Web search | Search the internet for information |
| Content extraction | Extract article content from URLs |
| Video understanding | Analyze YouTube videos and screen recordings |

## Installation

```bash
pi install npm:pi-web-access
```

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-web-access` |
| Platform | macOS, Linux, Windows* |
| Zero config | Yes (Exa MCP) |
| Video support | YouTube + screen recordings |
