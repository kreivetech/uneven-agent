pub mod nuxt;
pub mod express;
pub mod actixweb;
pub mod django;
pub mod nextjs;
pub mod nestjs;
pub mod fastapi;
pub mod laravel;
pub mod springboot;
pub mod rails;
pub mod gin;
pub mod axum;

use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn find_framework_fix(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    if let Some(suggestion) = nuxt::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = express::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = actixweb::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = django::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = nextjs::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = nestjs::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = fastapi::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = laravel::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = springboot::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = rails::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = gin::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = axum::suggest(error, content) {
        return Some(suggestion);
    }
    None
}
