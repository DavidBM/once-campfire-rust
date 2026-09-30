# Static files without a copy (2026-09-30)

`static_response` (crates/campfire/src/app.rs) sent every public file and digested asset by
copying the embedded bytes into a `Vec` (`served.body.into_owned()`) on each request. `e78bce9`
passes the `Cow` to `Body::from`, which sends a borrowed body as `Bytes::from_static`: no copy, no
allocation. Only a multipart range body is still owned. It also moves each header's `String` into
its `HeaderValue` (`try_from`, the same check as `from_str` without the copy).

Responses are byte-identical: `static_responses_send_the_embedded_bytes_without_copying_them`
serves every public file and asset (GET, HEAD, one range, two ranges, an unsatisfiable range,
If-Modified-Since) and compares status, headers and body with `campfire_assets::serve`, and checks
that a borrowed body goes out from the embedded bytes themselves.

## In process

`static_response_timing` (an ignored test beside the one above) times `static_response` for an
identity GET, building and dropping the response: 20,000 calls per run, 7 runs, median per call.
Native release test binary, three invocations per build; other agents' builds kept the host's
1-minute load average at 10–20. Raw output: [`runs.txt`](runs.txt).

    cargo test --release -p campfire --bin campfire static_response_timing -- --ignored --nocapture

| File | Before (`d93d3dd`) | After (`e78bce9`) | Change |
|---|---|---|---|
| /robots.txt (99 B) | 489 ns | 488 ns | same |
| _reset.css (1,218 B) | 509 ns | 488 ns | within noise |
| lexxy.js (922,910 B) | 14.32 µs | 487 ns | **29×** |
| lexxy.js.map (2,179,664 B) | 33.98 µs | 522 ns | **65×** |

Medians of the three invocations' medians. The copy was the whole difference: a body-only build
(`Body::from(served.body)` with the old `from_str` headers) gave 477, 492, 490 and 531 ns.

The header change is small. Interleaved with the body-only build over three rounds, it was about
20 ns faster at the median (510 → 488, 509 → 488, 513 → 487 and 547 → 522 ns), but in the third
round it was slower for three of the four files (_reset.css 510 vs 509 ns, lexxy.js 522 vs 513 ns,
the map 562 vs 546 ns): within this host's noise.

## Over HTTP

Measured in [`static-assets-http-20260930`](../static-assets-http-20260930/summary.md): main at
`0dbd10d` (the same code as `d93d3dd`) against `22bd7db`. Three native builds were interleaved over
three reps, 8 s per cell, with `--gzip 0`. Medians:

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

Serving a large asset without the copy saves 23–37% of its CPU per request, for 12–19% more
throughput at 16 clients. A 1.2 KB stylesheet is unchanged. At one client, throughput moves less
than CPU does.

In production, with the front on, the saving applies to front-cache misses, range requests (which
bypass the cache), identity requests for files over 1 MB and the bare app, not to every asset
request.
