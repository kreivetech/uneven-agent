use crate::components::AppLayout;
use crate::pages::{CockpitPage, HistoryPage, SettingsPage};
use dioxus::prelude::*;

#[derive(Debug, Clone, Routable, PartialEq)]
#[allow(clippy::enum_variant_names)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppLayout)]
    #[route("/")]
    CockpitPage {},
    #[route("/history")]
    HistoryPage {},
    #[route("/settings")]
    SettingsPage {},
}
