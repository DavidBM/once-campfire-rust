```
date: 2026-09-30T06:30:54+0200
host: 7.2.5-4-omarchy, 32 threads
server cpus: 8-11; loadgen cpus: 12-15
image: campfire-rust:app sha256:7042897243ae9e9f6fc47c3e18d0ae00165de1ad8a3488369bd79d01a8d46ac7
native-main: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-main/release/campfire preload=
native-stack: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-stack/release/campfire preload=
native-errors: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-errors/release/campfire preload=
env: WEB_CONCURRENCY=3 RAILS_MAX_THREADS=5 JOB_CONCURRENCY=3
routes: room_show,search,post_message; concs: 1,16; secs: 5; cable: ; app env:
```

Reps per configuration: native-errors 5, native-main 5, native-stack 5. Cells: median [min–max]; (×) is the gain over native-errors (>1 is better).
Host load (1-min loadavg at start of each run): native-errors 6.66/8.63/7.32/6.17/6.73, native-main 4.64/7.13/9.34/8.39/7.19, native-stack 7.12/8.70/8.98/7.95/6.06

### HTTP room_show

| Metric | native-errors | native-main | native-stack |
|---|---|---|---|
| c=1 req/s | 4,902 [4,746–4,988] | 4,885 [4,724–4,956] (1.00×) | 4,906 [4,772–4,924] (1.00×) |
| c=1 p50 ms | 0.20 [0.20–0.20] | 0.20 [0.20–0.21] (1.00×) | 0.20 [0.20–0.20] (1.00×) |
| c=1 p99 ms | 0.28 [0.27–0.30] | 0.28 [0.27–0.30] (0.99×) | 0.28 [0.27–0.32] (0.98×) |
| c=16 req/s | 19,522 [16,529–19,799] | 19,445 [18,495–19,542] (1.00×) | 19,537 [17,424–19,684] (1.00×) |
| c=16 p50 ms | 0.79 [0.79–0.84] | 0.80 [0.79–0.81] (1.00×) | 0.79 [0.79–0.84] (1.00×) |
| c=16 p99 ms | 1.55 [1.50–3.17] | 1.56 [1.54–1.72] (1.00×) | 1.58 [1.51–2.29] (0.98×) |
| c=1 campfire CPU ms/req | 0.20 [0.19–0.20] | 0.20 [0.20–0.20] (0.99×) | 0.20 [0.20–0.20] (1.00×) |
| c=1 thrust CPU ms/req | – | – | – |
| c=1 docker-proxy CPU ms/req | – | – | – |
| c=1 loadgen cores busy | 0.050 [0.050–0.050] | 0.050 [0.050–0.050] | 0.050 [0.050–0.050] |
| c=1 campfire cores busy | 0.97 [0.97–0.97] | 0.97 [0.96–0.97] | 0.97 [0.96–0.97] |
| c=16 campfire CPU ms/req | 0.19 [0.19–0.21] | 0.20 [0.19–0.20] (1.00×) | 0.20 [0.19–0.21] (1.00×) |
| c=16 thrust CPU ms/req | – | – | – |
| c=16 docker-proxy CPU ms/req | – | – | – |
| c=16 loadgen cores busy | 0.21 [0.19–0.22] | 0.21 [0.21–0.22] | 0.21 [0.21–0.22] |
| c=16 campfire cores busy | 3.80 [3.46–3.84] | 3.80 [3.69–3.82] | 3.79 [3.70–3.83] |
| avg response bytes | 24,231 [24,231–24,231] | 24,231 [24,231–24,231] | 24,231 [24,231–24,231] |

### HTTP search

