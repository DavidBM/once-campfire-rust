# Media subprocesses and upload staging, 2026-09-30

Branch `perf-media` on `d93d3dd`:
- `5cb64af` polls a media subprocess at least every 5 ms. The old pause doubled from 1 ms up to
  32 ms and then stayed at 50 ms.
- `1b449ad` reads and hashes a staged upload once.

## In process

[`timing.rs`](timing.rs) was copied into `crates/storage/examples` and run with `--release`.

Setup:
- The builds were `d93d3dd` (before), `5cb64af` (the poll cap) and `ba03155` (after). `ba03155` is an
  earlier draft of `1b449ad` with the same code; only a doc comment differs.
- The three builds ran interleaved, three times, on btrfs.
- Other agents were compiling at the time (load average 10–12).
- Each figure is the median of the three runs' medians. Each run's median is over 21 runs, or 11 for
  100 MB.
- Every line is in [`runs.txt`](runs.txt).

| Command | `output()` | `output_within` before | after |
|---|---|---|---|
| Preview, 320×240 clip | 32.9 ms | 63.5 ms | 32.6 ms |
| Preview, 1080p clip | 77.1 ms | 113.6 ms | 73.2 ms |
| ffprobe, 320×240 | 26.5 ms | 31.4 ms | 27.7 ms |
| ffprobe, 1080p | 31.2 ms | 31.5 ms (63.4 once) | 32.6 ms |

The old pauses checked at 31, 63 and 113 ms, so a command's time depended on which side of a check
the child exited. After the cap, `output_within` is always within 5 ms of `output()`. The
reference's fixture video, `alpha-centuri.mov`, previews in about 30 ms and probes in about 25 ms,
right at the old 31 ms check. Previewing a video upload runs on the request path, after ffprobe.

| Staging a file | Before | After | One MD5 pass |
|---|---|---|---|
| 10 MB | 28.0 ms | 13.7 ms | 13.5 ms |
| 100 MB | 278.5 ms | 136.4 ms | 135.7 ms |

MD5 runs at about 740 MB/s here, so the second pass cost about 1.35 ms per MB uploaded (0.7 ms for
a 505 KB JPEG). On btrfs, `copy_file_range` reflinks, so the copy itself is nearly free. Where the
kernel copies the data (ext4, or a tempfile on another filesystem), the copy takes the same time
before and after.
