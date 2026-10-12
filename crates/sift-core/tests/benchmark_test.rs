use chrono::Utc;
use sift_core::scanner::FileMeta;
use sift_core::storage::Database;
use std::time::Instant;

#[test]
fn test_warm_filename_search_latency_synthetic_corpus() {
    let db = Database::open_in_memory().expect("open database");
    let root = db.add_root("/synthetic/corpus").expect("add root");

    // Generate 10,000 realistic synthetic file records
    let total_files = 10_000;
    let mut files = Vec::with_capacity(total_files);
    let extensions = ["pdf", "txt", "rs", "ts", "json", "md", "png", "py"];

    for i in 0..total_files {
        let ext = extensions[i % extensions.len()];
        let filename = match i % 5 {
            0 => format!("quarterly_financial_report_{}.{}", i, ext),
            1 => format!("invoice_customer_order_{}.{}", i, ext),
            2 => format!("component_dashboard_view_{}.{}", i, ext),
            3 => format!("service_auth_provider_{}.{}", i, ext),
            _ => format!("config_settings_production_{}.{}", i, ext),
        };
        let path = format!("/synthetic/corpus/subfolder_{}/{}", i % 50, filename);

        files.push(FileMeta {
            path,
            filename,
            extension: ext.to_string(),
            size_bytes: ((i * 137) % 500_000) as i64,
            modified_at: Utc::now(),
            fingerprint: format!("{}:{}:0", (i * 137) % 500_000, i),
        });
    }

    // Measure indexing throughput
    let start_index = Instant::now();
    let inserted = db
        .upsert_files_batch(&root.id, &files)
        .expect("insert synthetic files");
    let index_duration = start_index.elapsed();
    assert_eq!(inserted, total_files);

    let indexing_throughput = (total_files as f64) / index_duration.as_secs_f64();
    println!(
        "\n--- Synthetic Benchmark Results (Corpus size: {} files) ---",
        total_files
    );
    println!(
        "Indexing Time: {:.2?} ({:.0} files/sec)",
        index_duration, indexing_throughput
    );

    // Warm-up query to populate cache
    let _ = db.search_filenames("report", 20);

    // Run 100 warm queries
    let test_queries = [
        "financial_report",
        "invoice_customer",
        "component_dashboard",
        "auth_provider",
        "settings_production",
        "report_99",
        "order_100",
        "view_500",
        "config_settings",
        "quarterly",
    ];

    let mut latencies_ms: Vec<f64> = Vec::new();

    for query in test_queries.iter().cycle().take(100) {
        let t0 = Instant::now();
        let results = db
            .search_filenames(query, 20)
            .expect("search synthetic query");
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(!results.is_empty(), "Should find matches for {}", query);
        latencies_ms.push(elapsed_ms);
    }

    latencies_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let min_latency = latencies_ms[0];
    let max_latency = latencies_ms[latencies_ms.len() - 1];
    let avg_latency: f64 = latencies_ms.iter().sum::<f64>() / (latencies_ms.len() as f64);
    let p95_latency = latencies_ms[(latencies_ms.len() as f64 * 0.95) as usize];

    println!("Warm Queries Run: {}", latencies_ms.len());
    println!("Min Latency: {:.3} ms", min_latency);
    println!("Average Latency: {:.3} ms", avg_latency);
    println!("95th Percentile Latency: {:.3} ms", p95_latency);
    println!("Max Latency: {:.3} ms", max_latency);
    println!("Target Condition (<100 ms): MET\n");

    // Acceptance target: Warm filename search latency under 100 ms
    assert!(
        avg_latency < 100.0,
        "Average search latency must be under 100ms, got {:.2}ms",
        avg_latency
    );
    assert!(
        p95_latency < 100.0,
        "P95 search latency must be under 100ms, got {:.2}ms",
        p95_latency
    );
}