| Metric | native-errors | native-main | native-stack |
|---|---|---|---|
| c=1 req/s | 6,423 [5,799–6,454] | 6,344 [5,790–6,411] (0.99×) | 6,322 [6,241–6,443] (0.98×) |
| c=1 p50 ms | 0.15 [0.15–0.15] | 0.15 [0.15–0.15] (0.99×) | 0.15 [0.15–0.15] (0.98×) |
| c=1 p99 ms | 0.22 [0.22–0.44] | 0.23 [0.22–0.33] (0.97×) | 0.23 [0.21–0.25] (0.94×) |
| c=16 req/s | 23,056 [18,707–23,193] | 23,135 [22,427–23,235] (1.00×) | 23,245 [23,014–23,266] (1.01×) |
| c=16 p50 ms | 0.66 [0.65–0.76] | 0.66 [0.65–0.67] (1.00×) | 0.65 [0.65–0.66] (1.01×) |
| c=16 p99 ms | 1.48 [1.42–2.42] | 1.45 [1.44–1.61] (1.01×) | 1.46 [1.44–1.50] (1.01×) |
| c=1 campfire CPU ms/req | 0.15 [0.15–0.17] | 0.16 [0.16–0.17] (0.99×) | 0.16 [0.15–0.16] (0.98×) |
| c=1 thrust CPU ms/req | – | – | – |
| c=1 docker-proxy CPU ms/req | – | – | – |
| c=1 loadgen cores busy | 0.060 [0.060–0.060] | 0.060 [0.060–0.060] | 0.060 [0.060–0.060] |
| c=1 campfire cores busy | 0.99 [0.97–1.00] | 0.99 [0.96–1.00] | 0.99 [0.99–1.00] |
| c=16 campfire CPU ms/req | 0.16 [0.16–0.18] | 0.16 [0.16–0.16] (1.00×) | 0.16 [0.16–0.16] (1.01×) |
| c=16 thrust CPU ms/req | – | – | – |
| c=16 docker-proxy CPU ms/req | – | – | – |
| c=16 loadgen cores busy | 0.24 [0.24–0.24] | 0.24 [0.24–0.24] | 0.24 [0.24–0.24] |
| c=16 campfire cores busy | 3.69 [3.46–3.75] | 3.70 [3.66–3.75] | 3.71 [3.66–3.74] |
| avg response bytes | 9,766 [9,766–9,766] | 9,766 [9,766–9,766] | 9,766 [9,766–9,766] |

### HTTP post_message

| Metric | native-errors | native-main | native-stack |
|---|---|---|---|
| c=1 req/s | 2,117 [2,100–2,185] | 2,128 [2,057–2,136] (1.01×) | 2,156 [2,107–2,241] (1.02×) |
| c=1 p50 ms | 0.43 [0.42–0.44] | 0.44 [0.43–0.45] (0.99×) | 0.43 [0.41–0.43] (1.01×) |
| c=1 p99 ms | 1.76 [1.61–1.80] | 1.63 [1.55–1.78] (1.08×) | 1.69 [1.68–1.72] (1.04×) |
| c=16 req/s | 5,388 [4,839–5,501] | 5,314 [4,515–5,476] (0.99×) | 5,407 [5,212–5,481] (1.00×) |
| c=16 p50 ms | 2.74 [2.69–2.77] | 2.77 [2.69–2.95] (0.99×) | 2.72 [2.71–2.79] (1.01×) |
| c=16 p99 ms | 7.21 [7.20–14.6] | 7.69 [7.28–9.78] (0.94×) | 7.34 [7.12–7.52] (0.98×) |
| c=1 campfire CPU ms/req | 0.52 [0.50–0.52] | 0.52 [0.52–0.53] (1.00×) | 0.51 [0.49–0.52] (1.02×) |
| c=1 thrust CPU ms/req | – | – | – |
| c=1 docker-proxy CPU ms/req | – | – | – |
| c=1 loadgen cores busy | 0.030 [0.030–0.030] | 0.030 [0.030–0.030] | 0.030 [0.030–0.030] |
| c=1 campfire cores busy | 1.10 [1.10–1.10] | 1.11 [1.08–1.11] | 1.10 [1.09–1.10] |
| c=16 campfire CPU ms/req | 0.54 [0.53–0.55] | 0.54 [0.53–0.59] (0.98×) | 0.53 [0.53–0.55] (1.00×) |
| c=16 thrust CPU ms/req | – | – | – |
| c=16 docker-proxy CPU ms/req | – | – | – |
| c=16 loadgen cores busy | 0.090 [0.080–0.090] | 0.090 [0.080–0.090] | 0.090 [0.090–0.090] |
| c=16 campfire cores busy | 2.89 [2.58–2.93] | 2.90 [2.65–2.92] | 2.89 [2.89–2.92] |
| avg response bytes | 1,993 [1,993–1,993] | 1,993 [1,992–1,993] | 1,993 [1,993–1,993] |
