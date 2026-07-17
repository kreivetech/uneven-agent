pub mod typescript;
pub mod detector;
pub mod javascript;
pub mod rust;
pub mod python;
pub mod go;
pub mod java;
pub mod php;
pub mod ruby;
pub mod cobol;
pub mod assembly;
pub mod solidity;
pub mod markup;
pub mod node;
pub mod linter;
pub mod runtime;
pub mod legacy;
pub mod frameworks;
pub mod proactive;

use crate::infrastructure::watcher::ErrorEvent;
use crate::infrastructure::agent::FixSuggestion;

pub fn find_heuristic_fix(error: &ErrorEvent, content: &str) -> Option<FixSuggestion> {
    if let Some(suggestion) = frameworks::find_framework_fix(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = javascript::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = cobol::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = assembly::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = solidity::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = typescript::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = rust::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = go::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = java::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = php::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = ruby::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = python::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = node::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = markup::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = legacy::suggest(error, content) {
        return Some(suggestion);
    }
    if let Some(suggestion) = runtime::suggest(error) {
        return Some(suggestion);
    }
    if let Some(suggestion) = linter::suggest(error) {
        return Some(suggestion);
    }

    None
}
