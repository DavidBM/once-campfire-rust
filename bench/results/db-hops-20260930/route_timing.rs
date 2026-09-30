//! Times whole requests in process: the app booted over the `default` parity seed, its router
//! driven directly (no sockets, no front server) by C tasks per route for a few seconds, the
//! requests `bench/run` makes (as David, `Accept-Encoding: gzip`, the busy room, posts to HQ).
//! Reports req/s, p50/p99 latency, process CPU per request and the peak thread count (which counts
//! blocking-pool threads a warm-up left idle: tokio keeps them 10 s). Copied into
//! crates/campfire/src/timing.rs, with `#[cfg(test)] mod timing;` added to main.rs, to run on 4
//! CPUs (so 4 runtime workers, as bench/run gives the server):
//!
//!   CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
//!     cargo test --release -p campfire --no-run
//!   CAMPFIRE_REQUIRE_SEED=1 taskset -c 8-11 target/release/deps/campfire-<hash> timing::routes \
//!     --ignored --nocapture --test-threads 1
//!
//! `TIMING_ROUTES`, `TIMING_CONCS` (default 1,16,64) and `TIMING_SECS` (default 3) change the matrix.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use crate::controllers::presenters::test_support::{ALL_TALK, HQ, TestApp, david_cookie};

/// `messages.busy_060`, the `before` of bench/run's messages_page.
const BEFORE: i64 = 933434569;

const ROUTES: &[&str] = &["room_show", "messages_page", "sidebar", "search", "post_message"];

#[test]
#[ignore]
fn routes() {
    let routes: Vec<String> =
        std::env::var("TIMING_ROUTES").map_or_else(|_| ROUTES.iter().map(|r| r.to_string()).collect(), |r| r.split(',').map(str::to_string).collect());
    let concs: Vec<usize> =
        std::env::var("TIMING_CONCS").unwrap_or_else(|_| "1,16,64".into()).split(',').map(|c| c.parse().unwrap()).collect();
    let secs: f64 = std::env::var("TIMING_SECS").map_or(3.0, |s| s.parse().unwrap());
    let workers = std::thread::available_parallelism().unwrap().get();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let app = TestApp::boot().await.expect("the default seed");
        let router = app.booted.router.clone();
        let cookie = david_cookie();
        println!("workers {workers}, readers {}", app.booted.app.config.db_readers);
        for route in &routes {
            load(&router, &cookie, route, 4, Duration::from_millis(1000)).await;
            for &conc in &concs {
                let outcome = load(&router, &cookie, route, conc, Duration::from_secs_f64(secs)).await;
                println!(
                    "{route:<14} c={conc:<3} {:>8.0} req/s  p50 {:>7.3} ms  p99 {:>7.3} ms  cpu {:>6.1} µs/req  peak threads {}",
                    outcome.requests as f64 / outcome.elapsed.as_secs_f64(),
                    outcome.p50_ms,
                    outcome.p99_ms,
                    outcome.cpu.as_secs_f64() * 1e6 / outcome.requests as f64,
                    outcome.peak_threads
                );
            }
        }
    });
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn request(route: &str, cookie: &str) -> Request<Body> {
    let get = |path: String| {
        Request::get(path)
            .header(header::HOST, "campfire.test")
            .header(header::COOKIE, cookie)
            .header(header::ACCEPT_ENCODING, "gzip")
            .body(Body::empty())
            .unwrap()
    };
    match route {
        "room_show" => get(format!("/rooms/{ALL_TALK}")),
        "messages_page" => get(format!("/rooms/{ALL_TALK}/messages?before={BEFORE}")),
        "sidebar" => get("/users/me/sidebar".into()),
        "search" => get("/searches?q=coffee".into()),
        "post_message" => {
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            Request::post(format!("/rooms/{HQ}/messages"))
                .header(header::HOST, "campfire.test")
                .header(header::COOKIE, cookie)
                .header(header::ACCEPT_ENCODING, "gzip")
                .header(header::ACCEPT, "text/vnd.turbo-stream.html, text/html, application/xhtml+xml")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header("sec-fetch-site", "same-origin")
                .body(Body::from(format!("message%5Bbody%5D=bench+write+{n}&message%5Bclient_message_id%5D=timing-{n}")))
                .unwrap()
        }
        other => panic!("unknown route {other}"),
    }
}

struct Outcome {
    requests: usize,
    elapsed: Duration,
    p50_ms: f64,
    p99_ms: f64,
    cpu: Duration,
    peak_threads: usize,
}

async fn load(router: &axum::Router, cookie: &str, route: &str, conc: usize, duration: Duration) -> Outcome {
    let running = Arc::new(AtomicBool::new(true));
    let peak = Arc::new(AtomicUsize::new(threads()));
    let sampler = {
        let (running, peak) = (running.clone(), peak.clone());
        std::thread::spawn(move || {
            while running.load(Ordering::Relaxed) {
                peak.fetch_max(threads(), Ordering::Relaxed);
                std::thread::sleep(Duration::from_millis(2));
            }
        })
    };
    let cpu_before = process_cpu();
    let start = Instant::now();
    let deadline = start + duration;
    let tasks: Vec<_> = (0..conc)
        .map(|_| {
            let (router, cookie, route) = (router.clone(), cookie.to_string(), route.to_string());
            tokio::spawn(async move {
                let mut latencies = Vec::new();
                while Instant::now() < deadline {
                    let t0 = Instant::now();
                    let response = router.clone().oneshot(request(&route, &cookie)).await.unwrap();
                    let status = response.status();
                    axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
                    assert_eq!(status, StatusCode::OK, "{route}");
                    latencies.push(t0.elapsed());
                }
                latencies
            })
        })
        .collect();
    let mut latencies = Vec::new();
    for task in tasks {
        latencies.extend(task.await.unwrap());
    }
    let elapsed = start.elapsed();
    let cpu = process_cpu() - cpu_before;
    running.store(false, Ordering::Relaxed);
    sampler.join().unwrap();
    latencies.sort();
    let percentile = |p: f64| latencies[((latencies.len() - 1) as f64 * p) as usize].as_secs_f64() * 1e3;
    Outcome {
        requests: latencies.len(),
        elapsed,
        p50_ms: percentile(0.50),
        p99_ms: percentile(0.99),
        cpu,
        peak_threads: peak.load(Ordering::Relaxed),
    }
}

/// The process's thread count, from /proc/self/status.
fn threads() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    status.lines().find_map(|line| line.strip_prefix("Threads:")).unwrap().trim().parse().unwrap()
}

/// User plus system CPU time of the whole process, from /proc/self/stat (clock ticks of 10 ms).
fn process_cpu() -> Duration {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap();
    let fields: Vec<&str> = stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
    let ticks: u64 = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    Duration::from_millis(ticks * 10)
}
