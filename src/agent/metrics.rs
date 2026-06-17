//! Agent mode: lightweight, dependency-free metrics registry.
//!
//! Cumulative counters are stored as [`AtomicU64`]; point-in-time gauges
//! (active sessions, in-flight execs, total session disk) are sampled from
//! live state at scrape time and passed into the render functions. Keeping
//! gauges out of the counter set means there is a single source of truth and
//! no drift between a mirrored counter and the thing it tracks.
//!
//! Output is hand-rendered: Prometheus text exposition format (the scrape
//! default) or flat JSON (`?format=json`). No `prometheus`/`metrics` crate —
//! this matches the rest of the agent layer, which hand-rolls its primitives.

use std::sync::atomic::{AtomicU64, Ordering};

/// All metric values use `Relaxed` ordering: each counter is independent and
/// we need atomicity of the increment, not ordering relative to other counters
/// or to surrounding program state. A scrape may observe a set of counters that
/// were never simultaneously "true", which is fine for monitoring.
const ORDER: Ordering = Ordering::Relaxed;

/// Cumulative, monotonic counters incremented at request choke points.
#[derive(Default)]
pub struct Metrics {
    exec_success: AtomicU64,
    exec_error: AtomicU64,
    exec_timeout: AtomicU64,
    /// Sum of execution wall-clock durations (ms) across all terminal outcomes.
    /// Paired with the derived exec count to form a Prometheus summary.
    exec_duration_ms_sum: AtomicU64,
    output_truncated: AtomicU64,
    sessions_created: AtomicU64,
    rejected_concurrency: AtomicU64,
    rejected_payload: AtomicU64,
    rejected_unauthorized: AtomicU64,
    /// Requests/execs rejected by a per-tenant rate cap (session count,
    /// concurrent exec, or requests/min).
    rejected_rate: AtomicU64,
}

/// Point-in-time gauge values, sampled from live state at scrape time rather
/// than mirrored into counters.
pub struct Gauges {
    pub sessions_active: u64,
    pub sessions_total: u64,
    pub exec_in_flight: u64,
    pub sessions_disk_bytes: u64,
}

/// Per-session resource row for the JSON metrics breakdown. Exposed in JSON
/// format only, and only in open (no-auth) mode — in auth mode per-session
/// rows would leak one tenant's footprint to another, so the scrape is capped
/// at global aggregates (see 0.20.5 Q2/Q3 in the implementation plan).
#[derive(serde::Serialize)]
pub struct SessionResourceRow {
    pub id: String,
    pub disk_bytes: u64,
    /// Configured memory ceiling in WASM pages (64 KiB each); `None` = unlimited.
    pub memory_cap_pages: Option<u32>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_exec_success(&self, dur_ms: u64) {
        self.exec_success.fetch_add(1, ORDER);
        self.exec_duration_ms_sum.fetch_add(dur_ms, ORDER);
    }

    pub fn record_exec_error(&self, dur_ms: u64) {
        self.exec_error.fetch_add(1, ORDER);
        self.exec_duration_ms_sum.fetch_add(dur_ms, ORDER);
    }

    pub fn record_exec_timeout(&self, dur_ms: u64) {
        self.exec_timeout.fetch_add(1, ORDER);
        self.exec_duration_ms_sum.fetch_add(dur_ms, ORDER);
    }

    pub fn record_output_truncated(&self) {
        self.output_truncated.fetch_add(1, ORDER);
    }

    pub fn record_session_created(&self) {
        self.sessions_created.fetch_add(1, ORDER);
    }

    pub fn record_rejected_concurrency(&self) {
        self.rejected_concurrency.fetch_add(1, ORDER);
    }

    pub fn record_rejected_payload(&self) {
        self.rejected_payload.fetch_add(1, ORDER);
    }

    pub fn record_rejected_unauthorized(&self) {
        self.rejected_unauthorized.fetch_add(1, ORDER);
    }

    pub fn record_rejected_rate(&self) {
        self.rejected_rate.fetch_add(1, ORDER);
    }

