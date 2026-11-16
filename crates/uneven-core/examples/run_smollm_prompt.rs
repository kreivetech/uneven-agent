use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use uneven_core::infrastructure::llm::inference::{load_model, run_inference_streaming};
use uneven_core::infrastructure::utils::hardware_detector::HardwareDetector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== UNEVEN REAL LOCAL MODEL INFERENCE TEST ===");

    let detector = HardwareDetector::new();
    if let Ok(info) = detector.detect_system() {
        println!(
            "Hardware: CPU cores {} | RAM {:.2} GB | GPU: {} (integrated: {}) | Weak: {}",
            info.cpu_cores,
            info.ram_total_mb as f64 / 1024.0,
            info.gpu.name.as_deref().unwrap_or("Unknown"),
            info.gpu.is_integrated,
            info.is_weak_system
        );
    }

    let model_name = "smollm2-135m-q4";
    let threads = std::env::var("UNEVEN_INFERENCE_THREADS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(2); // Default to 2 threads to keep at least 2 cores safe from watchdog killer
    println!("\n[1/3] Loading model '{}' (threads = {})...", model_name, threads);
    let start_load = Instant::now();
    load_model(model_name, threads).await?;
    println!("[1/3] Model loaded successfully in {:.2?}", start_load.elapsed());

    let prompt = std::env::args().nth(1).unwrap_or_else(|| {
        "<|im_start|>user\nquanto é 1 + 1 ? Responda direto e brevemente.<|im_end|>\n<|im_start|>assistant\n".to_string()
    });

    println!("\n[2/3] Prompt:\n{}", prompt.trim());
    println!("\n[3/3] Generating tokens...");

    let token_count = Arc::new(AtomicUsize::new(0));
    let last_token_time = Arc::new(std::sync::Mutex::new(Instant::now()));
    let token_count_cb = Arc::clone(&token_count);
    let last_time_cb = Arc::clone(&last_token_time);

    let start_gen = Instant::now();
    let (output, total_tokens) = run_inference_streaming(
        &prompt,
        64,     // max_tokens
        0.2,    // temperature (deterministic & focused)
        None,   // top_p
        1.1,    // repeat_penalty
        2048,   // max_context
        None,   // grammar
        move |token_piece| {
            let mut last = last_time_cb.lock().unwrap();
            let now = Instant::now();
            let step_ms = now.duration_since(*last).as_secs_f64() * 1000.0;
            *last = now;
            let idx = token_count_cb.fetch_add(1, Ordering::SeqCst) + 1;
            println!(
                "STREAM_TOKEN: index={} latency_ms={:.2} piece={:?}",
                idx, step_ms, token_piece
            );
        },
    )
    .await?;

    let total_elapsed = start_gen.elapsed();
    let tps = if total_elapsed.as_secs_f64() > 0.0 {
        total_tokens as f64 / total_elapsed.as_secs_f64()
    } else {
        0.0
    };

    println!("\n=== GENERATION COMPLETE ===");
    println!("Tokens generated: {}", total_tokens);
    println!("Total inference time: {:.2?}", total_elapsed);
    println!("Speed: {:.2} tokens/sec", tps);
    println!("\n=== ASSISTANT RESPONSE ===");
    println!("{}", output.trim());

    Ok(())
}
