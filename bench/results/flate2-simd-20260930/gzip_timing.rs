//! Times the app's three deflate call sites on real bodies, with the flate2/zlib-rs build the
//! app gets, and prints a SHA-256 of every output so two builds can be compared byte for byte.
//! Copied into crates/kit/examples/ to run:
//!
//!   cargo run --release -p campfire_kit --example gzip_timing -- time FILE...
//!   cargo run --release -p campfire_kit --example gzip_timing -- digest FILE...
//!
//! - gzip: `Rack::Deflater`'s `gzip_stream` (crates/kit/src/deflater.rs) on the whole body as one
//!   chunk: level 6, mtime and OS header, a sync flush, then the trailer.
//! - piece: `splice::compress` for a new message's part: the body's last 4 KB, raw deflate at
//!   level 6 with the 32 KB before it as the preset dictionary, sync-flushed.
//! - frame: the cable's permessage-deflate (crates/cable/src/socket.rs `deflate`) of a 1.5 KB
//!   broadcast: a fresh raw stream at level 6, sync-flushed.

use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};

use flate2::{Compress, Compression, FlushCompress, GzBuilder};
use sha2::{Digest, Sha256};

const RUNS: usize = 7;
const WINDOW: usize = 32 * 1024;
const PIECE: usize = 4 * 1024;
const FRAME: usize = 1536;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mode, files) = args.split_first().expect("time|digest FILE...");
    match mode.as_str() {
        "time" => files.iter().for_each(|file| time(file)),
        "digest" => digest(files),
        _ => panic!("time|digest FILE..."),
    }
}

fn time(file: &str) {
    let body = std::fs::read(file).unwrap();
    let name = file.rsplit('/').next().unwrap();
    let gzipped = gzip(&body);
    println!("{name} ({} B -> {} B, {})", body.len(), gzipped.len(), short_sha(&gzipped));
    report("gzip", median_of(|| gzip(&body)));
    if body.len() >= WINDOW + PIECE {
        let (dictionary, text) = body.split_at(body.len() - PIECE);
        report("piece", median_of(|| raw_deflate(dictionary, text)));
    }
    if body.len() >= FRAME {
        report("frame", median_of(|| raw_deflate(b"", &body[..FRAME])));
    }
}

/// One line per file (the gzip output's SHA-256), then one over all of them.
fn digest(files: &[String]) {
    let mut all = Sha256::new();
    for file in files {
        let body = std::fs::read(file).unwrap();
        let mut outputs = vec![gzip(&body)];
        if body.len() >= WINDOW + PIECE {
            let (dictionary, text) = body.split_at(body.len() - PIECE);
            outputs.push(raw_deflate(dictionary, text));
        }
        outputs.push(raw_deflate(b"", &body[..body.len().min(FRAME)]));
        let shas: Vec<String> = outputs.iter().map(|output| short_sha(output)).collect();
        println!("{} {file}", shas.join(" "));
        outputs.iter().for_each(|output| all.update(output));
    }
    println!("{} files, all outputs: {}", files.len(), hex::encode(all.finalize()));
}

fn gzip(body: &[u8]) -> Vec<u8> {
    let mut encoder = GzBuilder::new().mtime(0).operating_system(3).write(Vec::new(), Compression::default());
    encoder.write_all(body).unwrap();
    encoder.flush().unwrap();
    encoder.finish().unwrap()
}

fn raw_deflate(dictionary: &[u8], text: &[u8]) -> Vec<u8> {
    let mut deflate = Compress::new(Compression::default(), false);
    if !dictionary.is_empty() {
        deflate.set_dictionary(&dictionary[dictionary.len().saturating_sub(WINDOW)..]).unwrap();
    }
    let mut out = Vec::with_capacity(text.len() / 4 + 64);
    loop {
        let consumed = deflate.total_in() as usize;
        if out.capacity() - out.len() < 1024 {
            out.reserve(out.capacity().max(4096));
        }
        deflate.compress_vec(&text[consumed..], &mut out, FlushCompress::Sync).unwrap();
        if deflate.total_in() as usize == text.len() && out.len() < out.capacity() {
            return out;
        }
    }
}

/// Per call: the median and range of 7 runs of about 0.2 s each.
fn median_of<T>(f: impl Fn() -> T) -> (Duration, Duration, Duration) {
    let started = Instant::now();
    black_box(f());
    let iterations = (Duration::from_millis(200).as_nanos() / started.elapsed().as_nanos().max(1)).clamp(3, 100_000) as u32;
    let mut runs: Vec<Duration> = (0..RUNS)
        .map(|_| {
            let started = Instant::now();
            for _ in 0..iterations {
                black_box(f());
            }
            started.elapsed() / iterations
        })
        .collect();
    runs.sort();
    (runs[RUNS / 2], runs[0], runs[RUNS - 1])
}

fn report(what: &str, (median, min, max): (Duration, Duration, Duration)) {
    println!("  {what}: median {median:?}, range {min:?}..{max:?}");
}

fn short_sha(bytes: &[u8]) -> String {
    hex::encode(&Sha256::digest(bytes)[..8])
}
