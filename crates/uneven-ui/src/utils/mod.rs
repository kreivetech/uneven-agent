pub mod diff;
pub mod env_manager;
pub mod formatters;
pub mod project;
pub mod window;

#[allow(unused_imports)]
pub use diff::{compute_patch_diff, DiffLine, DiffLineType, DiffStats};
#[allow(unused_imports)]
pub use env_manager::*;
#[allow(unused_imports)]
pub use formatters::*;
#[allow(unused_imports)]
pub use project::*;
#[allow(unused_imports)]
pub use window::*;

const ICON_LOGO_BYTES: &[u8] = include_bytes!("../../icons/128x128.png");

fn encode_base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Returns the embedded 128x128 Uneven crystal icon as an in-memory Base64 Data URI.
pub fn icon_logo_data_uri() -> &'static str {
    static DATA_URI: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    DATA_URI.get_or_init(|| format!("data:image/png;base64,{}", encode_base64(ICON_LOGO_BYTES)))
}



