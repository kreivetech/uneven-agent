---
name: dioxus-app-patterns
description: >-
  Use this skill when building, configuring, or debugging Dioxus 0.7 Desktop,
  Web, or Fullstack applications (Dioxus.toml icons, OnceLock DB pools,
  Tailwind via dx, #[server] functions, and Dioxus 0.7 APIs).
---

# Dioxus 0.7 Application Patterns

Project-agnostic conventions for Dioxus 0.7 Desktop, Web, and Fullstack applications.

## 1. Icons & `Dioxus.toml` Configuration

- **Bundle icons (`Dioxus.toml`):** Declare standard bundle icons under `[bundle]` for Desktop/packaged builds:

```toml
[bundle]
icon = [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/256x256.png",
    "icons/icon.ico",
]
```

- **Desktop native window icon (`main.rs`):** Embed `icons/32x32.png` with `include_bytes!`, decode to RGBA via `image::load_from_memory(...).into_rgba8()`, and pass `Icon::from_rgba(...)` to `WindowBuilder::with_window_icon(Some(window_icon))`.
- **Web favicon:** Reference via `asset!("/assets/favicon.ico")` or `document::Link { rel: "icon", href: ... }`.

## 2. Database Pool Pattern (`OnceLock`)

- **Global static pool:** Store connection pools in `OnceLock` (`pub static DB: OnceLock<SqlitePool> = OnceLock::new();` or `PgPool`). In Fullstack apps, gate pool statics and DB modules with `#[cfg(feature = "server")]`.
- **Desktop initialization (`main.rs`):**
  1. Build a multi-thread Tokio runtime before `LaunchBuilder::desktop()`.
  2. Run `init_db().await` and `setup_db(&db).await` inside `rt.block_on(...)`.
  3. Store the pool via `DB.set(db)`.
  4. Provide it to the component tree in `App()` via `use_context_provider(|| DB.get().unwrap().clone())`.
- **Fullstack / Server initialization (`main.rs`):**
  1. Inside `serve(|| async move { ... })`, initialize each database pool (e.g. SQLite, PostgreSQL) independently.
  2. Run schema setup/DDL and store each pool in its respective `OnceLock`.
  3. Allow optional/secondary pools to log failures and degrade gracefully without crashing the primary server.

## 3. Tailwind, Server Functions & Validation

- **Tailwind CSS via `dx` only:** Never run `npm`, `npx`, or Node-based Tailwind CLIs. Dioxus compiles Tailwind automatically during `dx serve`, `dx check`, and `dx build`.
- **Fullstack server boundaries:** Use `#[server]` functions for frontend-consumed endpoints, and gate server-only imports/logic with `#[cfg(feature = "server")]` so WASM/client targets compile cleanly.
- **Form & input validation:** Use `catfive` on both client and server sides for input validation.

## 4. Dioxus 0.7 API Reference

Dioxus 0.7 removed legacy APIs (`cx`, `Scope`, `use_state`). For up-to-date RSX, components, `use_signal`, `use_memo`, `use_context`, `use_resource`, `Routable`, `#[post]`/`#[get]`, and hydration (`use_server_future`), see [dioxus_07_api.md](./references/dioxus_07_api.md).
