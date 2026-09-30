# Positional columns for Message, Membership and Room, 2026-09-30

`Message`, `Membership` and `Room` used to read each field by name (`row.get("created_at")`) from
a `SELECT *`. rusqlite finds a name by comparing it with each column's name in turn, so every
field of every row paid for a scan. They now select an explicit column list and read by position,
and one `sql::columns!` list per model generates both. The sidebar's join used to alias the room's
columns as `r_*`, and each of those lookups first scanned the nine memberships columns. It now
reads the room at an offset. `Statement::column_index` was 5.0% of messages_page, 4.0% of
room_show and 3.6% of the sidebar in the profile-20260929 folded stacks.

## In-process timing

`cargo test --release -p campfire_db --lib -- --ignored --nocapture row_reading_timing`
(`crates/db/src/tests/columns_test.rs`) builds an in-memory database with 200 messages in one room
and one user in 12 rooms, then times the model calls that the room page, the messages page and
the sidebar make. Each figure is the median of 7 runs of the mean time per call. The release test
binaries of `ce4f87a` (before: `d93d3dd` plus the timing) and `b4e57d8` (after) ran alternately,
five times each. Other agents were compiling at the time, and the 1-minute load average was
7–11 on 32 threads. Raw figures: [`runs.txt`](runs.txt).

| Call | Before, median [range] | After, median [range] | Change |
|---|---|---|---|
| `Message::last_page`, 40 rows (room_show) | 18.59 µs [17.66–18.75] | 13.12 µs [12.54–13.42] | −29% |
| `Message::page_before`, 40 rows (messages_page) | 18.81 µs [18.32–19.22] | 13.87 µs [12.99–14.14] | −26% |
| `Membership::visible_with_ordered_room`, 12 rows (sidebar) | 18.79 µs [18.24–19.05] | 9.82 µs [9.32–10.59] | −48% |
| `Message::find`, 1 row | 609 ns [598–644] | 500 ns [474–516] | −18% |
| `Room::find`, 1 row | 601 ns [597–643] | 475 ns [465–499] | −21% |

That is about 140 ns saved per message row and 750 ns per sidebar row (a membership with its
room). Single-row finds get faster too: the SQL is still a `&'static str` built by `concat!`, so
nothing is formatted per call.

## HTTP A/B

Measured over HTTP alongside the other page-path branches, in
[`hot-paths-http-20260930`](../hot-paths-http-20260930/summary.md): `main` at `0dbd10d` (the same code
as `d93d3dd`) against `6644d5f`, native release builds interleaved over 5 reps. Another project
raised the host's load average from 5 to 16 during the run. At one client, req/s moves by up to 20%
between reps of the same build. Medians:

| Route | Clients | main req/s | branch req/s | main CPU µs/req | branch CPU µs/req | p99 main → branch |
|---|---|---|---|---|---|---|
| room_show | 1 | 4,922 | 4,824 (0.98×) | 196.6 | 199.8 (+3.2) | 0.28 → 0.30 ms |
| room_show | 16 | 19,496 | 19,832 (1.02×) | 195.9 | 190.0 (-5.9) | 1.56 → 1.54 ms |
| room_show | 64 | 19,281 | 19,367 (1.00×) | 195.9 | 190.4 (-5.5) | 5.64 → 5.40 ms |
| messages_page | 1 | 5,503 | 5,082 (0.92×) | 179.2 | 182.2 (+3.0) | 0.27 → 0.34 ms |
| messages_page | 16 | 22,442 | 22,539 (1.00×) | 164.4 | 159.2 (-5.2) | 1.53 → 1.54 ms |
| messages_page | 64 | 22,514 | 23,624 (1.05×) | 164.6 | 159.8 (-4.8) | 4.84 → 4.46 ms |
| sidebar | 1 | 4,747 | 5,990 (1.26×) | 193.0 | 160.3 (-32.7) | 0.33 → 0.25 ms |
| sidebar | 16 | 21,485 | 23,171 (1.08×) | 169.0 | 159.1 (-9.9) | 1.57 → 1.47 ms |
| sidebar | 64 | 22,186 | 22,927 (1.03×) | 166.6 | 159.8 (-6.8) | 5.02 → 4.83 ms |
| search | 1 | 5,758 | 6,120 (1.06×) | 165.2 | 161.4 (-3.8) | 0.28 → 0.28 ms |
| search | 16 | 22,490 | 22,444 (1.00×) | 161.7 | 160.7 (-1.0) | 1.50 → 1.54 ms |
| search | 64 | 23,128 | 23,491 (1.02×) | 161.6 | 159.7 (-1.9) | 4.57 → 4.45 ms |
| post_message | 1 | 1,856 | 1,871 (1.01×) | 551.7 | 570.8 (+19.1) | 1.83 → 1.84 ms |
| post_message | 16 | 5,343 | 5,198 (0.97×) | 552.9 | 564.6 (+11.7) | 8.01 → 8.70 ms |
| post_message | 64 | 5,471 | 5,154 (0.94×) | 547.4 | 547.6 (+0.2) | 19.63 → 27.52 ms |

At 16 and 64 clients, room_show and messages_page save about 5 µs of CPU per request, and the
sidebar 7–10 µs (3–6%). That is what the in-process row timings predict: about 140 ns per message
row over a 40-message page, and 750 ns per sidebar row over 12. Throughput changes of that size are
within this run's noise. So are the one-client rows and post_message, which moves in both
directions between reps of the same build.
