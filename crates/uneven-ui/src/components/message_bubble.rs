use crate::components::MarkdownView;
use crate::state::{ChatMessage, MessageRole};
use dioxus::prelude::*;

#[component]
pub fn MessageBubble(message: ChatMessage) -> Element {
    let is_user = message.role == MessageRole::User;

    rsx! {
        div {
            class: "flex flex-col mb-4 [content-visibility:auto] [contain-intrinsic-size:0_80px]",
            class: if is_user { "items-end" } else { "items-start" },

            // Sender Label & Tag
            div { class: "flex items-center gap-2 mb-1 px-1 text-[11px] font-mono text-neutral-500 select-none",
                if is_user {
                    span { "You" }
                } else {
                    span { class: "text-neutral-400 font-bold", "◈" }
                    span { class: "font-medium text-neutral-300", "Uneven" }
                }
            }

            // Message Bubble Card
            div {
                class: if is_user {
                    "max-w-2xl rounded-2xl px-4 py-3 text-sm leading-relaxed bg-[#1c1c1c] border border-[#2c2c2c] text-neutral-100 shadow-sm"
                } else {
                    "w-full max-w-full rounded-2xl px-4 py-3 text-sm leading-relaxed bg-[#141414] border border-[#242424] text-neutral-200 shadow-sm"
                },

                // Attached Files Chips (if any)
                if !message.files.is_empty() {
                    div { class: "flex flex-wrap gap-1.5 mb-2.5 pb-2 border-b border-[#242424]",
                        for file in message.files.iter() {
                            span {
                                class: "inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-mono bg-[#0c0c0c] border border-[#222222] text-neutral-300",
                                span { "+" }
                                span { "{file}" }
                            }
                        }
                    }
                }

                // Message Text Content (rendered cleanly using pre-parsed blocks with 0 re-parsing on scroll!)
                MarkdownView { blocks: message.blocks }
            }
        }
    }
}

