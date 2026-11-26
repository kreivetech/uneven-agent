# GitHub Actions Multi-Platform Release Pattern

Reusable pattern for Rust and Dioxus desktop/CLI release workflows.

## Triggers, Matrix & Timeouts

- Trigger releases on version tags: `on: push: tags: ['v*']`.
- Set explicit `timeout-minutes` (e.g., `60`) on build jobs.
- Use `strategy: fail-fast: false` across OS targets so one platform failure does not cancel others:
  - `ubuntu-24.04` (`x86_64-unknown-linux-gnu`, and `aarch64-unknown-linux-gnu` via cross-compile if needed)
  - `windows-2022` (`x86_64-pc-windows-msvc`)
  - `macos-latest` (`aarch64-apple-darwin`)

## Standard Tooling Actions

- **Rust toolchain:** `dtolnay/rust-toolchain@stable` with explicit `targets: ${{ matrix.target }}`.
- **Cargo caching:** `Swatinem/rust-cache@v2` keyed by platform/target.
- **Dioxus CLI (desktop bundles):** `taiki-e/install-action@v2` with `tool: dioxus-cli@<version>`.
- **Linux desktop build deps:** Install `libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `lld`, and `patchelf` on `runner.os == 'Linux'`.

## Packaging & Secrets

- Package desktop targets with `dx bundle -r` (`--linux --package-types deb`, `--windows`, `--macos`).
- Store all publishing tokens in **GitHub Secrets** (e.g., `RELEASES_REPO_TOKEN` for publishing to a target releases repository); never hardcode credentials in workflow files.
