# Hot paths over HTTP: four branches against main, 2026-09-30

Plan step 10's page-path branches, each measured against `main` at `0dbd10d`. Its code is the same
as the branches' base, `d93d3dd`: after `d93d3dd` it only gained Ruby oracle vectors. Five native
release builds, interleaved per rep, each on a fresh copy of the seed:

| Config | Branch | Commit |
|---|---|---|
| main | `main` | `0dbd10d` |
| routing | `perf-routing` | `fb92455` (`is_match`, then one `RegexSet` per verb) |
| splice | `perf-splice` | `68444f7` (page parts: locate window, presized buffer, text identity by bytes, predecessors by SHA, two-generation FRAGMENTS) |
| db-hops | `perf-db-hops` | `6517414` (reader threads, merged reader trips, no read-back after a text post) |
| columns | `perf-columns` | `6644d5f` (positional columns for Message, Membership and Room) |

The command:

    bench/attrib --configs native-main,native-routing,native-splice,native-dbhops,native-columns \
      --reps 5 --routes room_show,messages_page,sidebar,search,post_message --concs 1,16,64 --secs 5 --cable ""

Another project's load raised the host's 1-minute load average from 5 to 16 over the run
([`env.txt`](env.txt), and `load_start` in each JSON). The last two reps of every build are the
noisiest, so read each cell against its rep range in [`report.md`](report.md). At one client, a
route's req/s moves by up to 20% between reps of the same build. At 16 and 64 clients the spread
is a few percent, and CPU per request is steadier than req/s.

Medians of the five reps. Each branch cell gives its req/s against main and the change in the app's
CPU per request (from `cpu_ms_per_req`, which [`report.md`](report.md) rounds to 10 µs):

| Route | Clients | main | routing | splice | db-hops | columns |
|---|---|---|---|---|---|---|
| room_show | 1 | 4,922 req/s, 197 µs | 0.98×, +4 µs | 1.11×, -21 µs | 1.01×, -7 µs | 0.98×, +3 µs |
| room_show | 16 | 19,496 req/s, 196 µs | 1.00×, -2 µs | 1.11×, -23 µs | 1.04×, -19 µs | 1.02×, -6 µs |
| room_show | 64 | 19,281 req/s, 196 µs | 1.01×, -2 µs | 1.13×, -25 µs | 1.16×, -26 µs | 1.00×, -5 µs |
| messages_page | 1 | 5,503 req/s, 179 µs | 1.03×, -5 µs | 1.10×, -16 µs | 1.06×, -17 µs | 0.92×, +3 µs |
| messages_page | 16 | 22,442 req/s, 164 µs | 1.00×, -1 µs | 1.05×, -12 µs | 1.12×, -19 µs | 1.00×, -5 µs |
| messages_page | 64 | 22,514 req/s, 165 µs | 1.01×, +1 µs | 1.09×, -12 µs | 1.22×, -29 µs | 1.05×, -5 µs |
| sidebar | 1 | 4,747 req/s, 193 µs | 1.17×, -19 µs | 1.27×, -34 µs | 1.14×, -27 µs | 1.26×, -33 µs |
| sidebar | 16 | 21,485 req/s, 169 µs | 1.02×, -1 µs | 1.08×, -13 µs | 1.11×, -22 µs | 1.08×, -10 µs |
| sidebar | 64 | 22,186 req/s, 167 µs | 1.03×, -3 µs | 1.10×, -13 µs | 1.22×, -29 µs | 1.03×, -7 µs |
| search | 1 | 5,758 req/s, 165 µs | 1.08×, -4 µs | 1.20×, -22 µs | 1.21×, -29 µs | 1.06×, -4 µs |
| search | 16 | 22,490 req/s, 162 µs | 1.01×, -2 µs | 1.10×, -15 µs | 1.18×, -34 µs | 1.00×, -1 µs |
| search | 64 | 23,128 req/s, 162 µs | 1.02×, -3 µs | 1.11×, -16 µs | 1.38×, -46 µs | 1.02×, -2 µs |
| post_message | 1 | 1,856 req/s, 552 µs | 0.97×, +32 µs | 1.03×, +1 µs | 1.08×, -30 µs | 1.01×, +19 µs |
| post_message | 16 | 5,343 req/s, 553 µs | 0.98×, -1 µs | 0.96×, +12 µs | 1.05×, -98 µs | 0.97×, +12 µs |
| post_message | 64 | 5,471 req/s, 547 µs | 0.99×, +3 µs | 0.99×, +10 µs | 1.15×, -103 µs | 0.94×, +0 µs |

What holds up against the noise:
- **db-hops** is the largest change. Every read route saves 19–46 µs of CPU per request at 16 and
  64 clients, and throughput rises 4–38%: search is +18% and +38%, messages_page and sidebar +22%
  at 64 clients. post_message saves about 100 µs per request.
- **splice** saves 12–25 µs per page request (7–13% of its CPU), for 5–13% more throughput on
  room_show, messages_page, sidebar and search at 16 and 64 clients. post_message doesn't render a
  page and is unchanged within noise.
- **columns** saves about 5–10 µs per request on room_show, messages_page and sidebar at 16 and
  64 clients (3–6%). That matches `column_index`'s 4–5% share in `profile-20260929`. Its
  throughput changes, and every post_message cell, are within noise.
- **routing** saves 1–3 µs per request at 16 and 64 clients, the 0.8–2.4 µs its in-process timing
  predicted: about 1%, below what this host can resolve in req/s.
- The sidebar at one client favours every branch by 14–27%. That comes from main: its last three
  reps there ran at the highest load of the run. Discount that row.
