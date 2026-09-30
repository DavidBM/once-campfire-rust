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

## Over HTTP

main at `0dbd10d` (the same code as `d93d3dd`) against `b0ed498`, native release builds
interleaved over three reps on a fresh copy of the seed. [`http/media_ab2.py`](http/media_ab2.py)
POSTs each file to `/rooms/:id/messages` on the bare app with curl, one at a time:
- random binary files as `application/octet-stream`, so they're stored with no image processing:
  15 posts of 1 MB, 11 of 10 MB and 5 of 100 MB
- two 2 s H.264 clips as `video/mp4`, which run ffprobe and the ffmpeg preview on the request
  path: 15 posts each

The app's `TMPDIR` was on the same btrfs volume as its storage. Each cell is the median of the three
reps' medians, with their range, of curl's total time; every POST answered 200.

| Upload | main | perf-media | Change |
|---|---|---|---|
| 1 MB binary | 6.1 ms (5.5–6.1) | 3.7 ms (3.5–4.0) | -2.4 ms |
| 10 MB binary | 39.8 ms (39.3–39.9) | 24.6 ms (24.0–25.4) | -15.3 ms |
| 100 MB binary | 316.0 ms (312.7–355.1) | 169.8 ms (169.3–169.9) | -146.3 ms |
| 2 s 320 × 240 clip | 77.8 ms (75.7–104.7) | 68.8 ms (63.7–74.5) | -9.0 ms |
| 2 s 1080p clip | 212.8 ms (183.5–215.8) | 182.7 ms (177.4–186.7) | -30.0 ms |

Staging saves about 1.45 ms per MB uploaded: the second MD5 pass, as in process. The clips gain
from the poll cap by different amounts, depending on when ffprobe and ffmpeg exit relative to the
old schedule's checks. Their times also vary much less between reps after it.
