# Dioxus 0.7 API Quick Reference

Dioxus 0.7 changes core APIs (`cx`, `Scope`, and `use_state` are removed).

## Launch & Root Component

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! { "Hello, Dioxus!" }
}
```

## RSX Syntax

```rust
rsx! {
    div {
        class: "container",
        color: "red",
        width: if condition { "100%" },
        "Hello, Dioxus!"
    }
    for i in 0..5 {
        div { "{i}" }
    }
    if condition {
        div { "Condition is true!" }
    }
    {children}
    {(0..5).map(|i| rsx! { span { "Item {i}" } })}
}
```

## Assets & Stylesheets

```rust
rsx! {
    document::Stylesheet { href: asset!("/assets/styles.css") }
    img { src: asset!("/assets/image.png"), alt: "An image" }
}
```

## Components & Props

- Annotate with `#[component]`; name must start with an uppercase letter or contain `_`.
- Props must be owned (`String`, `Vec<T>` instead of `&str`, `&[T]`) and implement `PartialEq + Clone`.
- Wrap reactive props in `ReadOnlySignal<T>` so memos/resources re-run on change.

## State (`use_signal`, `use_memo`, Context)

```rust
#[component]
fn Counter() -> Element {
    let mut count = use_signal(|| 0);
    let doubled = use_memo(move || count() * 2);

    rsx! {
        h1 { "Count: {count}" }
        h2 { "Doubled: {doubled}" }
        button { onclick: move |_| *count.write() += 1, "Increment" }
        button { onclick: move |_| count.with_mut(|c| *c += 1), "Mutate" }
    }
}
```

- Context sharing: parent calls `use_context_provider(|| signal);`, child calls `use_context::<Signal<T>>();`.

## Async (`use_resource`)

```rust
let data = use_resource(move || async move {
    // async request
});

match data() {
    Some(value) => rsx! { View { value } },
    None => rsx! { "Loading..." },
}
```

## Routing (`Routable`)

```rust
#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[layout(NavBar)]
        #[route("/")]
        Home {},
        #[route("/blog/:id")]
        BlogPost { id: i32 },
}

#[component]
fn NavBar() -> Element {
    rsx! {
        a { href: "/", "Home" }
        Outlet<Route> {}
    }
}

#[component]
fn App() -> Element {
    rsx! { Router::<Route> {} }
}
```

## Fullstack & Hydration

- Use `#[post("/api/...")]` / `#[get("/api/...")]` for server functions returning `Result<T, ServerFnError>`.
- Client and server initial renders must match for hydration:
  - Prefer `use_server_future` instead of `use_resource` for SSR-hydrated data.
  - Run browser-only APIs (e.g., `localStorage`) inside `use_effect` after hydration.
