//! Times the media work on an attachment upload's request path, in process: the video preview
//! command and ffprobe run with `Command::output()` and with `process::output_within`, and
//! `Storage::stage_file` for a 10 MB and a 100 MB file. Copied into crates/storage/examples/ to
//! run: `cargo run --release -p campfire_storage --example timing -- WORK_DIR SOURCE_DIR [RUNS]`.
//! The clips and the storage root go in WORK_DIR, the files to stage in SOURCE_DIR.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use campfire_storage::content_types::{VIDEO_PREVIEW_ARGUMENTS, ffmpeg_path, ffprobe_path};
use campfire_storage::key::checksum_file;
use campfire_storage::process::output_within;
use campfire_storage::{DiskService, Filename, Storage};
use rails_compat::Secrets;

const TIMEOUT: Duration = Duration::from_secs(60);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (work, sources) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    let runs: usize = args.get(2).map_or(21, |n| n.parse().unwrap());

    for (name, size) in [("small", "320x240"), ("1080p", "1920x1080")] {
        let clip = clip(&work, name, size);
        let (plain, within) = interleaved(runs, || run(preview(&clip), false), || run(preview(&clip), true));
        println!("preview {name}: output() {}, output_within {}", ms(plain), ms(within));
        let (plain, within) = interleaved(runs, || run(probe(&clip), false), || run(probe(&clip), true));
        println!("ffprobe {name}: output() {}, output_within {}", ms(plain), ms(within));
    }

    let verifier = rails_compat::app_verifier(&Secrets::new("timing"), "ActiveStorage");
    let storage = Storage::new(DiskService::new(work.join("storage"), "local"), verifier);
    for megabytes in [10, 100] {
        let source = upload(&sources, megabytes);
        let runs = if megabytes >= 100 { runs.min(11) } else { runs };
        let md5 = median(runs, || {
            checksum_file(&source).unwrap();
        });
        let staged = median(runs, || drop(storage.stage_file(&source, Filename::new("upload.mp4"), None).unwrap()));
        println!("{megabytes} MB: stage_file {}, one MD5 pass {}", ms(staged), ms(md5));
    }
}

/// `process::video_preview`'s command.
fn preview(clip: &Path) -> Command {
    let mut command = Command::new(ffmpeg_path());
    command.arg("-i").arg(clip).args(VIDEO_PREVIEW_ARGUMENTS).arg("-").stderr(Stdio::piped());
    command
}

/// `analyze::probe`'s command.
fn probe(clip: &Path) -> Command {
    let mut command = Command::new(ffprobe_path());
    command.args(["-print_format", "json", "-show_streams", "-show_format", "-v", "error"]).arg(clip).stderr(Stdio::inherit());
    command
}

fn run(mut command: Command, within: bool) {
    let output: Output = if within {
        output_within(&mut command, TIMEOUT).unwrap()
    } else {
        command.stdin(Stdio::null()).output().unwrap()
    };
    assert!(output.status.success() && !output.stdout.is_empty());
}

/// A two-second test clip, made once.
fn clip(work: &Path, name: &str, size: &str) -> PathBuf {
    let path = work.join(format!("clip-{name}.mp4"));
    if !path.exists() {
        let status = Command::new(ffmpeg_path())
            .args(["-y", "-v", "error", "-f", "lavfi", "-i", &format!("testsrc2=duration=2:size={size}:rate=30"), "-pix_fmt", "yuv420p"])
            .arg(&path)
            .status()
            .unwrap();
        assert!(status.success());
    }
    path
}

/// A file of pseudo-random bytes (xorshift), made once.
fn upload(dir: &Path, megabytes: usize) -> PathBuf {
    let path = dir.join(format!("upload-{megabytes}MB.bin"));
    if !path.exists() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut bytes = Vec::with_capacity(megabytes << 20);
        while bytes.len() < megabytes << 20 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.extend_from_slice(&state.to_le_bytes());
        }
        std::fs::File::create(&path).unwrap().write_all(&bytes).unwrap();
    }
    path
}

/// The median of `runs` timings of `a` and of `b`, run alternately after one untimed run each.
fn interleaved(runs: usize, mut a: impl FnMut(), mut b: impl FnMut()) -> (Duration, Duration) {
    a();
    b();
    let (mut times_a, mut times_b) = (Vec::new(), Vec::new());
    for _ in 0..runs {
        times_a.push(time(&mut a));
        times_b.push(time(&mut b));
    }
    (middle(times_a), middle(times_b))
}

/// The median of `runs` timings of `f`, after one untimed run.
fn median(runs: usize, mut f: impl FnMut()) -> Duration {
    f();
    middle((0..runs).map(|_| time(&mut f)).collect())
}

fn time(f: &mut impl FnMut()) -> Duration {
    let started = Instant::now();
    f();
    started.elapsed()
}

fn middle(mut times: Vec<Duration>) -> Duration {
    times.sort();
    times[times.len() / 2]
}

fn ms(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}
