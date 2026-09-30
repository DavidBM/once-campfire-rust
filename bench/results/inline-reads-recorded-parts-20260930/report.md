```
date: 2026-09-30T17:57:41+0200
host: 7.2.3-arch1-3, 32 threads
server cpus: 8-11; loadgen cpus: 12-15
image: campfire-rust:app sha256:776069d599316097c45bdd60306bfee9538e5a0c869e623914eccb7f56a8c282
native-main: /home/emoon/once-campfire-rust/target/h2h-upstream preload=
native-pr: /home/emoon/once-campfire-rust/target/h2h-trim preload=
env: WEB_CONCURRENCY=3 RAILS_MAX_THREADS=5 JOB_CONCURRENCY=3
routes: room_show,messages_page,sidebar,post_message; concs: 16; secs: 10; cable: ; app env: 
user agent: (none)
```

Reps per configuration: native-main 5, native-pr 5. Cells: median [min–max]; (×) is the gain over native-main (>1 is better).
Host load (1-min loadavg at start of each run): native-main 1.56/5.33/6.28/6.14/6.81, native-pr 4.99/6.18/6.78/7.96/7.59

### HTTP room_show

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 28,540 [27,589–28,863] | 41,411 [41,266–41,711] (1.45×) |
| c=16 p50 ms | 0.52 [0.52–0.54] | 0.37 [0.37–0.38] (1.40×) |
| c=16 p99 ms | 1.22 [1.22–1.31] | 0.69 [0.69–0.70] (1.77×) |
| c=16 campfire CPU ms/req | 0.12 [0.12–0.12] | 0.091 [0.091–0.092] (1.31×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.29 [0.29–0.30] | 0.42 [0.41–0.43] |
| c=16 campfire cores busy | 3.43 [3.32–3.44] | 3.79 [3.78–3.79] |
| avg response bytes | 24,231 [24,231–24,231] | 24,231 [24,231–24,231] |

### HTTP messages_page

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 33,498 [32,991–33,665] | 47,926 [47,646–48,055] (1.43×) |
| c=16 p50 ms | 0.44 [0.44–0.45] | 0.33 [0.32–0.33] (1.36×) |
| c=16 p99 ms | 1.04 [1.02–1.07] | 0.56 [0.56–0.57] (1.85×) |
| c=16 campfire CPU ms/req | 0.10 [0.10–0.10] | 0.078 [0.078–0.079] (1.31×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.33 [0.33–0.34] | 0.46 [0.45–0.47] |
| c=16 campfire cores busy | 3.44 [3.43–3.46] | 3.76 [3.75–3.76] |
| avg response bytes | 16,158 [16,158–16,158] | 16,158 [16,158–16,158] |

### HTTP sidebar

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 32,998 [32,587–33,139] | 39,936 [39,574–40,552] (1.21×) |
| c=16 p50 ms | 0.45 [0.44–0.45] | 0.39 [0.38–0.39] (1.15×) |
| c=16 p99 ms | 1.13 [1.12–1.15] | 0.74 [0.72–0.75] (1.53×) |
| c=16 campfire CPU ms/req | 0.10 [0.10–0.10] | 0.095 [0.094–0.096] (1.08×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.31 [0.30–0.31] | 0.37 [0.36–0.37] |
| c=16 campfire cores busy | 3.40 [3.39–3.40] | 3.80 [3.80–3.80] |
| avg response bytes | 5,910 [5,910–5,910] | 5,910 [5,910–5,910] |

### HTTP post_message

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 4,825 [4,799–4,936] | 5,318 [4,922–5,348] (1.10×) |
| c=16 p50 ms | 1.99 [1.98–2.00] | 1.71 [1.68–1.71] (1.16×) |
| c=16 p99 ms | 46.2 [43.9–47.7] | 44.8 [43.8–54.6] (1.03×) |
| c=16 campfire CPU ms/req | 0.37 [0.37–0.37] | 0.31 [0.31–0.32] (1.18×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.070 [0.070–0.070] | 0.070 [0.070–0.070] |
| c=16 campfire cores busy | 1.79 [1.78–1.84] | 1.67 [1.55–1.69] |
| avg response bytes | 1,993 [1,993–1,993] | 1,993 [1,993–1,993] |
