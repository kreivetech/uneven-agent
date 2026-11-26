---
name: clean-code-engineering
description: >-
  Use this skill when writing, refactoring, modularizing, or reviewing code in
  any project to enforce Clean Code, safe reuse via cp, anti-drift, error
  sanitization, and production reliability.
---

# Clean Code & Engineering Standards

Language-agnostic and Rust engineering standards for safe, modular, maintainable codebases.

## 1. Language & Anti-Drift

- **English in codebase:** All source code, identifiers, comments, docstrings, file paths, and commit messages must be written strictly in **English**. Match the user's language in chat explanations.
- **Zero-guessing:** Read target files before editing. Never assume or fabricate signatures, imports, or contents. Make minimal, surgical edits.

## 2. Modularity, Reuse (`cp`) & Preservation

- **Small, modular units:** Keep functions single-purpose with shallow nesting (cyclomatic complexity `<= 10`). Avoid monolithic single-file modules.
- **Reuse and modularize via `cp` (never rewrite from scratch):**
  - When creating a module/endpoint that mirrors an existing one, copy (`cp`) the existing file and adapt its exports, types, and handlers cleanly instead of rewriting from scratch.
  - When modularizing monolithic code, copy (`cp`) the source file into the new module/files, trim what does not belong, and adjust exports/imports cleanly to avoid rewrites and regressions.
- **DRY without speculative abstractions:** Extract shared helpers when logic repeats, and avoid unnecessary `if`s or over-engineered abstractions.
- **Preserve existing features:** Never delete, disable, or break existing features, endpoints, or database queries without explicit notice and prior user approval.

## 3. Reliability, Error Sanitization & Rust Safety

- **Error sanitization:** Never expose raw stack traces, internal filesystem paths, database schemas, or provider diagnostics to end users; route errors through the project's sanitizer and canonical error codes when available.
- **Production safety:** Validate external inputs, never hardcode credentials/tokens, enforce explicit timeouts on network/DB calls, and avoid blocking I/O inside async runtimes.
- **Rust production paths:** Avoid unchecked `unwrap()`, `expect()`, or `panic!()` in production code.
- **Host safety:** Avoid triggering redundant parallel builds or test runs (`cargo check`, `cargo build`, etc.) when another build is already active.
