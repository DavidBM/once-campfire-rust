# Database round trips, 2026-09-30

Branch `perf-db-hops` on `d93d3dd`: reads on reader threads that take them from one queue instead of `spawn_blocking` (`aa04095`), one reader trip for the layout and for search (`6bfee19`), and no read-back after a text-only post (`2e7f20d`).

## How

Everything was measured in process, with no sockets and no HTTP load test.

- [`read_timing.rs`](read_timing.rs) times `Database::read` alone. C tasks each loop a small indexed read (`User::find`) on a 4-worker runtime with 5 reader connections. 5 reps of 3 s.
- [`route_timing.rs`](route_timing.rs) times whole requests through the app's router, booted over the `default` parity seed, as `bench/run` makes them (David, gzip, the busy room, posts to HQ). There is no front server and no Action Cable server, so its thread counts leave out the cable connections runtime. 3 reps of 3 s, plus 5 reps at one client ([`route-one-client-runs.txt`](route-one-client-runs.txt)).
- Everything was pinned to 4 CPUs (`taskset -c 8-11`), with builds interleaved per rep. Other agents' builds loaded the host throughout; the 1-minute load average was 6–20.
- The builds were:
  - the base, `d93d3dd`;
  - the review's semaphore, [`semaphore.diff`](semaphore.diff) against `d93d3dd`, which takes a permit per reader connection before `spawn_blocking`;
  - each commit as first written: `cf21465`, `f1638d0` and `9f94135`.
- The review fixes rewrote those three commits as `aa04095`, `6bfee19` and `2e7f20d`. On the measured paths, one thing changed: a queued read whose caller has gone now runs instead of being skipped, because some reads broadcast.
- [`read-rerun.txt`](read-rerun.txt) repeats the read timings on `aa04095`, the base and the semaphore patch, all rebuilt from source. Every median came within 2.5% of the first runs ([`read-runs.txt`](read-runs.txt)).

## Reads alone

Medians of 5 runs: reads/s, CPU per read and p99, then the peak thread count.

| Concurrent reads | Base | Semaphore | Reader threads |
|---|---|---|---|
| 1 | 175k, 6.8 µs, p99 9.5 µs | 182k, 6.7 µs, 7.9 µs | 206k, 5.9 µs, 7.1 µs |
| 16 | 794k, 4.9 µs, p99 213 µs | 643k, 5.7 µs, 67 µs | 1,890k, 2.1 µs, 19 µs |
| 64 | 748k, 5.3 µs, p99 690 µs | 638k, 5.7 µs, 147 µs | 1,908k, 2.0 µs, 70 µs |
| Peak threads | 165–449 | 45–91 | 13 |

## Whole requests

Medians of 3 runs, in requests/s, with the change against the base. Each column after the semaphore adds one commit to the column before it.

| Route | Clients | Base | Semaphore | Reader threads | + merged trips | + no read-back |
|---|---|---|---|---|---|---|
| room_show | 16 | 21,797 | 19,058 (−13%) | 23,640 (+8%) | 23,545 (+8%) | 24,015 (+10%) |
| room_show | 64 | 21,918 | 18,353 (−16%) | 25,045 (+14%) | 25,305 (+15%) | 25,027 (+14%) |
| messages_page | 16 | 25,248 | 20,856 (−17%) | 28,159 (+12%) | 29,191 (+16%) | 29,157 (+15%) |
| messages_page | 64 | 26,165 | 20,953 (−20%) | 31,624 (+21%) | 31,966 (+22%) | 31,908 (+22%) |
| sidebar | 16 | 25,909 | 21,600 (−17%) | 28,545 (+10%) | 30,458 (+18%) | 30,132 (+16%) |
| sidebar | 64 | 26,687 | 21,854 (−18%) | 32,772 (+23%) | 33,334 (+25%) | 33,111 (+24%) |
| search | 1 | 7,351 | 7,211 (−2%) | 7,648 (+4%) | 8,333 (+13%) | 8,293 (+13%) |
| search | 16 | 25,507 | 21,734 (−15%) | 29,463 (+16%) | 32,904 (+29%) | 33,361 (+31%) |
| search | 64 | 27,686 | 21,732 (−22%) | 37,222 (+34%) | 39,784 (+44%) | 39,359 (+42%) |
| post_message | 16 | 5,890 | 6,093 (+3%) | 6,714 (+14%) | 6,637 (+13%) | 6,413 (+9%) |
| post_message | 64 | 6,145 | 6,290 (+2%) | 7,005 (+14%) | 6,575 (+7%) | 6,934 (+13%) |
| Peak threads | 16, 64 | 78–198 | 41–61 | 14–15 | 14–15 | 14–15 |

