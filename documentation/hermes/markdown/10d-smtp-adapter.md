# Hermes Platform Adapters -- SMTP: Email

## Purpose

The email adapter (`_send_email`, lines 1046-1075) sends messages via SMTP using Python's built-in `smtplib`. It's a one-shot connection — no persistent IMAP listener — using STARTTLS for encryption. The adapter reads credentials from environment variables and config extras.

Source: `hermes-agent/tools/send_message_tool.py:1046-1075`

## Aha Moments

**Aha: Email uses a one-shot SMTP connection, not a persistent IMAP listener.** Unlike platforms that poll for incoming messages (IMAP, webhooks), the send path simply opens an SMTP connection, sends the message, and closes it. Incoming email is handled separately by the gateway's IMAP listener.

**Aha: STARTTLS with the default SSL context.** The adapter uses `ssl.create_default_context()` which verifies the server's certificate chain. This means self-signed mail server certificates will fail unless added to the system trust store.

**Aha: Subject is always "Hermes Agent".** The subject line is hardcoded, not configurable per-message. This keeps the sender identifiable and prevents the agent from spoofing arbitrary subjects.

## Architecture

```mermaid
sequenceDiagram
    participant Agent as Hermes Agent
    adapter as _send_email()
    SMTP as SMTP Server

    Agent->>adapter: chat_id, message
    adapter->>adapter: Read EMAIL_ADDRESS,<br/>EMAIL_PASSWORD, EMAIL_SMTP_HOST

    adapter->>SMTP: Connect (host:port)
    SMTP-->>adapter: 220 Ready
    adapter->>SMTP: STARTTLS
    SMTP-->>adapter: 220 Ready (encrypted)
    adapter->>SMTP: EHLO (after TLS)
    SMTP-->>adapter: 250 OK
    adapter->>SMTP: AUTH LOGIN
    SMTP-->>adapter: 235 Authenticated
    adapter->>SMTP: MAIL FROM (EMAIL_ADDRESS)
    SMTP-->>adapter: 250 OK
    adapter->>SMTP: RCPT TO (chat_id)
    SMTP-->>adapter: 250 OK
    adapter->>SMTP: DATA (MIME message)
    SMTP-->>adapter: 250 Accepted
    adapter->>SMTP: QUIT
    SMTP-->>adapter: 221 Bye
    adapter-->>Agent: {success: true}
```

## Implementation

```python
async def _send_email(extra, chat_id, message):
    import smtplib
    from email.mime.text import MIMEText

    # Configuration: env vars or config extras
    address = extra.get("address") or os.getenv("EMAIL_ADDRESS", "")
    password = os.getenv("EMAIL_PASSWORD", "")
    smtp_host = extra.get("smtp_host") or os.getenv("EMAIL_SMTP_HOST", "")
    try:
        smtp_port = int(os.getenv("EMAIL_SMTP_PORT", "587"))
    except (ValueError, TypeError):
        smtp_port = 587

    if not all([address, password, smtp_host]):
        return {"error": "Email not configured (EMAIL_ADDRESS, EMAIL_PASSWORD, EMAIL_SMTP_HOST required)"}

    try:
        # Build MIME message
        msg = MIMEText(message, "plain", "utf-8")
        msg["From"] = address
        msg["To"] = chat_id
        msg["Subject"] = "Hermes Agent"

        # Connect with STARTTLS
        server = smtplib.SMTP(smtp_host, smtp_port)
        server.starttls(context=ssl.create_default_context())
        server.login(address, password)
        server.send_message(msg)
        server.quit()

        return {"success": True, "platform": "email", "chat_id": chat_id}
    except Exception as e:
        return _error(f"Email send failed: {e}")
```

### MIME Message Construction

```python
msg = MIMEText(message, "plain", "utf-8")
msg["From"] = address        # Sender address (EMAIL_ADDRESS)
msg["To"] = chat_id          # Recipient address (the chat_id)
msg["Subject"] = "Hermes Agent"
```

The message is plain text with UTF-8 encoding. No HTML emails — the agent's markdown output is sent as-is.

### Configuration

```bash
# Environment variables
export EMAIL_ADDRESS="hermes@example.com"
export EMAIL_PASSWORD="your-app-password"    # App password, not account password
export EMAIL_SMTP_HOST="smtp.example.com"
export EMAIL_SMTP_PORT="587"                 # Default: 587 (STARTTLS)
```

```yaml
# config.yaml (alternative)
platforms:
  - name: email
    enabled: true
    extra:
      address: "hermes@example.com"
      smtp_host: "smtp.example.com"
```

### Port Selection

| Port | Protocol | Usage |
|------|----------|-------|
| 587 | STARTTLS | Default — encrypted after initial plaintext |
| 465 | SMTPS (implicit TLS) | Some providers (Gmail supports both) |
| 25 | Plain SMTP | Rarely used (often blocked by ISPs) |

The adapter defaults to 587. To use implicit TLS (port 465), the `starttls()` call would need to be replaced with `SMTP_SSL()`, which is not currently implemented.

## Building Your Own SMTP Adapter

```python
import smtplib
from email.mime.text import MIMEText

async def _send_email(smtp_host, smtp_port, address, password, to, subject, body):
    msg = MIMEText(body, "plain", "utf-8")
    msg["From"] = address
    msg["To"] = to
    msg["Subject"] = subject

    server = smtplib.SMTP(smtp_host, smtp_port)
    server.starttls(context=ssl.create_default_context())
    server.login(address, password)
    server.send_message(msg)
    server.quit()
```

For HTML emails, use `MIMEText(body, "html", "utf-8")` instead. For attachments, use `MIMEMultipart` with `MIMEBase` parts.

## Key Files

```
tools/
  └── send_message_tool.py   ← _send_email() (lines 1046-1075)
```

[Back to platform adapters overview → 10-platform-adapters.md](10-platform-adapters.md)
[See Matrix adapter → 10e-matrix-adapter.md](10e-matrix-adapter.md)
