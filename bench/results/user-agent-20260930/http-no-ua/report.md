```
date: 2026-09-30T10:21:04+0200
host: 7.2.5-4-omarchy, 32 threads
server cpus: 8-11; loadgen cpus: 12-15
image: campfire-rust:app sha256:7042897243ae9e9f6fc47c3e18d0ae00165de1ad8a3488369bd79d01a8d46ac7
native-main: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-main/release/campfire preload=
native-ua: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-ua/release/campfire preload=
env: WEB_CONCURRENCY=3 RAILS_MAX_THREADS=5 JOB_CONCURRENCY=3
routes: room_show; concs: 1,16; secs: 5; cable: ; app env: 
user agent: (none)
```

Reps per configuration: native-main 5, native-ua 5. Cells: median [min–max]; (×) is the gain over native-main (>1 is better).
Host load (1-min loadavg at start of each run): native-main 11.21/10.85/10.60/8.90/9.22, native-ua 10.20/11.26/10.15/9.50/9.53

### HTTP room_show

| Metric | native-main | native-ua |
|---|---|---|
| c=1 req/s | 4,652 [4,213–4,776] | 4,641 [4,298–4,794] (1.00×) |
| c=1 p50 ms | 0.20 [0.20–0.21] | 0.20 [0.20–0.21] (1.00×) |
| c=1 p99 ms | 0.34 [0.30–0.36] | 0.33 [0.30–0.39] (1.01×) |
| c=16 req/s | 19,113 [13,546–19,261] | 19,188 [18,651–19,682] (1.00×) |
| c=16 p50 ms | 0.81 [0.80–0.94] | 0.81 [0.79–0.82] (1.01×) |
| c=16 p99 ms | 1.62 [1.61–4.20] | 1.61 [1.55–1.79] (1.00×) |
| c=1 campfire CPU ms/req | 0.21 [0.20–0.22] | 0.21 [0.20–0.21] (1.00×) |
| c=1 thrust CPU ms/req | – | – |
| c=1 docker-proxy CPU ms/req | – | – |
| c=1 loadgen cores busy | 0.050 [0.050–0.050] | 0.050 [0.050–0.050] |
| c=1 campfire cores busy | 0.97 [0.92–0.97] | 0.96 [0.89–0.97] |
| c=16 campfire CPU ms/req | 0.20 [0.20–0.23] | 0.20 [0.19–0.20] (1.00×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.22 [0.21–0.22] | 0.22 [0.21–0.22] |
| c=16 campfire cores busy | 3.79 [3.10–3.80] | 3.79 [3.74–3.80] |
| avg response bytes | 24,231 [24,231–24,231] | 24,231 [24,231–24,231] |
