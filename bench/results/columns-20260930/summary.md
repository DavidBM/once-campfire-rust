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

To be added by the coordinator: `bench/attrib` over room_show, messages_page, sidebar and search
(c=1 and c=16), with main against `perf-columns`.
