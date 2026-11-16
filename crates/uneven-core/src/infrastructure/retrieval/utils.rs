//! Vector math and context assembly utilities powered by `ragcraft`.

pub use ragcraft::{
    assemble_context, assemble_context_with_threshold, cosine_sim, dot_product, l2_norm,
    l2_normalize,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::retrieval::types::RetrievalResult;

    fn unit(len: usize, idx: usize) -> Vec<f32> {
        let mut v = vec![0.0f32; len];
        v[idx] = 1.0;
        v
    }

    #[test]
    fn cosine_sim_identical_unit_vectors_is_one() {
        let v = unit(4, 0);
        assert!((cosine_sim(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_sim_orthogonal_vectors_is_zero() {
        let a = unit(4, 0);
        let b = unit(4, 1);
        assert_eq!(cosine_sim(&a, &b), 0.0);
    }

    #[test]
    fn cosine_sim_zero_vector_is_zero() {
        let a = vec![1.0f32, 0.0, 0.0];
        let z = vec![0.0f32, 0.0, 0.0];
        assert_eq!(cosine_sim(&a, &z), 0.0);
    }

    #[test]
    fn cosine_sim_mismatched_lengths_is_zero() {
        let a = vec![1.0f32, 0.0, 0.0];
        let b = vec![1.0f32, 0.0];
        assert_eq!(cosine_sim(&a, &b), 0.0);
    }

    #[test]
    fn assemble_context_empty_results() {
        let result = RetrievalResult { documents: vec![], scores: vec![] };
        let ctx = assemble_context(&result);
        assert!(ctx.contains("No relevant documents found"));
    }

    #[test]
    fn assemble_context_formats_relevance_pct() {
        let result = RetrievalResult {
            documents: vec!["doc content".to_string()],
            scores: vec![0.75],
        };
        let ctx = assemble_context(&result);
        assert!(ctx.contains("75.00%"));
        assert!(ctx.contains("doc content"));
    }

    #[test]
    fn assemble_context_includes_all_documents() {
        let result = RetrievalResult {
            documents: vec!["alpha".into(), "beta".into(), "gamma".into()],
            scores:    vec![0.9, 0.7, 0.5],
        };
        let ctx = assemble_context(&result);
        assert!(ctx.contains("alpha"));
        assert!(ctx.contains("beta"));
        assert!(ctx.contains("gamma"));
        assert!(ctx.contains("90.00%"));
        assert!(ctx.contains("50.00%"));
    }

    #[test]
    fn assemble_context_has_separator_between_documents() {
        let result = RetrievalResult {
            documents: vec!["first".into(), "second".into()],
            scores:    vec![1.0, 0.8],
        };
        let ctx = assemble_context(&result);
        assert!(ctx.contains("---"));
    }

    #[test]
    fn assemble_context_starts_with_header() {
        let result = RetrievalResult { documents: vec![], scores: vec![] };
        assert!(assemble_context(&result).starts_with("## RELEVANT CONTEXT"));
    }

    #[test]
    fn cosine_sim_partial_overlap() {
        let a = [1.0f32, 1.0, 0.0, 0.0];
        let b = [1.0f32, 0.0, 0.0, 0.0];
        let sim = cosine_sim(&a, &b);
        assert!((sim - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5, "sim={sim}");
    }
}
