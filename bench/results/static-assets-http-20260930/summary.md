# Static assets over HTTP: perf-static-body and perf-flate2-simd against main, 2026-09-30

Three native release builds, interleaved per rep, each on a fresh copy of the seed:
- main at `0dbd10d` (the same code as the branches' base `d93d3dd`)
- `perf-static-body` at `22bd7db`
- `perf-flate2-simd` at `2d9b6a8`

[`assets_ab.py`](assets_ab.py) drives bench/lib's loadgen, pinned as bench/attrib pins it. For
each request it warms up for 2 s at 4 clients, then runs 8 s each at 1 and 16 clients, for three
reps. The bare app is on `PORT + 1`, so the front server's response cache can't answer. The front
case is the source map, whose 2.18 MB identity response is over the cache's 1 MB item limit. The
host's load average was 5–11 ([`env.txt`](env.txt), `load_start` in each JSON). Medians of the
three reps, with the app's CPU per request from `cpu_ms_per_req`.

perf-static-body, identity (`--gzip 0`):

| Request | Clients | main req/s | branch req/s | main CPU/req | branch CPU/req |
|---|---|---|---|---|---|
| lexxy.js (923 KB), bare app, identity | 1 | 6,771 | 6,752 (1.00×) | 68.8 µs | 51.9 µs (-25%) |
| lexxy.js (923 KB), bare app, identity | 16 | 29,977 | 33,646 (1.12×) | 86.0 µs | 66.2 µs (-23%) |
| lexxy.js.map (2.18 MB), bare app, identity | 1 | 2,738 | 2,732 (1.00×) | 141.6 µs | 107.1 µs (-24%) |
| lexxy.js.map (2.18 MB), bare app, identity | 16 | 6,619 | 7,733 (1.17×) | 312.2 µs | 196.6 µs (-37%) |
| _reset.css (1.2 KB), bare app, identity | 1 | 73,961 | 74,540 (1.01×) | 7.8 µs | 7.7 µs (-1%) |
| _reset.css (1.2 KB), bare app, identity | 16 | 489,498 | 502,821 (1.03×) | 6.2 µs | 6.1 µs (-2%) |
| lexxy.js.map (2.18 MB), front server, identity | 1 | 2,695 | 3,083 (1.14×) | 149.3 µs | 115.3 µs (-23%) |
| lexxy.js.map (2.18 MB), front server, identity | 16 | 6,546 | 7,808 (1.19×) | 324.7 µs | 208.8 µs (-36%) |

perf-flate2-simd, gzip (`--gzip 1`), where every request deflates the whole asset on the bare app:

| Request | Clients | main req/s | branch req/s | main CPU/req | branch CPU/req |
|---|---|---|---|---|---|
| lexxy.js, bare app, gzip | 1 | 101 | 105 (1.05×) | 9.83 ms | 9.38 ms (-5%) |
| lexxy.js, bare app, gzip | 16 | 400 | 421 (1.05×) | 9.95 ms | 9.46 ms (-5%) |
| lexxy.js.map, bare app, gzip | 1 | 39 | 41 (1.05×) | 25.29 ms | 24.15 ms (-5%) |
| lexxy.js.map, bare app, gzip | 16 | 156 | 164 (1.05×) | 25.55 ms | 24.37 ms (-5%) |

Every response was a 200.
