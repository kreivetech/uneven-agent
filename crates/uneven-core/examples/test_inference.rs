use uneven_core::infrastructure::llm::inference::{load_model, run_inference};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading smollm2-135m-q4 with 1 thread...");
    let start = Instant::now();
    load_model("smollm2-135m-q4", 1).await?;
    println!("Model loaded in {:.2?}", start.elapsed());

    let long_system = "You are a helpful and concise developer assistant. You always answer precisely. ".repeat(20);
    let prompt = format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\nola<|im_end|>\n<|im_start|>assistant\n", long_system);

    let chunk_sizes = vec![16, 32, 64, 128];
    for chunk_size in chunk_sizes {
        println!("\n--- Testing with chunk size = {} ---", chunk_size);
        std::env::set_var("UNEVEN_TEST_CHUNK_SIZE", chunk_size.to_string());
        
        let start = Instant::now();
        // Generate only 1 token to isolate the prefill processing time
        let (res, tokens) = run_inference(&prompt, 1, 0.3, None, 1.1, 2048, None).await?;
        let elapsed = start.elapsed();
        println!("Prefill completed in {:.2?}. Tokens generated: {}, Response: {:?}", elapsed, tokens, res);
    }

    Ok(())
}
