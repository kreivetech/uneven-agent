//! Resource Guardian CLI Adapter
//!
//! Re-exports the unified multiplatform ResourceGuardian from uneven-core.

#[allow(unused_imports)]
pub use uneven_core::infrastructure::security::guardian::{
    get_igpu_frequency_mhz, get_memory_info, GuardianConfig, GuardianIncident, ResourceGuardian,
};
