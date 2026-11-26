---
name: readme-and-changelog
description: >-
  Use this skill when creating or updating README.md or CHANGELOG.md files in
  any repository to enforce clean structure, scannable sections, zero emojis,
  and accurate release notes.
---

# README & Changelog Authoring

Standards for writing clear, scannable `README.md` and `CHANGELOG.md` files across any project.

## 1. General Rules

- **Language:** Write technical `README.md` and `CHANGELOG.md` files in **English** (unless the project explicitly requires another language).
- **Clean formatting:** Zero decorative emojis, avoid em-dash (`—`) overuse, and avoid filler or redundant descriptions.
- **No sensitive leakage:** Never include credentials, `.env` values, personal local filesystem paths, or private internal files in public documentation.

## 2. `README.md` Structure

1. **Header:** Optional centered banner image (with descriptive `alt`), clear project title, and concise badges (version, license, platforms) when relevant.
2. **Executive Summary (1 paragraph or blockquote):** State what the project does, who it is for, and the core problem it solves right at the top.
3. **Core Capabilities / Key Features:** Short bullet list (`**Title**: concrete benefit`).
4. **Installation & Quick Start:** Copy-pasteable commands and a link to Releases when prebuilt binaries/installers exist.
5. **Architecture (when applicable):** Clean `text` directory tree with short inline comments explaining each module's responsibility.

## 3. `CHANGELOG.md` Standards

- Follow **Keep a Changelog** structure with `[Unreleased]` at the top and versioned sections (`## [X.Y.Z] - YYYY-MM-DD`).
- Group entries under standard categories: `Added`, `Changed`, `Fixed`, `Removed`, and `Security`.
- Derive entries strictly from verified commits, diffs, or release tags—never fabricate changes or include agent co-author notes.
- Keep each entry concise, user/developer-facing, and focused on observable behavior or fixes.
