# Sending Email via SMTP

Credentials must be stored in the project directory's `.env` file:
- `SMTP_HOST`
- `SMTP_PORT`
- `SMTP_USER`
- `SMTP_FROM_NAME`
- `SMTP_PASS`

## Base Send Command

```bash
python3 -c '
import smtplib
from email.message import EmailMessage
from email.utils import formatdate

env = {}
with open(".env", "r", encoding="utf-8") as f:
    for line in f:
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, v = line.split("=", 1)
            env[k.strip()] = v.strip().strip("\"").strip("\x27")

msg = EmailMessage()
msg["Subject"] = "SUBJECT_HERE"
msg["From"] = f"{env[\"SMTP_FROM_NAME\"]} <{env[\"SMTP_USER\"]}>"
msg["To"] = "RECIPIENT_HERE"
msg["Date"] = formatdate(localtime=True)
msg.set_content("BODY_HERE")

with smtplib.SMTP(env["SMTP_HOST"], int(env["SMTP_PORT"]), timeout=20) as s:
    s.ehlo()
    s.starttls()
    s.ehlo()
    s.login(env["SMTP_USER"], env["SMTP_PASS"])
    s.send_message(msg)

print(f"Email sent to {msg[\"To\"]}!")
'
```
