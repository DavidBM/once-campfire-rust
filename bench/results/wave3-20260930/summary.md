# Code-review wave 3 (plan steps 6–9), 2026-09-30

Three native release builds, interleaved per rep:
- `native-main`: `main` at `8b807e4`.
- `native-stack`: `57f71b4`, the tip of the stacked branches. It holds:
  - the Ruby-drift fixes (#27)
  - one ActiveSupport::JSON encoder, password hashing, message verifier and Content-Disposition in `rails_compat`
  - one clock, with kit's `Crypto` trait removed
  - the `ruby_compat` crate, which now holds every ERB escaper, `to_i`/`to_f` and byte-range parser
- `native-errors`: `391a789`, the database-error helpers and the `?` sweep (#28).

The JSON encoder, the ERB escapers and the write transaction guard changed shape on hot paths.

`bench/attrib --configs native-main,native-stack,native-errors --reps 5 --routes
room_show,search,post_message --concs 1,16 --secs 5`. Another project kept the host's 1-minute
load average at 5–9 throughout.

| Route | Clients | main req/s | stack | errors |
|---|---|---|---|---|
| room_show | 1 | 4,885 | 4,906 (1.00×) | 4,902 (1.00×) |
| room_show | 16 | 19,445 | 19,537 (1.00×) | 19,522 (1.00×) |
| search | 1 | 6,344 | 6,322 (1.00×) | 6,423 (1.01×) |
| search | 16 | 23,135 | 23,245 (1.00×) | 23,056 (1.00×) |
| post_message | 1 | 2,128 | 2,156 (1.01×) | 2,117 (0.99×) |
| post_message | 16 | 5,314 | 5,407 (1.02×) | 5,388 (1.01×) |

Every median is within 2% of main. CPU per request agrees at the report's two-decimal resolution:
0.20 ms on room_show, 0.15–0.16 on search, 0.51–0.54 on post_message. The p99 medians
move by at most 7%, both ways, inside each build's rep-to-rep range. The largest is post_message
at c=1 on errors: 1.76 ms against 1.63, with ranges 1.61–1.80 and 1.55–1.78. The transaction
guard still sends `BEGIN IMMEDIATE`/`COMMIT`. Both branches are performance-neutral.

[`first-pass/`](first-pass/report.md) is the first run: 3 reps over all five routes. It had main
at 0.84–0.91× on room_show at c=1, because main's own reps there were noisy (2,500, 4,173 and 4,798
req/s); the 5-rep rerun above shows no regression in the branches. messages_page and sidebar
there are within 4% for both branches at both client counts. In the rerun, reps 2–3 (06:33–06:36)
overlapped a `cargo test` on the same host. They are the low ends of the ranges, for example
16,529 and 17,424 req/s on room_show at c=16, and the medians hold.

Full tables: [`report.md`](report.md). Its columns are in alphabetical order, so its ratios are
against native-errors, not main; the table above is recomputed against main.