    /// Consistent-enough snapshot of all counters for rendering.
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            exec_success: self.exec_success.load(ORDER),
            exec_error: self.exec_error.load(ORDER),
            exec_timeout: self.exec_timeout.load(ORDER),
            exec_duration_ms_sum: self.exec_duration_ms_sum.load(ORDER),
            output_truncated: self.output_truncated.load(ORDER),
            sessions_created: self.sessions_created.load(ORDER),
            rejected_concurrency: self.rejected_concurrency.load(ORDER),
            rejected_payload: self.rejected_payload.load(ORDER),
            rejected_unauthorized: self.rejected_unauthorized.load(ORDER),
            rejected_rate: self.rejected_rate.load(ORDER),
        }
    }

    /// Render the Prometheus text exposition format (the scrape default).
    /// `per_session` rows are intentionally not emitted here — Prometheus
    /// labels on session ids would be unbounded-cardinality, and per-session
    /// data is withheld in auth mode anyway.
    pub fn render_prometheus(&self, g: &Gauges) -> String {
        let s = self.snapshot();
        let exec_count = s.exec_success + s.exec_error + s.exec_timeout;
        let mut out = String::with_capacity(2048);

        metric(
            &mut out,
            "wasmrun_agent_exec_total",
            "Total code executions by terminal result.",
            "counter",
            &[
                (&[("result", "success")], s.exec_success),
                (&[("result", "error")], s.exec_error),
                (&[("result", "timeout")], s.exec_timeout),
            ],
        );
        metric(
            &mut out,
            "wasmrun_agent_exec_duration_ms_sum",
            "Sum of execution wall-clock durations in milliseconds.",
            "counter",
            &[(&[], s.exec_duration_ms_sum)],
        );
        metric(
            &mut out,
            "wasmrun_agent_exec_duration_ms_count",
            "Count of executions contributing to the duration sum.",
            "counter",
            &[(&[], exec_count)],
        );
        metric(
            &mut out,
            "wasmrun_agent_output_truncated_total",
            "Executions whose captured output hit the output cap.",
            "counter",
            &[(&[], s.output_truncated)],
        );
        metric(
            &mut out,
            "wasmrun_agent_sessions_created_total",
            "Total sessions created since startup.",
            "counter",
            &[(&[], s.sessions_created)],
        );
        metric(
            &mut out,
            "wasmrun_agent_exec_rejected_total",
            "Executions/requests rejected before doing work, by reason.",
            "counter",
            &[
                (&[("reason", "concurrency")], s.rejected_concurrency),
                (&[("reason", "payload")], s.rejected_payload),
                (&[("reason", "unauthorized")], s.rejected_unauthorized),
                (&[("reason", "rate")], s.rejected_rate),
            ],
        );
        metric(
            &mut out,
            "wasmrun_agent_sessions_active",
            "Currently active (non-expired) sessions.",
            "gauge",
            &[(&[], g.sessions_active)],
        );
        metric(
            &mut out,
            "wasmrun_agent_sessions_total",
            "Total sessions tracked, including expired-but-not-cleaned.",
            "gauge",
            &[(&[], g.sessions_total)],
        );
        metric(
            &mut out,
            "wasmrun_agent_exec_in_flight",
            "Exec workers currently running.",
            "gauge",
            &[(&[], g.exec_in_flight)],
        );
        metric(
            &mut out,
            "wasmrun_agent_sessions_disk_bytes",
            "Total on-disk footprint across active sessions in bytes.",
            "gauge",
            &[(&[], g.sessions_disk_bytes)],
        );
        out
    }

    /// Render a flat JSON object. `per_session` is included only when the
    /// caller passes `Some` (open mode); in auth mode it is `None` so the
    /// scrape stays at global aggregates.
    pub fn render_json(
        &self,
        g: &Gauges,
        per_session: Option<Vec<SessionResourceRow>>,
    ) -> serde_json::Value {
        let s = self.snapshot();
        let exec_count = s.exec_success + s.exec_error + s.exec_timeout;
        let mut obj = serde_json::json!({
            "exec_total": {
                "success": s.exec_success,
                "error": s.exec_error,
                "timeout": s.exec_timeout,
            },
            "exec_duration_ms_sum": s.exec_duration_ms_sum,
            "exec_duration_ms_count": exec_count,
            "output_truncated_total": s.output_truncated,
            "sessions_created_total": s.sessions_created,
            "exec_rejected_total": {
                "concurrency": s.rejected_concurrency,
                "payload": s.rejected_payload,
                "unauthorized": s.rejected_unauthorized,
                "rate": s.rejected_rate,
            },
            "sessions_active": g.sessions_active,
            "sessions_total": g.sessions_total,
            "exec_in_flight": g.exec_in_flight,
            "sessions_disk_bytes": g.sessions_disk_bytes,
        });
        if let Some(rows) = per_session {
            obj["sessions"] = serde_json::to_value(rows).unwrap_or(serde_json::Value::Null);
        }
        obj
    }
}

struct Snapshot {
    exec_success: u64,
    exec_error: u64,
    exec_timeout: u64,
    exec_duration_ms_sum: u64,
    output_truncated: u64,
    sessions_created: u64,
    rejected_concurrency: u64,
    rejected_payload: u64,
    rejected_unauthorized: u64,
    rejected_rate: u64,
}

