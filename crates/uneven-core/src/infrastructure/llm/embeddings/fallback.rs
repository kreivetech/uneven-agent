use crate::UnevenError;
pub const EMBEDDING_DIM: usize = 384;

pub fn hash_embed(text: &str) -> Vec<f32> {
    let mut embedding = vec![0.0f32; EMBEDDING_DIM];
    let bytes = text.as_bytes();

    for window in bytes.windows(3) {
        let h = fnv3(window[0], window[1], window[2]);
        let idx = (h as usize) % EMBEDDING_DIM;
        embedding[idx] += 1.0;
    }

    for word in text.split_whitespace() {
        let h = word.bytes().fold(2166136261u32, |acc, b| {
            (acc ^ b as u32).wrapping_mul(16777619)
        });
        let idx = (h as usize) % EMBEDDING_DIM;
        embedding[idx] += 2.0;
    }

    for (i, &b) in bytes.iter().enumerate() {
        let pos_hash = (b as u32).wrapping_mul(2654435769).wrapping_add(i as u32);
        let idx = (pos_hash as usize) % EMBEDDING_DIM;
        embedding[idx] += 0.5;
    }

    let _ = normalize_l2(&mut embedding);
    embedding
}

#[inline]
fn fnv3(a: u8, b: u8, c: u8) -> u32 {
    let h = 2166136261u32;
    let h = h.wrapping_mul(16777619) ^ a as u32;
    let h = h.wrapping_mul(16777619) ^ b as u32;
    h.wrapping_mul(16777619) ^ c as u32
}

pub fn normalize_l2(vec: &mut [f32]) -> Result<(), UnevenError> {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return Err(UnevenError::LlmError("Cannot normalize zero vector".to_string()));
    }
    for x in vec.iter_mut() {
        *x /= norm;
    }
    Ok(())
}
