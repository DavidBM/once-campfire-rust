# Route recognition: `is_match`, then one `RegexSet` per verb, 2026-09-30

`recognize` (`crates/campfire/src/controllers.rs`) timed in-process by its `times_recognition`
harness, an ignored test run in a release build:
`cargo test --release -p campfire --bin campfire -- --ignored --nocapture times_recognition`.
Each figure is the median of 9 samples of 200,000 `GET` recognitions of one path, on a thread that
takes each regex's cache from the shared pool, as the server's workers do. Three binaries, run
interleaved three times:

- `base`: `d93d3dd` plus the harness. It calls `captures` on each route in turn.
- `is_match`: `3124ea8`, which checks `is_match` before `captures`.
- `RegexSet`: `6801941`, one set per verb. The branch tip, `fb92455`, only adds test paths.

Other agents' cargo builds were running (1-minute load 2.4–11.6). Every rep is in
[`runs.txt`](runs.txt). Below: the median of the three reps' medians in ns per call, with the
range across reps.

| Path | base | is_match | RegexSet | Saved |
|---|---|---|---|---|
| room_show `/rooms/12` | 1,049 (1,017–1,077) | 425 (424–442) | 246 (244–263) | 0.80 µs, 4.3× |
| messages_page `/rooms/12/messages` | 1,382 (1,374–1,459) | 839 (824–877) | 271 (266–286) | 1.11 µs, 5.1× |
| search `/searches` | 1,393 (1,393–1,406) | 514 (510–531) | 211 (210–217) | 1.18 µs, 6.6× |
| `/up` | 1,227 (1,213–1,227) | 345 (332–366) | 194 (193–196) | 1.03 µs, 6.3× |
| 404 `/wp-login.php` | 2,043 (2,026–2,169) | 708 (704–710) | 47 (47–47) | 2.00 µs, 43× |
| Active Storage representation, 310 bytes | 6,435 (6,385–6,550) | 5,064 (4,878–5,075) | 2,801 (2,800–2,835) | 3.6 µs, 2.3× |
| Active Storage blob, 2.3 KB filename | 33,156 (33,010–33,339) | 34,570 (34,557–35,198) | 32,565 (32,458–32,626) | 1.02× |

Capturing and unescaping the `*filename` glob dominates the long filename, and neither commit
touches that step. `is_match` alone made it 4% slower because its glob regex then runs twice; the
set removes the extra run.

## The in-process saving is a lower bound

In [`profile-20260929`](../profile-20260929/report.md) (`main` at `9e2a110`, 16 clients),
`campfire::recognize` takes 1.63% of room_show's CPU, 1.67% of messages_page's and 1.42% of
search's, inclusive. At 16 clients those routes cost 0.195, 0.164 and 0.161 ms of server CPU per
request ([`wave3-20260930`](../wave3-20260930/summary.md), native-main). Recognition therefore cost
3.2, 2.7 and 2.3 µs per request in the server, 1.6–3× what the single uncontended thread above
measures. Two likely causes:

- The page work between requests cools the caches.
- The four workers share each regex's cache pool. The old loop drew from a pool for every route it
  tried (47 of them for `/rooms/:id`), and the set draws from two.

So the 0.8–1.2 µs per page above, 0.4–0.7% of a page's CPU, is a lower bound. If the server's cost
shrinks in the same proportion, the saving is about 2–2.4 µs, or 1.2–1.3% of CPU per page. Both
figures are inside this host's req/s noise of ±2–3%. The HTTP A/B should therefore read:

- CPU per request, from `cpu_ms_per_req` in the raw JSON. It has 0.1 µs resolution; `report.md`
  rounds to 10 µs.
- The share of `campfire::recognize` in a `bench/profile cpu` run, from
  `bench/lib/share.py cpu-<route>.folded 'recognize=;campfire::recognize(;|$)'`.