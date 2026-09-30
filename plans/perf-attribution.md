# Performance attribution: where the Rust port's time goes

Measurement only: no app code changed. This answers why the preliminary benchmark
(`bench/results/prelim-20260926/report.md`) shows the Rust port only 3–10× faster than Rails, and
ranks what to change next. Every number below comes from the runs in
`bench/results/attrib-20260926/`, made with the harness added to `bench/` for this
(see [Reproducing](#reproducing)).

## The short answer

The port isn't slow where Rust is slow; it spends its time on work Rails either also does in C or
skips altogether, and on a few pieces of avoidable plumbing:

1. **gzip is most of every page.** `Rack::Deflater` compresses every response, and the room page is
   464 KB. At the level Rails uses (6) the port's pure-Rust deflate (miniz_oxide) takes 2.2 ms per
   room page on one core, *slower* than C zlib, which Rails uses (1.9 ms for zlib 1.3.2 here). gzip is 62% of room show's
   CPU, 57% of the messages page's and 39% of the sidebar's. Rails pays the same ~2 ms, so the
   ratio between the two apps shrinks toward 1 on the big pages.
2. **Every message is rebuilt even when its fragment is cached.** The presenter builds a full
   `MessageView` (rich-text sanitize and render, an attachment query, avatar signing, boosts) for
   all ~40 messages, and only then asks the fragment cache, which hits and throws the view away.
   Rails' `cache [message, "presentation-v3"]` wraps the whole partial, so on a hit it evaluates
   none of it. That's 66% of room show's non-gzip CPU and 82% of its 16,700 allocations per
   request.
3. **Message POSTs stall on WAL checkpoints.** SQLite's auto-checkpoint runs on the single writer
   thread every ~64 posts and does three fsyncs (~4 ms each on this btrfs disk). That is the whole
   13 ms p99 at c=1, and it halves write throughput at c=16 (2,300 vs 5,000 req/s with the database
   on tmpfs).
4. **Cable fan-out zero-fills 128 KiB per socket read.** tungstenite resizes its read buffer to its
   full 128 KiB capacity (a memset) on every read attempt, and the connection loop polls the read
   half each time it wakes to send a frame. That's 57% of fan-out CPU and 125 MB of the memory at
   1,000 clients. A one-line setting (prototyped) gives 2.75× the delivery throughput.
5. **The serving setup caps the cheap routes.** Thruster costs twice the app's own CPU on `/up` and
   limits it to ~20k req/s (the app alone serves 111k), and 1 ms of CPU per uncompressed 464 KB
   body. Docker's userland proxy (a benchmark artifact: it only exists for published ports hit
   from the host) adds up to ~20% more. Together they cost as much CPU per cable delivery as the app.

Items 1, 2 and 4 are one-line-to-local changes; 3 is a contained refactor. Together they make the
room page ~2.5× cheaper at the same gzip level (~4.5× at level 1), fan-out ≥2.75× cheaper, and take
the POST tail from 12 ms to ~1 ms. Against the preliminary Rails numbers that moves room show from
4.4× to ~11× Rails at c=16, fan-out from ~5× to ≥13×, and POST p99 at c=1 from 2.2× to ~25×.

## Setup

Host: Ryzen 9 9955HX (16 cores/32 threads), kernel 7.2.5, btrfs `/home`. Server pinned to CPUs 8–11,
load generator to 12–15 (bench/run's defaults), with the reference process model (3 job workers, 5
readers). Other agents were running parity captures throughout (1-minute load average 8–40, recorded
per run in every report); configurations were interleaved within each rep so drift hits them alike,
and cells are medians of 3 reps with [min–max]. Profiles and memory runs used CPUs 4–7/0–3 on port
4490 while matrices ran.

The app under test is `a654b08` (the deadlock fix included), built into `campfire-rust:perf-a654b08`
for the Docker runs and natively (`target/perf-*`, `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`,
which doesn't change codegen) for build variants and profiles. `native-base` and Docker
`host-direct` agree within noise, so native results carry over.

`kernel.perf_event_paranoid=2` rules out perf and samply for an unprivileged user, so CPU profiles
come from gperftools' `libprofiler.so` loaded with `LD_PRELOAD` (SIGPROF sampling at 1 kHz,
unwinding through `.eh_frame`), allocation counts and heap profiles from jemalloc's profiler loaded
the same way, and the fsync trace from a 40-line `LD_PRELOAD` shim. None needs a special build.

## 1. Serving-setup overhead

`bench/results/attrib-20260926/setup/report.md` (3 reps × 4 configurations). "thrust" and
"docker-proxy" are the CPU those processes burn per request, read from `/proc/<pid>/stat`.

| c=16 unless noted | proxy-thruster (bench/run) | proxy-direct | host-thruster | host-direct | campfire CPU/req | thrust CPU/req | docker-proxy CPU/req |
|---|---|---|---|---|---|---|---|
| `/up` req/s | 18,485 | 91,027 | 20,379 | **111,374** (6.0×) | 0.03–0.05 ms | 0.10 ms | 0.01–0.02 ms |
| room show req/s (gzip) | 726 | 855 | 742 | **870** (1.20×) | 4.3–4.8 ms | 0.34 ms | 0.05 ms |
| room show req/s (identity, 464 KB) | 814 | 1,824 | 1,162 | **1,891** (2.3×) | 1.8–2.9 ms | 1.12 ms | 0.16 ms |
| messages page req/s | 851 | 1,059 | 1,003 | **1,072** (1.26×) | 3.4–3.9 ms | 0.35 ms | 0.05 ms |
| sidebar req/s | 3,671 | 5,398 | 4,250 | **4,624** (1.26×) | 0.66–0.79 ms | 0.18 ms | 0.03 ms |
| message POST req/s | 1,832 | 1,869 | 1,767 | 1,738 (0.95×) | 0.9 ms | 0.17 ms | 0.02 ms |
| message POST c=1 p50 / p99 ms | 1.07 / 13.1 | 0.83 / 12.4 | 0.91 / 12.2 | 0.76 / 12.5 | | | |
| cable 1,000 clients: msgs/s to all | 51.3 | 60.4 | 65.8 | **78.1** (1.52×) | 35–49 µs/delivery | 15–17 µs | 24–26 µs |

- **Thruster** is the ceiling for trivial routes (`/up`: 0.1 ms of thrust CPU per request against
  0.05 ms in the app, ~20k req/s either side of Docker's proxy) and a large tax on big uncompressed
  bodies (1.1 ms per 464 KB response). Behind gzip it's 5–10% of a page. It passes the app's gzip
  through untouched. The serving-layer work in flight removes this.
- **docker-proxy** costs 0.01–0.16 ms per request and 24 µs per cable delivery; it runs outside the
  container's cpuset, so it steals CPU from the load generator's side rather than the app's. It only
  exists because the benchmark reaches a published port from the host; production traffic arriving
  on the NIC is NATed by iptables. **bench/run should use `--network host`** (the Rust app binds
  `PORT`, Thruster `HTTP_PORT`) so neither app is measured through it.
- **Message POST throughput doesn't move with the setup at all**: only ~1.8 of 4 cores are busy.
  It's bound by the single writer thread and its checkpoint fsyncs (§5).
- For cable, Thruster + docker-proxy together burn ~40 µs per delivery, as much as the app itself.
  The load generator needs 23–32 µs per delivery, so at ~80k deliveries/s it's using 2–2.5 of its
  4 cores: fan-out results above ~120k deliveries/s are load-generator-bound.

## 2. Compression cost

`bench/results/attrib-20260926/gzip/` (`loadgen gzip`, one core, median of 100): the app's exact
call sequence (one write, sync flush, finish) on the room page as the app renders it.

| Backend, level | room page (463,818 B) | out bytes | sidebar (31,379 B) |
|---|---|---|---|
| miniz_oxide 6 (**the app today**, flate2's default backend) | **2,199 µs** | 44,242 | 154 µs |
| miniz_oxide 1 | 610 µs | 75,496 | 36 µs |
| zlib-rs 6 (flate2 feature `zlib-rs`) | **946 µs** | 45,583 (+3%) | 110 µs |
| zlib-rs 4 | 679 µs | 48,122 | 71 µs |
| zlib-rs 1 | 269 µs | 68,879 (+56%) | 28 µs |
| C zlib 1.3.2, level 6 (Python's `zlib`; Ruby's `Zlib` links the same library) | 1,893 µs | 44,341 | |

Inside the app gzip is 62% of room show CPU (≈2.7 ms at c=16, higher than on an idle core because
four busy SMT-sibling cores share caches), 57% of the messages page and 39% of the sidebar. The
ETag's SHA-256 over the same 464 KB (`Rack::ETag`) is another 4% (0.17 ms), already using the SHA
extensions.

Prototype (`native-expzlib`: only `flate2 = { default-features = false, features = ["zlib-rs"] }`,
3 interleaved reps, `experiments/report.md`): room show **1.46–1.54×** throughput (CPU 6.2 → 4.1
ms/req), messages page 1.32–1.60×, sidebar 1.13–1.42×, for +3% bytes.

## 3. CPU profiles

`bench/results/attrib-20260926/profile/cpu-*.svg` (flamegraphs) and `cpu-*.top.md` (rollups by
owning subsystem, top self and inclusive functions), native build, 10 s at c=16 (c=1 for
`post_message_c1`), and the saturated phase of the 1,000-client fan-out.

| Profile | CPU/req | Where it goes |
|---|---|---|
| room show | 4.3–4.8 ms | gzip 62%; building `MessageView`s 25%; ETag SHA-256 4%; askama page 2% |
| room show without gzip | 1.8–2.1 ms | `MessageView`s **66%** (rich text `present_message` 26%, attachment lookup 19.5% of which SQL *prepare* 12.5%, `plain_text_body` 7%, `user_view`/avatar signing 4%, boosts 2.4%); ETag 10%; tokio/hyper 6%; askama page 4% |
| messages page (`?before=`) | 3.4–3.9 ms | gzip 57%; `MessageView`s 32%; attachment-lookup prepares 7% |
| sidebar | 0.66–0.79 ms | gzip 39%; SQLite 19%; askama 12%; auth concerns 6%; tokio 5.5% |
| message POST (c=1) | 0.9 ms | readers 49% (broadcast render 22%, response render 13%, push job 8%), tokio workers 30%, writer thread 20%; SQLite 40% overall (prepare 9%); gzip of the turbo-stream 10% |
| cable fan-out, 1,000 clients | 35–65 µs/delivery | **memset in tungstenite `FrameCodec::read_in` 57%**; socket write syscalls 24%; the rest is channels and tasks |

The room page is also allocation-heavy: **16,651 allocations and 6.4 MB allocated per request**
(`profile/alloc-room_show.json`, `alloc-room_show.alloc_*.txt`; jemalloc full-rate profiling, 0 vs
1,000 requests), 82.5% of the allocations from building `MessageView`s (rich text 45%, plain text
23%, attachment lookup 8%, `user_view` 7.5%) and half the bytes from gzip state and output.

With the read-buffer fix prototyped (`profile-expws/cpu-cable1000.*`), fan-out CPU per delivery
drops from ~60 to ~23 µs and the profile becomes: write syscalls 44% (one `write` per frame per
client on loopback TCP, which also pays for the receiving side), memcpy 10% (a per-client copy of
each ~10 KB frame), mpsc 7.5%, read syscalls 7.7%.

## 4. Build tuning

`build/report.md` (6 variants × 3 reps, full route set and cable) and `build-cpu/report.md` (5
variants × 5 reps, CPU-bound routes at c=16). Variants: `base` (the workspace's release profile),
`lto` (`lto = "fat"`, `codegen-units = 1`), `tuned` (those plus `panic = "abort"`), and `-je` /
`-mi` for jemalloc 5.4.0 / mimalloc 2.3.2 loaded with `LD_PRELOAD` (a stand-in for
`#[global_allocator]` that also covers SQLite's and libvips' mallocs).

**The gains are below this host's noise floor.** Medians moved by ±5–25% in both directions
between the two runs (host load 8–41 during them), and no variant beat `base` consistently.
Best-of-reps campfire CPU per request (least-interfered sample) at c=16:

| | base | lto | tuned | base-je | tuned-je | base-mi |
|---|---|---|---|---|---|---|
| room show (gzip-bound), ms | 3.99–4.08 | 3.97–4.16 | 3.93–4.08 | 3.98–4.23 | 3.80 | 4.14 |
| room show without gzip, ms | 1.73–1.77 | 1.62–1.66 | 1.58–1.60 | 1.55–1.58 | 1.43 | 1.67 |
| sidebar, ms | 0.62–0.65 | 0.59–0.62 | 0.57–0.59 | 0.55–0.58 | 0.51 | 0.71 |
| messages page, ms | 3.12 | 3.05 | 3.02 | 3.18 | | 3.17 |
| RssAnon after the HTTP suite, MB (median) | 179 | 242 | 268 | 193 | | 429 |
| binary size (with line tables) | 112 MB | 88 MB | 81 MB | | | |

Read with that caveat: fat LTO with one codegen unit is worth ~5–10% on app-bound routes and
nothing on gzip-bound ones; `panic = "abort"` adds nothing measurable on top; jemalloc is worth
~5–10% of CPU on allocation-heavy routes and doesn't reduce retained memory at its default decay;
mimalloc (v2, preloaded) is no faster and retains twice the memory. None of this compares with
items 1–4 below, and re-measuring it needs a quiet host (or instruction counts, which need
`perf_event_paranoid ≤ 1` or valgrind).

## 5. The message POST's p99

`bench/stall` (`stall/stall-c1.json`, `stall/stall-c16.json`): 20 s closed-loop POSTs with every
request's start and latency recorded, while an `LD_PRELOAD` shim logs each `fsync`/`fdatasync` the
app makes. Same binary, database on the bench work dir (btrfs) or on `/dev/shm`.

| c=1, 3 reps | disk | tmpfs |
|---|---|---|
| req/s | 1,189–1,231 | 1,659–1,772 |
| p50 / p99 / max ms | 0.60–0.63 / **11.7–11.9** / 25–33 | 0.55–0.58 / **0.75–1.03** / 5–10 |
| requests ≥ 3 ms | 401–405, of which **98–99.8% overlap an fsync** | 1–4 |
| p99 of requests that overlap no fsync | 0.86–1.14 ms | 0.74–1.02 ms |
| fsyncs | every ~64 posts: 2× `-wal` (p50 4.0–4.3 ms) + 1× main db (3.3–3.5 ms) | same count, ~0 ms |

| c=16, 2 reps | disk | tmpfs |
|---|---|---|
| req/s | 2,315 (and 851 in a rep where host I/O pushed single fsyncs to 190–340 ms) | **5,077 / 5,006** |
| p99 ms | 23.6 / 192 | 5.1 / 5.2 |

Attribution: it is the **WAL auto-checkpoint**, not the writer queue or broadcast rendering. With
`synchronous=NORMAL` commits don't fsync; every ~1,000 WAL pages (≈16 pages per post: message,
rich text, the FTS index and memberships) SQLite checkpoints *inside the committing transaction on
the writer thread*, fsyncing the WAL twice and the database once. The POST that triggers it waits
~12 ms, and every write queued behind it waits too, which is why write throughput doubles when
fsync is free. Broadcast rendering and the response render run on readers after commit and are
visible in CPU (35% of a POST) but not in the tail. Rails runs the same pragmas (the adapter's
defaults, auto-checkpoint included), so it has the same stall; it has more latency elsewhere to
hide it.

## 6. Memory under cable fan-out

The preliminary report's ~1 GB peak was the container's `memory.current` over a whole rep: the
HTTP suite, then 100, 500 and 1,000 cable clients, including Thruster. Attributed
(`memory-*/`, `profile/heap-cable.json`, `heap-cable-peak.inuse_space.txt`, `experiments/`):

| At 1,000 subscribed clients (glibc malloc) | app RssAnon | Thruster RssAnon | container |
|---|---|---|---|
| fresh app, fan-out only (4 runs) | 12 → 170 connected → 208–232 saturated | 105–157 | 389–410 |
| after the full HTTP suite (as bench/run) | 290–345 before → 435–491 connected → 472–532 saturated | 105–156 | 734–747 |
| same with `RAILS_MAX_THREADS=128` (the prelim's workaround) | 352 → 500 → 534 | 153 | 747 |

- **tungstenite read buffers: 125 MB.** Each socket's `BytesMut::with_capacity(128 KiB)` is
  zero-filled by the first read, so it's resident: 58% of the live heap at peak (jemalloc heap
  profile, 212 MB live). With a 4 KiB read buffer (prototype) the app holds 59 MB instead of 179 MB
  once 1,000 clients are connected, and 101 MB instead of 213 MB saturated.
- **Send queues and per-client frame copies: ~20% of the live heap at peak (~45 MB).** Each stream
  forwarder formats `{"identifier":…,"message":…}` into a new `String` per client (≈10 KB for a
  message append), queued in a 256-deep mpsc per connection. The queues stayed short here (the
  load generator keeps up), so this is the part that grows if clients read slowly: the bound is
  256 × 10 KB × clients.
- **Thruster: 105–157 MB** for 2,000 proxied sockets.
- **Allocator retention: ~280–330 MB.** A fresh app is at 12 MB; after the HTTP suite (464 KB pages
  and gzip state at c=64 across up to 130 blocking-pool threads) glibc keeps ~300 MB it never
  returns. Readers aren't the cause (128 vs 5 readers: 534 vs 532 MB). See §4 for the allocator
  comparison.
- The rest (tasks, channels, subscriptions, tokio) is ~40 MB.

## Ranked optimizations

Gains are for the route named, measured where a prototype exists and otherwise estimated from the
profile shares above. "Parity risk" is the risk to the Playwright/vector parity suite.

| # | Change | Evidence | Expected gain | Parity risk | Files |
|---|---|---|---|---|---|
| 1 | **Set the WebSocket read buffer to 4–8 KiB** (`WebSocketUpgrade::read_buffer_size`) | 57% of fan-out CPU is `memset` in `FrameCodec::read_in`; 125 MB resident read buffers at 1,000 clients | **Prototyped: 2.75× fan-out throughput** (42k → 115k deliveries/s, load-generator-bound), 2.8× less CPU per delivery, paced post→all p50 29 → 15 ms, −112 MB at 1,000 clients | None: frames are unchanged and the buffer still grows for large incoming messages | `crates/cable/src/server.rs` |
| 2 | **Switch flate2 to the zlib-rs backend** at the same level 6 | gzip = 62/57/39% of room/messages/sidebar CPU; zlib-rs is 2.3× faster than miniz_oxide and faster than Rails' C zlib | **Prototyped: room show 1.5×, messages page 1.3–1.6×, sidebar 1.1–1.4×** throughput; +3% bytes | Low: gzip bytes already differ from Ruby's zlib, so parity can only compare decoded bodies (worth one check that nothing compares raw gzip bytes or lengths) | root `Cargo.toml` (`flate2` features) |
| 3 | **Look up the message fragment before building its view** (render `MessageView` only on a cache miss, as `cache [message, …]` does in Rails; needs only `id`/`updated_at` for the key) | 66% of room show's non-gzip CPU and 82% of its allocations build views that are discarded on a hit | room show −1.1 ms/req (≈ −65% of non-gzip CPU; ~1.35× with gzip as today, ~1.6× more after #2); messages page similar (views are 32% of it today); POSTs −5–8% (one of the two renders of a new message hits) | Medium-low: the miss path must stay byte-identical, including `message_tag`'s rescue into `_unrenderable` (inside the cache block in Rails too, so it only runs on a miss there as well) | `crates/campfire/src/controllers/presenters.rs`, `crates/views/src/messages.rs`, and the callers of `Presenter::messages`: `controllers/{rooms,messages,searches}.rs`, `controllers/rooms/refreshes.rs` |
| 4 | **Move WAL checkpoints off the writer thread**: `wal_autocheckpoint=0` on the writer, and a background thread running `PRAGMA wal_checkpoint(PASSIVE)` on its own connection when the WAL passes ~1,000 pages (or every few hundred ms) | 98–99.8% of POSTs ≥ 3 ms overlap a checkpoint's fsyncs; tmpfs removes them | POST p99 c=1 **~12 → ~1 ms**; POST throughput at c≥16 **up to 2.2×** (2.3k → 5.0k req/s upper bound) | None observable (same journal mode and durability; the reference's checkpoint timing isn't part of its behavior) | `crates/db/src/database.rs`, `crates/db/src/schema.rs` |
| 5 | **Cache prepared statements** where queries use `query_row`/`prepare` directly, and raise rusqlite's per-connection statement cache from its default 16 | `Blob::attached` prepares its SQL on every call: 12.5% of room show (no gzip), 4.6% of a POST; others in `presenters_a::query_users`, `Message::create`, `unread_memberships`, `touch_row` | room show −13% CPU without #3 (−2% with it); POST −5–9%; sidebar −5% | None | `crates/storage/src/blob.rs`, `crates/campfire/src/controllers/presenters_a/*`, `crates/db/src/models/{message,room}.rs`, `crates/db/src/database.rs` (`set_prepared_statement_cache_capacity`) |
| 6 | **Batch cable writes and share encoded frames**: `feed` every pending frame and flush once per wake-up (one `write` for several frames), stop re-polling the read half on every outbound wake-up, and encode `{"identifier","message"}` once per (identifier, payload) instead of per client | after #1, write syscalls are 44% of fan-out CPU, per-client ~10 KB frame copies 10%, read syscalls 8% | fan-out another ~1.3–1.6× (estimate); smaller per-client queues | None (same frames, same order per connection) | `crates/cable/src/connection.rs`, `crates/cable/src/channel.rs` |
| 7 | **jemalloc as the global allocator** (see §4) | 16.7k allocations per room show; best-of-reps −5–10% CPU on app-bound routes with jemalloc preloaded; mimalloc no better and 2× the retained memory | ~5–10% CPU on app-bound routes; no memory win at default settings | None | `crates/campfire/Cargo.toml`, `crates/campfire/src/main.rs` |
| 8 | **Run reads on reader threads** (one per reader connection, taking reads from one queue) instead of `spawn_blocking`, which parks blocking-pool threads on the pool's `Condvar` | 99–136 threads under load for 5 readers; each glibc thread can pin an arena | fewer threads and less retained memory (part of the ~300 MB above). Measured in process (bench/results/db-hops-20260930): a fixed thread count, and 8–34% more requests/s on read-heavy routes at 16 and 64 concurrent requests. The async semaphore first proposed here (waiting for a connection before `spawn_blocking`) also bounds the threads, but lost those routes 13–22% to its extra hand-off between threads | None | `crates/db/src/database.rs` |
| 9 | **Measure without docker-proxy** (`--network host` in bench/run) and finish replacing Thruster (in flight) | §1 | `/up` 6×; big uncompressed bodies 2.3×; pages 1.2–1.3×; fan-out 1.5× | None | `bench/run`; serving layer |
| 10 | Build profile: `lto = "fat"`, `codegen-units = 1` | §4 | ~5–10% on app-bound routes (best-of-reps; medians within noise), 22% smaller binary; clean release builds ~2× slower (1m24s → 2m40s) | None | root `Cargo.toml` |
| — | Not recommended: `panic = "abort"` | §4 | nothing measurable beyond LTO | **High**: the DB writer thread and the Web Push pool `catch_unwind` so one bad job can't take them down, and a panicking request task would abort the process instead of failing one request | — |
| — | Not worth it now: ETag hashing (4% of room show, already SHA-NI; required by `Rack::ETag`), gzip level below 6 (zlib-rs level 1 is another 3.5× cheaper but +56% bytes: a product decision, not a parity-neutral one) | | | | |

Rough combined effect on the room page (c=16, 4.5 ms of CPU today: gzip 2.8, views 1.1, rest
0.6): ~3.0 ms with #2 → ~1.9 ms with #3 (#5's biggest caller disappears with #3), i.e. ~2.4× the
throughput, ~2.7× with the build tuning in §4, ~4.5× if gzip level 1 were acceptable. Cable fan-out
is ≥2.75× with #1 alone (the load generator becomes the limit). POSTs lose their p99 cliff and
roughly double their ceiling with #4.

## Reproducing

```sh
# Serving setup: 4 configurations × 3 reps (routes, CPU per process per request, cable memory by phase)
RUST_IMAGE=campfire-rust:perf-a654b08 bench/attrib --reps 3 --out bench/results/<dir>/setup
# Build variants and allocators (native binaries; allocators via LD_PRELOAD)
NATIVE_BASE_BIN=... NATIVE_BASE_MI_BIN=... NATIVE_BASE_MI_PRELOAD=.../libmimalloc.so \
  bench/attrib --configs native-base,native-base-mi --reps 3 --secs 4 --out ...
# CPU profiles, allocation counts, peak heap during fan-out
DEBUGINFOD_URLS=https://debuginfod.archlinux.org NATIVE_BASE_BIN=... bench/profile cpu --label base
bench/profile alloc --label base --route room_show --requests 1000
bench/profile heap --label base --clients 1000
# POST tail vs fsync
bench/stall --label base --conc 1 --reps 3
# gzip cost of a fetched page
target/bench/release/loadgen fetch --base ... --cookie ... --path /rooms/<id> --out room.html
target/bench/release/loadgen gzip --file room.html
# Drill into a profile
bench/lib/share.py profile/cpu-room_show.folded 'gzip=miniz' 'views=renderable_message'
bench/lib/children.py profile/cpu-room_show_identity.folded 'campfire::content$'
```

Binaries were built from a clean worktree of `a654b08`:
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only cargo build --release -p campfire`, plus
`CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 [CARGO_PROFILE_RELEASE_PANIC=abort]`
for the tuned variants (no profile change committed). mimalloc 2.3.2 was built from the
`libmimalloc-sys` 0.1.49 sources (`cc -O3 -DMI_MALLOC_OVERRIDE -shared src/static.c`); jemalloc is
the system's 5.4.0 (`/usr/lib/libjemalloc.so`). Folded stacks, raw profiles and per-request traces are
gitignored (they run to gigabytes); the flamegraphs, rollups and JSON summaries are committed.
