# Page parts on the hot path (plan §4.6, "Page parts"), 2026-09-30

The `perf-splice` branch, from `d93d3dd`, one commit per change:
1. `7ae402c` `locate` scans the `MAX_GLUE` window after the previous fragment before building a
   memmem `Finder`.
2. `5b4d5b2` The room, search and messages pages render into a buffer presized from their cached
   fragments' lengths, the layout's asset tags and a 16 KB margin.
3. `03a471a` `pieces()` builds its `Vec<Bytes>` directly (no `Option`, no `expect`). Not a
   performance change.
4. `7a1776d` A text part's SHA-256 is remembered by its bytes (foldhash plus a compare), in a
   16 MB two-generation map with a 256 KB per-entry cap.
5. `b0e7d15` A piece names its predecessor by SHA-256; the `_pin` Weaks are gone.
6. `a145d4e` FRAGMENTS is a 32 MB two-generation map instead of 8,192 entries with `retain` and
   `clear()`.

Gzip bodies, ETags and Content-Length are unchanged: the digests are the same SHA-256s, and the
splice tests decode every page they gzip.

## In-process timings

Timing harnesses in the tree: `deflater::splice::tests::timing` (kit) and
`controllers::presenters::page::tests::render_timing` (campfire, so it runs under the binary's
jemalloc). Both are `#[ignore]`d tests, run with `cargo test --release` (fat LTO) and pinned to
one core with `taskset -c 20`. Each figure is the median of 9 batches (2,000–20,000 iterations
each) per run. The table gives the median of 3 runs, with the range of the 3 in brackets. The
kit harness page is shaped like a captured room page: a 27 KB head, 40 messages of 9–11 KB with
7 bytes of glue between them, and a 7 KB tail (440 KB in all). The sidebar is a 30 KB page
without fragments.

"Before" is `d93d3dd` with the same harness pasted in. "After" is `a145d4e`. The two ran
back to back at 07:28–07:30, while other agents were compiling: the 1-minute load average was
5–7 on 32 threads.

| µs per page | Before | After |
|---|---|---|
| `locate`, 40 fragments | 12.68 [11.94–12.85] | 5.64 [5.64–6.17] |
| Text identity, head and tail (SHA-256 before; lookup after) | 14.04 [13.68–14.14] | 0.88 [0.88–0.92] |
| Whole assembly: `PageParts::new`, ETag, gzip from stored pieces | 35.19 [34.80–35.40] | 15.74 [15.49–16.02] |
| Sidebar, whole-page part: the same three | 12.80 [12.79–12.94] | 1.30 [1.29–1.36] |
| `locate`, fragments 900 B of markup apart | 12.71 [12.64–13.34] | 15.43 [15.18–15.79] |
| `locate`, fragments 900 B of `<b>x</b>` apart | 12.37 [12.13–12.46] | 22.11 [22.02–23.16] |
| Room page render, 437 KB, 40 cached messages | 21.51 [21.10–21.75] | 19.10 [18.93–19.62] |

The render row comes from one binary (07:32): askama's `render` against `layouts::render_page`.
An earlier session measured 23.98 µs against 21.09 µs.

Per commit, whole assembly: 34.2 µs at `d93d3dd`, 30.1 after (1), 16.0 after (4), 15.4 after (6).
In commit (6), `fragment_shas` does one foldhash lookup per fragment instead of two SipHash ones.

Together a room page saves about 22 µs of CPU: ~7 in `locate`, ~13 in text identity, ~2.5 in
the render and ~0.6 in fragment lookups. That is roughly a tenth of room_show's 0.20 ms per
request (`wave3-20260930`).

## Caveats

- The benchmark asks for one page as one person, so the text map hits every time: this is the
  upper bound. Each person-and-room page has its own ~34 KB of text, and the 16 MB map holds at
  least ~240 of them (a generation). A miss costs what every request cost before, plus a copy of
  the text (~1 µs for 34 KB).
- Memory: the text map starts empty and grows to at most 16 MB (plus two 256 KB entries).
  FRAGMENTS is now bounded at 32 MB including its pieces. Before, it held at most 8,192 entries,
  with no bound on their pieces' bytes. A presized page is at most 16 KB plus a 12 KB guess per
  uncached message over its length. Before, doubling left up to 2× its length.
- Fragments far apart pay the window scan on top of the old search: +2.7 µs over 40 fragments
  separated by ordinary markup, and +9.7 µs when every fifth byte is a `<`. No template does this.
  Every template puts `"\n    "`, `"\n"` or a turbo-stream wrapper (under 256 B) between messages.

## HTTP A/B

Measured over HTTP alongside the other page-path branches, in
[`hot-paths-http-20260930`](../hot-paths-http-20260930/summary.md): `main` at `0dbd10d` (the same code
as `d93d3dd`) against `68444f7`, native release builds interleaved over 5 reps. Another project
raised the host's load average from 5 to 16 during the run. At one client, req/s moves by up to 20%
between reps of the same build. Medians:

| Route | Clients | main req/s | branch req/s | main CPU µs/req | branch CPU µs/req | p99 main → branch |
|---|---|---|---|---|---|---|
| room_show | 1 | 4,922 | 5,464 (1.11×) | 196.6 | 175.3 (-21.3) | 0.28 → 0.28 ms |
| room_show | 16 | 19,496 | 21,620 (1.11×) | 195.9 | 172.4 (-23.5) | 1.56 → 1.53 ms |
| room_show | 64 | 19,281 | 21,791 (1.13×) | 195.9 | 171.3 (-24.6) | 5.64 → 4.91 ms |
| messages_page | 1 | 5,503 | 6,075 (1.10×) | 179.2 | 163.0 (-16.2) | 0.27 → 0.25 ms |
| messages_page | 16 | 22,442 | 23,477 (1.05×) | 164.4 | 152.6 (-11.8) | 1.53 → 1.48 ms |
| messages_page | 64 | 22,514 | 24,557 (1.09×) | 164.6 | 152.5 (-12.1) | 4.84 → 4.43 ms |
| sidebar | 1 | 4,747 | 6,019 (1.27×) | 193.0 | 159.2 (-33.8) | 0.33 → 0.24 ms |
| sidebar | 16 | 21,485 | 23,188 (1.08×) | 169.0 | 155.9 (-13.1) | 1.57 → 1.48 ms |
| sidebar | 64 | 22,186 | 24,320 (1.10×) | 166.6 | 153.2 (-13.4) | 5.02 → 4.50 ms |
| search | 1 | 5,758 | 6,928 (1.20×) | 165.2 | 143.2 (-22.0) | 0.28 → 0.23 ms |
| search | 16 | 22,490 | 24,652 (1.10×) | 161.7 | 147.1 (-14.6) | 1.50 → 1.39 ms |
| search | 64 | 23,128 | 25,751 (1.11×) | 161.6 | 145.3 (-16.3) | 4.57 → 4.20 ms |
| post_message | 1 | 1,856 | 1,913 (1.03×) | 551.7 | 552.4 (+0.7) | 1.83 → 1.81 ms |
| post_message | 16 | 5,343 | 5,104 (0.96×) | 552.9 | 564.5 (+11.6) | 8.01 → 7.96 ms |
| post_message | 64 | 5,471 | 5,408 (0.99×) | 547.4 | 556.9 (+9.5) | 19.63 → 23.61 ms |

Page routes save 12–25 µs of CPU per request at 16 and 64 clients (7–13%) and serve 5–13% more
requests. room_show is 1.11× at 16 clients and 1.13× at 64. With one user in one room the text map
hits on every request, so this is an upper bound (see above). post_message renders no page and is
unchanged within noise.
