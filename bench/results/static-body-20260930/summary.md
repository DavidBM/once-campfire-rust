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

To be added by the coordinator. Through the front server, assets up to `MAX_CACHE_ITEM_SIZE` (1 MB)
come from its response cache after the first request, so only the source map reaches
`static_response` there; lexxy.js and _reset.css have to be requested from the bare app port. With
gzip, the per-request deflate of a large asset (about 9 ms for lexxy.js) hides the copy, so the
comparison is with `--gzip 0`.
