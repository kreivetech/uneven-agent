/**
 * Simple example of uneven-core usage
 */

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Uneven Core Example");
    println!("==================\n");

    // Create configuration
    let config = uneven_core::UnevenConfig {
        project_root: ".".to_string(),
        brain: uneven_core::BrainConfig {
            provider: "local".to_string(),
            model: "llama3.2".to_string(),
            temperature: 0.3,
            max_tokens: 2048,
            ..Default::default()
        },
        knowledge: Some(uneven_core::KnowledgeConfig {
            dirs: Some(vec!["./src".to_string()]),
            files: Some(vec!["./README.md".to_string()]),
            urls: Some(vec![]),
            ..Default::default()
        }),
        watch: Some(uneven_core::WatchConfig {
            terminal: Some("npm run dev".to_string()),
            auto_fix: Some(true),
            confirm_before_fix: Some(false),
            dirs: Some(vec!["./src".to_string()]),
            ..Default::default()
        }),
        security: uneven_core::DataSecurityConfig::default(),
        ..Default::default()
    };

    println!("Configuration created:");
    println!("  Provider: {}", config.brain.provider);
    println!("  Model: {}", config.brain.model);
    println!(
        "  Knowledge dirs: {:?}",
        config.knowledge.as_ref().and_then(|k| k.dirs.as_ref())
    );
    println!(
        "  Watch command: {:?}",
        config.watch.as_ref().and_then(|w| w.terminal.as_ref())
    );

    Ok(())
}
