use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use uneven_core::infrastructure::reporting::generate_multi_dashboard;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut datasets = HashMap::new();

    let monthly_rows: Vec<serde_json::Value> = vec![
        json!({"month": "2026-01", "revenue": 124000.0, "orders": 410, "avg_ticket": 302.4, "margin": 38400.0}),
        json!({"month": "2026-02", "revenue": 138500.0, "orders": 455, "avg_ticket": 304.4, "margin": 42900.0}),
        json!({"month": "2026-03", "revenue": 152000.0, "orders": 490, "avg_ticket": 310.2, "margin": 47100.0}),
        json!({"month": "2026-04", "revenue": 149200.0, "orders": 472, "avg_ticket": 316.1, "margin": 46200.0}),
        json!({"month": "2026-05", "revenue": 176800.0, "orders": 560, "avg_ticket": 315.7, "margin": 54800.0}),
        json!({"month": "2026-06", "revenue": 194300.0, "orders": 618, "avg_ticket": 314.4, "margin": 60200.0}),
    ];

    let category_rows: Vec<serde_json::Value> = (1..=14)
        .map(|i| {
            json!({
                "category": format!("Segment {}", i),
                "revenue": (15 - i) as f64 * 12500.0,
                "orders": i * 42
            })
        })
        .collect();

    datasets.insert("monthly_trend".to_string(), monthly_rows);
    datasets.insert("by_segment".to_string(), category_rows);

    let layout_spec = json!({
        "layout": [
            { "type": "kpi", "dataset": "monthly_trend", "metric": "revenue", "label": "Total Revenue", "icon": "dollar-sign" },
            { "type": "kpi", "dataset": "monthly_trend", "metric": "orders", "label": "Total Orders", "icon": "shopping-bag" },
            { "type": "kpi", "dataset": "monthly_trend", "metric": "margin", "label": "Gross Margin", "icon": "trending-up" },
            { "type": "kpi", "dataset": "by_segment", "metric": "revenue", "label": "Segment Volume", "icon": "layers" },
            { "type": "kpi", "dataset": "by_segment", "metric": "orders", "label": "Segment Orders", "icon": "activity" },
            { "type": "chart", "dataset": "by_segment", "chartType": "bar", "x": "category", "y": "revenue", "title": "Revenue by Segment", "subtitle": "Top 8 + Others" },
            { "type": "chart", "dataset": "monthly_trend", "chartType": "line", "x": "month", "y": "revenue", "title": "Monthly Revenue Trend", "subtitle": "2026 H1 progression" },
            { "type": "chart", "dataset": "by_segment", "chartType": "donut", "x": "category", "y": "revenue", "title": "Segment Share (Top 6)", "subtitle": "Proportional contribution" },
            { "type": "chart", "dataset": "monthly_trend", "chartType": "combo", "x": "month", "y": ["revenue", "orders"], "title": "Revenue vs Orders (Dual Y)", "subtitle": "Auto-paginated to Analytics Page 2" }
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
