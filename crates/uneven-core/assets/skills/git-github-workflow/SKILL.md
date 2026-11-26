---
name: git-github-workflow
description: >-
  Use this skill when creating branches, staging and committing changes,
  auditing repositories for sensitive files, or configuring GitHub Actions
  release workflows.
---

# Git & GitHub Workflow

Standards for clean branching, atomic commits, secret hygiene, and multi-platform GitHub Actions releases.

## 1. Branches & Commits

- **Always use dedicated branches:** Create a specific branch for each feature, fix, or refactor (`feat/...`, `fix/...`, `refactor/...`) instead of committing directly to `main`, unless explicitly instructed otherwise.
- **Small, focused commits:** Never commit many unrelated files at once. Stage and commit only the files belonging to a single logical change.
- **Descriptive commit messages (no agent co-author):** Write clear, descriptive commit messages in English (e.g., `feat(auth): add session timeout guard`). **Never** add `Co-authored-by:` trailers referencing the AI/agent.

## 2. Secret & Environment Hygiene

- **Never commit secrets or local environments:** Never add `.env`, `.env.*`, `venv/`, `.venv/`, `node_modules/`, private keys, API tokens, or sensitive user data to Git—especially in public repositories.
- **Pre-commit check:** Inspect `.gitignore`, `git status`, and `git diff --staged` before every commit to confirm no credentials, local paths, or temporary artifacts are staged.

## 3. GitHub Actions CI / Release Workflows

For multi-platform Rust/Dioxus release workflow patterns (tag triggers, OS matrix, `rust-cache`, `dioxus-cli`, and secret-backed release publishing), see [github_actions_release.md](./references/github_actions_release.md).
