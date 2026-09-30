# zlib-rs with its runtime CPU feature detection (2026-09-30)

flate2 was declared with `features = ["zlib-rs"]` alone, so zlib-rs was built without `std`, and
without `std` every one of its runtime CPU feature checks is `false` (zlib-rs 0.6.8,
`src/cpu_features.rs`). Production gzip ran the portable match comparison (`compare256`), window
slide (`slide_hash`) and CRC-32 even on CPUs with AVX2, BMI2 and PCLMULQDQ; on arm64 it had NEON
(a compile-time feature there) but not the ARMv8 CRC instructions. No other path gave it `std`:
`cargo tree -e features -i zlib-rs` at `d93d3dd` lists only `rust-allocator`. `26d3fff` adds
flate2's `runtime_detection`, which turns on `zlib-rs/std`, as the loadgen already did. Nothing new
is compiled; Cargo.lock only records flate2's weak `crc32fast?/std` edge.

## Same bytes

The compressed output doesn't change. In zlib-rs's deflate, the CPU features only choose between
implementations of pure functions with one right answer (the length of a match, the slid hash
table, the CRC); the hash function and the match strategy depend on the level alone
(`HashCalcVariant::for_max_chain_length`). Checked over 421 files, 19.6 MB (the four bench pages
rendered from the seed, the views' golden pages, and every asset source): the gzip, stored-piece
and cable-frame outputs of each have the same SHA-256 from both builds.

Nothing depends on the exact compressed bytes anyway: ETags are SHA-256s of the uncompressed body
or its parts, the splice CRC is `crc32fast` (already runtime-detected), compressed pieces and the
front server's cache live in memory only, and no test compares compressed bytes with a recorded
vector (the gzip tests decompress and compare the body).

## In process

[`gzip_timing.rs`](gzip_timing.rs), copied into crates/kit/examples/ and built at each commit, times
the app's three deflate call sites at level 6 (`Compression::default()`): whole-body gzip (the
`gzip_stream` of crates/kit/src/deflater.rs), a new message's piece (`splice::compress`, 4 KB with a
32 KB dictionary) and a cable frame (1.5 KB permessage-deflate). Per call, median of 7 runs of about
0.2 s; builds interleaved, three rounds; AMD Ryzen AI Max+ 395; other agents' builds kept the load
average at 6–10. Raw output: [`runs.txt`](runs.txt).

| Whole-body gzip | Before (`d93d3dd`) | After (`26d3fff`) | Change |
|---|---|---|---|
| room_show (416,018 B) | 753.3 µs | 634.3 µs | −15.8% |
| messages_page (383,844 B) | 443.7 µs | 346.4 µs | −21.9% |
| search (149,522 B) | 253.8 µs | 218.4 µs | −13.9% |
| sidebar (30,664 B) | 111.8 µs | 110.2 µs | within noise |
| lexxy.js (922,893 B) | 9.220 ms | 8.815 ms | −4.4% |

Stored pieces (55–60 µs) and cable frames (9.7–12.1 µs) are unchanged: at 4 KB and 1.5 KB there is
little window to slide and few long matches.

In steady state the room, messages and search pages (and the sidebar) reuse their stored
compressed pieces, so a request to them compresses only new content. Through the front server (on
by default), the gzipped response of every static asset is cached after its first request, the
source map included: the cache's item limit (`MAX_CACHE_ITEM_SIZE`, 1 MB) applies to the body it
records, which for a gzip client is already compressed, and the 2.18 MB map gzips to 426,066 B.
So with the front on, an asset is deflated once per cache fill, not once per request. Whole-body
deflate runs per request only for responses without parts (bodies under 1 KB among them), for cache
misses and first renders, and wherever the front is bypassed (the bare app on `TARGET_PORT`).

## Over HTTP

To be added by the coordinator. Through the front, lexxy.js and its map would come from the
response cache after the first request and time the same before and after, so the asset comparison
(`--gzip 1`) runs against the bare app port, `PORT + 1`.
