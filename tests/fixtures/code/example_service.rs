pub struct MetricsAggregator {
    pub total_operations: u64,
    pub latency_histogram: Vec<f64>,
}

impl MetricsAggregator {
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            latency_histogram: Vec::new(),
        }
    }

    pub fn record_latency(&mut self, duration_ms: f64) {
        self.total_operations += 1;
        self.latency_histogram.push(duration_ms);
    }
}
