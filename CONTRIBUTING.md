# Contributing to Uneven Agent

Thank you for your interest in contributing to **Uneven Agent**!

## Project Structure

This repository is a Cargo workspace (`edition = "2024"`) organized into three crates:

- `crates/uneven-core` — Core agent engine, indexing, security auditing, diff engine, and analysis primitives.
- `crates/uneven-cli` — Command-line interface (`uneven`).
- `crates/uneven-ui` — Desktop IDE interface.

## Getting Started

1. Fork the repository and clone your fork:
   ```bash
   git clone https://github.com/<your-username>/uneven-agent.git
   cd uneven-agent
   ```
2. Create a feature or fix branch from `main`:
   ```bash
   git checkout -b feat/your-change
   ```
3. Build and run checks locally using Rust (stable toolchain):
   ```bash
   cargo check --workspace
   cargo test --workspace
   ```

## Pull Request Guidelines

- Keep PRs focused on a single bug fix, feature, or documentation improvement.
- Ensure `cargo check --workspace` and `cargo test --workspace` pass before opening a PR.
- Follow existing code style and preserve unrelated comments/docstrings.
- Use clear, descriptive commit messages (e.g., `fix(cli): ...`, `feat(core): ...`, `docs: ...`).
- Do not include local environment files (`.env`), build artifacts, or API keys.

## Reporting Bugs & Feature Requests

- Open an [Issue](https://github.com/kreivetech/uneven-agent/issues) with clear reproduction steps, environment details (OS, Rust version), and expected vs. actual behavior.
- For security vulnerabilities, **do not** open a public issue — see [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions will be licensed under the repository's **Business Source License 1.1 (BSL-1.1)** — see [LICENSE](LICENSE) for full terms.