/// Append one Prometheus metric family: `# HELP`, `# TYPE`, then one sample
/// line per label-set. An empty label slice renders a bare metric line.
fn metric(
    out: &mut String,
    name: &str,
    help: &str,
    kind: &str,
    samples: &[(&[(&str, &str)], u64)],
) {
    out.push_str("# HELP ");
    out.push_str(name);
    out.push(' ');
    out.push_str(help);
    out.push('\n');
    out.push_str("# TYPE ");
    out.push_str(name);
    out.push(' ');
    out.push_str(kind);
    out.push('\n');
    for (labels, value) in samples {
        out.push_str(name);
        if !labels.is_empty() {
            out.push('{');
            for (i, (k, v)) in labels.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(k);
                out.push_str("=\"");
                out.push_str(v);
                out.push('"');
            }
            out.push('}');
        }
        out.push(' ');
        out.push_str(&value.to_string());
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gauges() -> Gauges {
        Gauges {
            sessions_active: 2,
            sessions_total: 3,
            exec_in_flight: 1,
            sessions_disk_bytes: 4096,
        }
    }

    #[test]
    fn counters_increment() {
        let m = Metrics::new();
        m.record_exec_success(10);
        m.record_exec_success(20);
        m.record_exec_error(5);
        m.record_exec_timeout(30);
        m.record_output_truncated();
        m.record_session_created();
        m.record_rejected_concurrency();
        m.record_rejected_payload();
        m.record_rejected_unauthorized();
        m.record_rejected_rate();
        m.record_rejected_rate();

        let s = m.snapshot();
        assert_eq!(s.exec_success, 2);
        assert_eq!(s.exec_error, 1);
        assert_eq!(s.exec_timeout, 1);
        assert_eq!(s.exec_duration_ms_sum, 65); // 10+20+5+30
        assert_eq!(s.output_truncated, 1);
        assert_eq!(s.sessions_created, 1);
        assert_eq!(s.rejected_concurrency, 1);
        assert_eq!(s.rejected_payload, 1);
        assert_eq!(s.rejected_unauthorized, 1);
        assert_eq!(s.rejected_rate, 2);

        // The `rate` reason surfaces in both exposition formats.
        let g = gauges();
        assert!(m
            .render_prometheus(&g)
            .contains("wasmrun_agent_exec_rejected_total{reason=\"rate\"} 2"));
        assert_eq!(m.render_json(&g, None)["exec_rejected_total"]["rate"], 2);
    }

    #[test]
    fn prometheus_render_is_well_formed() {
        let m = Metrics::new();
        m.record_exec_success(40);
        m.record_exec_error(10);
        let text = m.render_prometheus(&gauges());

        // Every metric family has a HELP and TYPE line.
        let help = text.matches("# HELP ").count();
        let typ = text.matches("# TYPE ").count();
        assert_eq!(help, typ);
        assert!(help >= 10);

        // Labeled and gauge samples present with expected values.
        assert!(text.contains("wasmrun_agent_exec_total{result=\"success\"} 1"));
        assert!(text.contains("wasmrun_agent_exec_total{result=\"error\"} 1"));
        assert!(text.contains("wasmrun_agent_exec_duration_ms_sum 50"));
        assert!(text.contains("wasmrun_agent_exec_duration_ms_count 2"));
        assert!(text.contains("wasmrun_agent_sessions_active 2"));
        assert!(text.contains("wasmrun_agent_exec_in_flight 1"));
        assert!(text.contains("wasmrun_agent_sessions_disk_bytes 4096"));

        // No sample line should be empty or malformed (each ends in a number).
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let last = line.rsplit(' ').next().unwrap();
            assert!(last.parse::<u64>().is_ok(), "bad sample line: {line}");
        }
    }

    #[test]
    fn json_render_shape() {
        let m = Metrics::new();
        m.record_exec_success(40);
        m.record_exec_timeout(60);
        let v = m.render_json(&gauges(), None);

        assert_eq!(v["exec_total"]["success"], 1);
        assert_eq!(v["exec_total"]["timeout"], 1);
        assert_eq!(v["exec_duration_ms_sum"], 100);
        assert_eq!(v["exec_duration_ms_count"], 2);
        assert_eq!(v["sessions_active"], 2);
        assert_eq!(v["sessions_disk_bytes"], 4096);
        // Per-session omitted when None (auth mode / aggregates only).
        assert!(v.get("sessions").is_none());
    }

    #[test]
    fn json_render_includes_per_session_when_present() {
        let m = Metrics::new();
        let rows = vec![SessionResourceRow {
            id: "abc".into(),
            disk_bytes: 1024,
            memory_cap_pages: Some(4096),
        }];
        let v = m.render_json(&gauges(), Some(rows));
        assert_eq!(v["sessions"][0]["id"], "abc");
        assert_eq!(v["sessions"][0]["disk_bytes"], 1024);
        assert_eq!(v["sessions"][0]["memory_cap_pages"], 4096);
    }
}
