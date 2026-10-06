# Refresh through `(room_id, updated_at)`: before and after (2026-10-06)

Measures `index_messages_on_room_id_and_updated_at` (added on boot) with `page_updated_since` sorting on
`+created_at`, against `main` at `ccece30`.

## Setup

- A laptop with an AMD Ryzen 9 7940HX, Docker Desktop on Windows 11. The app's release image, built from
  `Dockerfile`, on CPUs 8–11, with `RAILS_ENV=production`; one image at a time.
- The database from basecamp/once-campfire#333: just over 2,000,000 messages in five rooms, room 1 with 1,000,001
  of them and 10,003 members, 10,008 users, after an `ANALYZE`. Its seed and the latency probe are in
  [this gist](https://gist.github.com/namespaceMarcello/d45418b37887134fb22d526a69631826). `main` and this change
  ran one after the other on the same copy; `main` first, so the copy had no `updated_at` index when it booted, and
  this change created it at its boot.

## Request

`GET /rooms/1/refresh?since=<a minute ago>` (nothing updated since), one request at a time after 3 warm-ups, median of
11. Raw: [`latency.jsonl`](latency.jsonl).

| | `main` | this change |
|---|---:|---:|
| median | 217.8 ms | 1.3 ms |
| min – max | 211.3 – 226.1 ms | 1.0 – 1.8 ms |

## Queries (sqlite3, room 1)

[`queries.sh`](queries.sh) on a copy of the same database, output in [`queries.txt`](queries.txt). Each query leaves
out the 40 ids `page_created_since` finds, as the controller does; median of 5.

| Messages in room 1 updated since `since` | `main` | this change |
|---:|---:|---:|
| 0 | 214 ms | < 1 ms |
| 4 | 220 ms | < 1 ms |
| 1,004 | < 1 ms | 1 ms |
| 100,004 | < 1 ms | 37 ms |
| 1,000,001 (`since=0`) | < 1 ms | 365 ms |

Both return the same rows in every case. `main` walks the room by `(room_id, created_at)` until it has 40 updated
rows, so it's slow when few are; this change finds the updated rows through the index and sorts them, so it's slow
when many are. The browser sends the later of the page's load time and the newest `updated_at` it shows.

Creating the index took 0.9 s, and it takes 70.2 MB of the 707.5 MB database. 10,000 inserts in one transaction took
22 ms with it and 14 ms without; 10,000 touches, 202 ms and 173 ms (medians of 3, rolled back).
