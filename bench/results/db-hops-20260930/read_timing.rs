//! Times `Database::read` under concurrency: C tasks each loop one small indexed read (a user by id,
//! as the session and membership lookups are) for a few seconds, on a 4-worker runtime with 5 reader
//! connections (`RAILS_MAX_THREADS`' default). Reports reads/s, p50/p99 latency, process CPU per
//! read and the peak thread count. Copied into crates/db/examples/ to run:
//!
//!   CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
//!     cargo build --release -p campfire_db --example read_timing
//!   taskset -c 8-11 target/release/examples/read_timing
//!
//! `TIMING_CONCS` (default 1,16,64,256) and `TIMING_SECS` (default 3) change the matrix.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use campfire_db::fixtures;
use campfire_db::{Config, Database, Env, User};

fn main() {
    let concs: Vec<usize> = std::env::var("TIMING_CONCS")
        .unwrap_or_else(|_| "1,16,64,256".into())
        .split(',')
        .map(|c| c.parse().unwrap())
        .collect();
    let secs: f64 = std::env::var("TIMING_SECS").map_or(3.0, |s| s.parse().unwrap());
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build().unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::new(dir.path().join("timing.sqlite3"));
        config.readers = 5;
        config.environment = "test".into();
        let db = Database::open(config, Env::default()).unwrap();
        db.write(|tx| {
            let options = fixtures::Options { now: tx.now(), bcrypt_cost: 4 };
            fixtures::load(tx.conn(), &fixtures::reference_dir(), &options).map(|_| ())
        })
        .await
        .unwrap();
        let user_id = fixtures::identify("david");

        load(&db, user_id, 16, Duration::from_millis(500)).await;
        for conc in concs {
            let result = load(&db, user_id, conc, Duration::from_secs_f64(secs)).await;
            println!(
                "c={conc:<4} {:>9.0} reads/s  p50 {:>7.1} µs  p99 {:>8.1} µs  cpu {:>5.2} µs/read  peak threads {}",
                result.reads as f64 / result.elapsed.as_secs_f64(),
                result.p50_us,
                result.p99_us,
                result.cpu.as_secs_f64() * 1e6 / result.reads as f64,
                result.peak_threads
            );
        }
    });
}

struct Outcome {
    reads: usize,
    elapsed: Duration,
    p50_us: f64,
    p99_us: f64,
    cpu: Duration,
    peak_threads: usize,
}

async fn load(db: &Database, user_id: i64, conc: usize, duration: Duration) -> Outcome {
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
            let db = db.clone();
            tokio::spawn(async move {
                let mut latencies = Vec::new();
                while Instant::now() < deadline {
                    let t0 = Instant::now();
                    db.read(move |conn| User::find(conn, user_id)).await.unwrap();
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
    let percentile = |p: f64| latencies[((latencies.len() - 1) as f64 * p) as usize].as_secs_f64() * 1e6;
    Outcome { reads: latencies.len(), elapsed, p50_us: percentile(0.50), p99_us: percentile(0.99), cpu, peak_threads: peak.load(Ordering::Relaxed) }
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
