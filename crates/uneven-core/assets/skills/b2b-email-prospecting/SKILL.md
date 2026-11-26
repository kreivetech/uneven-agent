---
name: b2b-email-prospecting
description: >-
  Use this skill when researching B2B leads, drafting cold outreach emails to
  company decision-makers, or sending outreach emails via SMTP using .env
  credentials.
---

# B2B Email Prospecting

Standards for targeted B2B outreach, natural human copy, and safe domain sending cadence.

## Targeting & Verification Rules

1. **Pain-driven mapping (no spray-and-pray):** Only reach out after identifying companies with a realistic, accessible problem that the project's offering solves directly.
2. **Mandatory official website verification (beyond the home page):** Before finalizing or suggesting a commercial email address, inspect the company's official website directly (do not rely solely on search snippets or third-party directories). Do not blindly trust the home page alone: also check internal pages such as **Contact**, **About**, **Team**, **Press**, or equivalents to confirm the real address and select the most appropriate channel.
3. **Target decision-makers:** Address emails to **management, directors, or operations leads** (those who decide what the company adopts), avoiding strictly technical silos or generic support/compliance inboxes unless the scope requires it.
4. **Frictionless framing:** Lead with practical business gains (security, compliance, speed, team autonomy). Avoid statements that create internal resistance (e.g., claiming the company "does not need to hire" a given role).

## Copy & Formatting Rules

- **Tone:** Executive, clear, direct, and business-focused.
- **Zero jargon, code, or internal artifacts:** Never mention code snippets, internal file names or extensions (e.g., `.spec`, `.json`, `.rs`), library/framework names, inline code backticks (`` `...` ``), or engineering clichés (e.g., "surgical", "runtime", "stack").
- **No self-explanations or defensive justifications:** Never explain implementation details or rationale behind the scenes (e.g., "I built this because...", "we use X to ensure Y"). State only the practical benefit and business outcome directly and confidently.
- **Value-first positioning (no self-deprecation):** Lead with the practical solution and delivery. Do not mention in first contact that the operation is "new" or "early-stage"; address company stage or track record only if asked directly.
- **Natural writing style (anti-AI):** Never overuse em-dashes (`—`) in the body; prefer commas, periods, or parentheses.
- **Clean format:** Plain text, **no attachments** in the first email (to avoid corporate gateway blocks).
- **Domain safety:** Never send mass blasts; maintain a controlled, personalized cadence.

## Base Email Structure

Consult the current project's `AGENTS.md` for product details, value proposition, link, and standard signature. Match the target prospect's language.

```text
Subject: [Direct benefit or avoided risk for the Company]

Hi [Name],

[Short opening connected to a practical business pain]

[Objective explanation of the solution in plain business language, zero technical jargon]

[Plain-text reference to the official website or page, when applicable]

[Light CTA for a quick 15-minute conversation]

[Standard signature defined in the project]
```

## Terminal Sending (SMTP)

Credentials must live exclusively in the project's `.env` file (`SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_FROM_NAME`, `SMTP_PASS`). Never hardcode passwords or fixed signatures in this skill.

For the base SMTP sending command, see [smtp.md](./references/smtp.md).
