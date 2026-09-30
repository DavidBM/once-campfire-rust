```
date: 2026-09-30T10:17:01+0200
host: 7.2.5-4-omarchy, 32 threads
server cpus: 8-11; loadgen cpus: 12-15
image: campfire-rust:app sha256:7042897243ae9e9f6fc47c3e18d0ae00165de1ad8a3488369bd79d01a8d46ac7
native-main: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-main/release/campfire preload=
native-ua: /home/dhh/Work/basecamp/once-campfire-rust/target/prof-ua/release/campfire preload=
env: WEB_CONCURRENCY=3 RAILS_MAX_THREADS=5 JOB_CONCURRENCY=3
routes: room_show,post_message; concs: 1,16; secs: 5; cable: ; app env: 
user agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36
```

Reps per configuration: native-main 5, native-ua 5. Cells: median [min–max]; (×) is the gain over native-main (>1 is better).
Host load (1-min loadavg at start of each run): native-main 5.99/7.47/13.58/11.21/9.61, native-ua 7.23/7.70/11.75/10.22/11.27

### HTTP room_show

| Metric | native-main | native-ua |
|---|---|---|
| c=1 req/s | 4,355 [4,032–4,613] | 4,327 [3,464–4,702] (0.99×) |
| c=1 p50 ms | 0.22 [0.21–0.22] | 0.21 [0.20–0.25] (1.01×) |
| c=1 p99 ms | 0.35 [0.31–0.48] | 0.38 [0.30–0.66] (0.92×) |
| c=16 req/s | 18,013 [16,244–18,532] | 17,970 [15,240–18,956] (1.00×) |
| c=16 p50 ms | 0.86 [0.83–0.94] | 0.84 [0.82–0.95] (1.02×) |
| c=16 p99 ms | 1.71 [1.67–1.99] | 1.82 [1.61–2.26] (0.94×) |
| c=1 campfire CPU ms/req | 0.22 [0.21–0.23] | 0.22 [0.21–0.26] (1.01×) |
| c=1 thrust CPU ms/req | – | – |
| c=1 docker-proxy CPU ms/req | – | – |
| c=1 loadgen cores busy | 0.050 [0.050–0.050] | 0.050 [0.050–0.060] |
| c=1 campfire cores busy | 0.96 [0.93–0.97] | 0.96 [0.89–0.97] |
| c=16 campfire CPU ms/req | 0.21 [0.20–0.23] | 0.21 [0.20–0.23] (1.02×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.22 [0.21–0.27] | 0.22 [0.22–0.25] |
| c=16 campfire cores busy | 3.77 [3.67–3.80] | 3.71 [3.53–3.79] |
| avg response bytes | 24,252 [24,252–24,252] | 24,252 [24,252–24,252] |

### HTTP post_message

| Metric | native-main | native-ua |
|---|---|---|
| c=1 req/s | 1,921 [1,094–1,973] | 1,920 [1,407–2,140] (1.00×) |
| c=1 p50 ms | 0.47 [0.46–0.81] | 0.46 [0.43–0.57] (1.02×) |
| c=1 p99 ms | 1.92 [1.68–2.08] | 1.82 [1.62–2.62] (1.05×) |
| c=16 req/s | 4,882 [3,832–5,168] | 5,033 [4,430–5,450] (1.03×) |
| c=16 p50 ms | 2.81 [2.78–3.42] | 2.87 [2.70–3.06] (0.98×) |
| c=16 p99 ms | 9.59 [8.74–20.0] | 8.86 [7.54–13.4] (1.08×) |
| c=1 campfire CPU ms/req | 0.56 [0.54–0.85] | 0.55 [0.52–0.68] (1.03×) |
| c=1 thrust CPU ms/req | – | – |
| c=1 docker-proxy CPU ms/req | – | – |
| c=1 loadgen cores busy | 0.040 [0.030–0.060] | 0.040 [0.030–0.050] |
| c=1 campfire cores busy | 1.07 [0.93–1.07] | 1.05 [0.96–1.10] |
| c=16 campfire CPU ms/req | 0.55 [0.54–0.66] | 0.56 [0.53–0.58] (0.98×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.080 [0.080–0.13] | 0.10 [0.090–0.10] |
| c=16 campfire cores busy | 2.65 [2.52–2.80] | 2.80 [2.58–2.90] |
| avg response bytes | 1,992 [1,991–1,992] | 1,992 [1,991–1,992] |
