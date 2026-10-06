# Search newest first off the full-text index: before and after (2026-10-06)

Measures `search_reachable` ordering by `idx.rowid DESC` instead of `created_at DESC`, against `main` at `ccece30`.

## Setup

- A laptop with an AMD Ryzen 9 7940HX, Docker Desktop on Windows 11. The app's release image, built from
  `Dockerfile`, on CPUs 8–11, with `RAILS_ENV=production`; `wrk -t2 -c16` on CPUs 12–15, 5 s of warm-up discarded,
  then 20 s. One image at a time.
- The database from basecamp/once-campfire#333: just over 2,000,000 messages in five rooms (one with 1,000,001 and
  10,003 members), 10,008 users. `coffee` matches 40,000 messages and `message` nearly all of them. Its seed, the `wrk`
  driver and the latency probe are in
  [this gist](https://gist.github.com/namespaceMarcello/d45418b37887134fb22d526a69631826). `main` and this change ran
  one after the other on the same copy.

## Requests

`GET /searches?q=coffee` as a user in all five rooms. Raw: [`latency.jsonl`](latency.jsonl) (one request at a time
after 3 warm-ups, 11 measured) and [`throughput.jsonl`](throughput.jsonl) (`wrk`).

| | `main` | this change |
|---|---:|---:|
| one request at a time, median | 81.0 ms | 2.5 ms |
| requests/s, 16 clients | 12.6 | 4,390 |
| p50 / p99 under load | 1.23 s / 1.70 s | 3.39 ms / 7.64 ms |

Both return the same page: 100 messages, 938,112 bytes.

## Queries (sqlite3)

[`queries.sh`](queries.sh) on a copy of the same database, output in [`queries.txt`](queries.txt); median of 5.

| Search | searcher | `ORDER BY created_at` | `ORDER BY idx.rowid` |
|---|---|---:|---:|
| `coffee` | in all five rooms | 60 ms | < 1 ms |
| `message` | in all five rooms | 794 ms | < 1 ms |
| `coffee` | in none of them | 47 ms | 50 ms |
| `message` | in none of them | 294 ms | 508 ms |

Both orders return the same 100 messages. The plan loses `USE TEMP B-TREE FOR ORDER BY` and walks the index newest
first, stopping when the page is full. A searcher whose rooms hold fewer matches than a page never fills it, so the walk
reads the whole index, backwards, which took 1.7 times as long as the forward read and sort for `message`.
