use super::types::RetrievalResult;

pub fn assemble_context(results: &RetrievalResult) -> String {
    let mut ctx = String::from("## RELEVANT CONTEXT\n\n");
    let mut added = 0;
    for (doc, score) in results.documents.iter().zip(&results.scores) {
        if *score >= 0.40 {
            ctx.push_str(&format!("**Relevance: {:.2}%**\n\n", score * 100.0));
            ctx.push_str(doc);
            ctx.push_str("\n\n---\n\n");
            added += 1;
        }
    }
    if added == 0 {
        ctx.push_str("(No relevant documents found)\n");
    }
    ctx
}

/// Dot product of two L2-normalised vectors = cosine similarity.
#[inline]
pub fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // ── cosine_sim: non-trivial values ────────────────────────────────────────

    #[test]
    fn cosine_sim_partial_overlap() {
        let a = [1.0f32, 1.0, 0.0, 0.0];
        let b = [1.0f32, 0.0, 0.0, 0.0];
        // Both L2-normalised: a = [0.707, 0.707, 0, 0], b = [1, 0, 0, 0]
        // cos(θ) = 0.707 * 1 + 0.707 * 0 = 0.707
        let a_norm: Vec<f32> = { let n = (a.iter().map(|x| x*x).sum::<f32>()).sqrt(); a.iter().map(|x| x/n).collect() };
        let b_norm: Vec<f32> = { let n = (b.iter().map(|x| x*x).sum::<f32>()).sqrt(); b.iter().map(|x| x/n).collect() };
        let sim = cosine_sim(&a_norm, &b_norm);
        assert!((sim - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5, "sim={sim}");
    }
}