- **Reader threads** save CPU per request on every read-heavy route. At 64 clients, room_show uses 157 instead of 179 µs, and search 102 instead of 141 µs. p99 is lower too, for example messages_page at 64 clients: 3.6 instead of 4.0 ms.
- **The semaphore** bounds the threads, but every contended read takes one more hand-off between threads, and it loses 13–22% on read-heavy routes at 16 and 64 clients.
- **Merged trips** add 7–12% to search at every concurrency.
- **The skipped read-back** saves one small read, about 6 µs out of a post's roughly 400 µs of CPU. That is below this host's noise: at one client, post_message ranged over 1,851–2,614 req/s between reps of the same build.
- Changes under about 5% are noise. For example, room_show at one client ranged from −2% to +6% across builds whose GET paths are identical.

## What an HTTP A/B should show

The largest changes are at 64 clients, on room_show, messages_page, sidebar and search. search also changes at 1 and 16 clients. post_message changes only at 16 and 64 clients, through its reads.

The real server's thread count also includes the cable connections runtime (one worker per CPU) and any Web Push threads. Expect about 20–25 threads under load rather than 14–15: still a fixed count, and about 5× fewer than on the base.

## HTTP A/B

Measured over HTTP alongside the other page-path branches, in
[`hot-paths-http-20260930`](../hot-paths-http-20260930/summary.md): `main` at `0dbd10d` (the same code
as `d93d3dd`) against `6517414`, native release builds interleaved over 5 reps. Another project
raised the host's load average from 5 to 16 during the run. At one client, req/s moves by up to 20%
between reps of the same build. Medians:

| Route | Clients | main req/s | branch req/s | main CPU µs/req | branch CPU µs/req | p99 main → branch |
|---|---|---|---|---|---|---|
| room_show | 1 | 4,922 | 4,974 (1.01×) | 196.6 | 189.7 (-6.9) | 0.28 → 0.31 ms |
| room_show | 16 | 19,496 | 20,330 (1.04×) | 195.9 | 176.9 (-19.0) | 1.56 → 1.53 ms |
| room_show | 64 | 19,281 | 22,308 (1.16×) | 195.9 | 170.2 (-25.7) | 5.64 → 5.06 ms |
| messages_page | 1 | 5,503 | 5,841 (1.06×) | 179.2 | 162.4 (-16.8) | 0.27 → 0.26 ms |
| messages_page | 16 | 22,442 | 25,138 (1.12×) | 164.4 | 145.2 (-19.2) | 1.53 → 1.24 ms |
| messages_page | 64 | 22,514 | 27,574 (1.22×) | 164.6 | 135.7 (-28.9) | 4.84 → 4.12 ms |
| sidebar | 1 | 4,747 | 5,400 (1.14×) | 193.0 | 166.3 (-26.7) | 0.33 → 0.30 ms |
| sidebar | 16 | 21,485 | 23,801 (1.11×) | 169.0 | 147.3 (-21.7) | 1.57 → 1.40 ms |
| sidebar | 64 | 22,186 | 27,024 (1.22×) | 166.6 | 137.8 (-28.8) | 5.02 → 4.29 ms |
| search | 1 | 5,758 | 6,957 (1.21×) | 165.2 | 136.0 (-29.2) | 0.28 → 0.23 ms |
| search | 16 | 22,490 | 26,612 (1.18×) | 161.7 | 127.5 (-34.2) | 1.50 → 1.24 ms |
| search | 64 | 23,128 | 31,991 (1.38×) | 161.6 | 115.6 (-46.0) | 4.57 → 3.58 ms |
| post_message | 1 | 1,856 | 2,006 (1.08×) | 551.7 | 521.5 (-30.2) | 1.83 → 1.92 ms |
| post_message | 16 | 5,343 | 5,618 (1.05×) | 552.9 | 454.4 (-98.5) | 8.01 → 8.14 ms |
| post_message | 64 | 5,471 | 6,279 (1.15×) | 547.4 | 444.4 (-103.0) | 19.63 → 16.88 ms |

Over HTTP, with the front server and gzip, every read route saves 19–46 µs of CPU per request at
16 and 64 clients. Throughput is up 4–38%: search 1.18× and 1.38×, messages_page and sidebar 1.22×
at 64 clients. post_message saves about 100 µs of CPU per request at 16 and 64 clients. These are
close to the in-process route timings above: the largest changes are at 64 clients and on search.
Thread counts weren't sampled in this run.
