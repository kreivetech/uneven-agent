use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use uneven_core::infrastructure::reporting::generate_multi_dashboard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut datasets = HashMap::new();

    let monthly_rows: Vec<serde_json::Value> = vec![
        json!({"month": "2026-01", "revenue": 124000.0, "orders": 410}),
        json!({"month": "2026-02", "revenue": 138500.0, "orders": 455}),
        json!({"month": "2026-03", "revenue": 152000.0, "orders": 490}),
        json!({"month": "2026-04", "revenue": 149200.0, "orders": 472}),
        json!({"month": "2026-05", "revenue": 176800.0, "orders": 560}),
        json!({"month": "2026-06", "revenue": 194300.0, "orders": 618}),
    ];

    let category_rows: Vec<serde_json::Value> = (1..=14)
        .map(|i| {
            json!({
                "category": format!("Segment Category {}", i),
                "revenue": (15 - i) as f64 * 12500.0,
                "orders": i * 42
            })
        })
        .collect();

    datasets.insert("monthly_trend".to_string(), monthly_rows);
    datasets.insert("by_segment".to_string(), category_rows);

    let layout_spec = json!({
        "layout": [
            { "type": "kpi", "dataset": "monthly_trend", "metric": "revenue", "label": "Total Revenue", "span": 4, "icon": "lucide:dollar-sign" },
            { "type": "kpi", "dataset": "monthly_trend", "metric": "orders", "label": "Total Orders", "span": 4, "icon": "lucide:shopping-bag" },
            { "type": "explanation", "text": "Revenue grew 56.7% from Jan to Jun 2026. Top 6 segments concentrate most of the volume while smaller segments are grouped in Others.", "span": 4 },
            { "type": "chart", "dataset": "monthly_trend", "chartType": "combo", "x": "month", "y": ["revenue", "orders"], "title": "Revenue & Orders Trend", "subtitle": "Combo chart (column + line)", "span": 8, "height": 360 },
            { "type": "chart", "dataset": "by_segment", "chartType": "donut", "x": "category", "y": "revenue", "title": "Segment Share (Top 6)", "subtitle": "Proportional revenue contribution", "span": 4, "height": 360 },
            { "type": "chart", "dataset": "by_segment", "chartType": "bar", "x": "category", "y": "revenue", "title": "Revenue by Segment (14 categories)", "subtitle": "Auto-horizontal bar with dynamic height", "span": 7 },
            { "type": "chart", "dataset": "monthly_trend", "chartType": "area", "x": "month", "y": "revenue", "title": "Cumulative Momentum", "subtitle": "Gradient area curve", "span": 5, "height": 380 }
        ]
    });

    let html = generate_multi_dashboard(
        &datasets,
        "Executive BI Dashboard Preview",
        "dark",
        Some(&layout_spec),
    );

    let output_dir = PathBuf::from("target/reports");
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("dashboard_preview.html");
    std::fs::write(&output_path, &html)?;

    println!("Generated: {}", output_path.display());
    Ok(())
}
