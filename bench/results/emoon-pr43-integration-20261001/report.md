```
date: 2026-10-01T13:33:59+0200
host: 7.2.5-4-omarchy, 32 threads
server cpus: 8-11; loadgen cpus: 12-15
image: campfire-rust:app sha256:7042897243ae9e9f6fc47c3e18d0ae00165de1ad8a3488369bd79d01a8d46ac7
native-main: /home/dhh/Work/basecamp/once-campfire-rust/target/review-emoon-main preload=
native-pr: /home/dhh/Work/basecamp/once-campfire-rust/target/review-emoon-pr preload=
env: WEB_CONCURRENCY=3 RAILS_MAX_THREADS=5 JOB_CONCURRENCY=3
routes: room_show,messages_page,sidebar,post_message; concs: 16,64; secs: 5; cable: ; app env:
user agent: (none)
baseline commit: 4419b5f68755e302d3683fb3a750c70ccbc05a58
PR commit: 1ea6d6f6b24fd21e7d01e69b7c92df5c380bcbde
compiler: rustc 1.98.1 (48a229cea 2026-09-01)
build: cargo build --release --locked -p campfire; workspace profile: lto=fat, codegen-units=1; no profile overrides
review-emoon-main sha256: 97e0c239d415b5bc7258f0cb382df2dd91165229a490cbc5e6fa72c9524a9875
review-emoon-pr sha256: efca000f8a97768a4be0e405a46fa2325e31e0fa5cbb12e64ded90b8318e12fa
CPU: AMD RYZEN AI MAX+ 395 w/ Radeon 8060S
command: NATIVE_MAIN_BIN=<baseline binary> NATIVE_PR_BIN=<PR binary> PORT=4490 BENCH_WORK_DIR=<isolated work directory> bench/attrib --configs native-main,native-pr --reps 3 --routes room_show,messages_page,sidebar,post_message --concs 16,64 --secs 5 --cable "" --out bench/results/emoon-pr43-integration-20261001
```

Reps per configuration: native-main 3, native-pr 3. Cells: median [min–max]; (×) is the gain over native-main (>1 is better).
Host load (1-min loadavg at start of each run): native-main 5.41/9.01/6.72, native-pr 7.33/8.73/7.09

### HTTP room_show

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 25,956 [25,806–26,114] | 34,930 [32,279–35,693] (1.35×) |
| c=16 p50 ms | 0.58 [0.58–0.59] | 0.45 [0.43–0.47] (1.31×) |
| c=16 p99 ms | 1.25 [1.24–1.25] | 0.81 [0.79–0.96] (1.54×) |
| c=64 req/s | 28,572 [28,428–28,669] | 35,843 [33,679–36,448] (1.25×) |
| c=64 p50 ms | 2.16 [2.15–2.18] | 1.74 [1.71–1.84] (1.24×) |
| c=64 p99 ms | 3.96 [3.94–4.00] | 3.10 [3.05–3.44] (1.28×) |
| c=16 campfire CPU ms/req | 0.14 [0.14–0.14] | 0.11 [0.11–0.12] (1.29×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.29 [0.29–0.29] | 0.41 [0.40–0.51] |
| c=16 campfire cores busy | 3.61 [3.61–3.61] | 3.76 [3.76–3.76] |
| c=64 campfire CPU ms/req | 0.13 [0.13–0.13] | 0.10 [0.10–0.11] (1.26×) |
| c=64 thrust CPU ms/req | – | – |
| c=64 docker-proxy CPU ms/req | – | – |
| c=64 loadgen cores busy | 0.33 [0.33–0.34] | 0.42 [0.41–0.51] |
| c=64 campfire cores busy | 3.79 [3.78–3.79] | 3.76 [3.74–3.76] |
| avg response bytes | 24,231 [24,231–24,231] | 24,231 [24,231–24,231] |

### HTTP messages_page

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 30,292 [30,130–30,453] | 40,589 [37,385–41,060] (1.34×) |
| c=16 p50 ms | 0.50 [0.50–0.50] | 0.39 [0.38–0.41] (1.30×) |
| c=16 p99 ms | 1.05 [1.03–1.06] | 0.66 [0.65–0.78] (1.59×) |
| c=64 req/s | 33,590 [33,417–33,714] | 41,213 [38,692–41,697] (1.23×) |
| c=64 p50 ms | 1.82 [1.82–1.83] | 1.52 [1.50–1.61] (1.20×) |
| c=64 p99 ms | 3.33 [3.33–3.34] | 2.59 [2.56–2.90] (1.28×) |
| c=16 campfire CPU ms/req | 0.12 [0.12–0.12] | 0.092 [0.091–0.100] (1.29×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.33 [0.33–0.33] | 0.44 [0.43–0.57] |
| c=16 campfire cores busy | 3.61 [3.58–3.62] | 3.73 [3.73–3.73] |
| c=64 campfire CPU ms/req | 0.11 [0.11–0.11] | 0.090 [0.089–0.096] (1.25×) |
| c=64 thrust CPU ms/req | – | – |
| c=64 docker-proxy CPU ms/req | – | – |
| c=64 loadgen cores busy | 0.38 [0.37–0.38] | 0.46 [0.45–0.57] |
| c=64 campfire cores busy | 3.77 [3.75–3.77] | 3.72 [3.72–3.73] |
| avg response bytes | 16,158 [16,158–16,158] | 16,158 [16,158–16,158] |

### HTTP sidebar

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 29,575 [29,557–29,667] | 33,611 [31,319–34,069] (1.14×) |
| c=16 p50 ms | 0.50 [0.50–0.50] | 0.46 [0.45–0.49] (1.10×) |
| c=16 p99 ms | 1.17 [1.16–1.17] | 0.87 [0.85–0.96] (1.35×) |
| c=64 req/s | 33,769 [33,500–33,799] | 34,822 [33,774–35,029] (1.03×) |
| c=64 p50 ms | 1.82 [1.81–1.82] | 1.79 [1.76–1.84] (1.02×) |
| c=64 p99 ms | 3.44 [3.44–3.55] | 3.33 [3.31–3.45] (1.03×) |
| c=16 campfire CPU ms/req | 0.12 [0.12–0.12] | 0.11 [0.11–0.12] (1.07×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.31 [0.31–0.31] | 0.35 [0.35–0.47] |
| c=16 campfire cores busy | 3.56 [3.56–3.58] | 3.78 [3.78–3.78] |
| c=64 campfire CPU ms/req | 0.11 [0.11–0.11] | 0.11 [0.11–0.11] (1.03×) |
| c=64 thrust CPU ms/req | – | – |
| c=64 docker-proxy CPU ms/req | – | – |
| c=64 loadgen cores busy | 0.37 [0.36–0.38] | 0.37 [0.37–0.37] |
| c=64 campfire cores busy | 3.76 [3.75–3.78] | 3.77 [3.75–3.78] |
| avg response bytes | 5,910 [5,910–5,910] | 5,910 [5,910–5,910] |

### HTTP post_message

| Metric | native-main | native-pr |
|---|---|---|
| c=16 req/s | 6,078 [5,711–6,193] | 6,848 [6,710–6,896] (1.13×) |
| c=16 p50 ms | 2.38 [2.32–2.52] | 2.04 [2.03–2.10] (1.17×) |
| c=16 p99 ms | 6.98 [6.97–7.55] | 6.49 [6.26–6.55] (1.08×) |
| c=64 req/s | 6,348 [6,129–6,495] | 6,872 [6,784–6,898] (1.08×) |
| c=64 p50 ms | 9.81 [9.56–10.1] | 8.95 [8.95–9.12] (1.10×) |
| c=64 p99 ms | 15.8 [15.7–17.0] | 14.1 [13.9–14.5] (1.12×) |
| c=16 campfire CPU ms/req | 0.45 [0.44–0.47] | 0.38 [0.38–0.39] (1.16×) |
| c=16 thrust CPU ms/req | – | – |
| c=16 docker-proxy CPU ms/req | – | – |
| c=16 loadgen cores busy | 0.10 [0.10–0.13] | 0.11 [0.11–0.11] |
| c=16 campfire cores busy | 2.71 [2.66–2.73] | 2.62 [2.61–2.65] |
| c=64 campfire CPU ms/req | 0.44 [0.43–0.46] | 0.39 [0.39–0.40] (1.14×) |
| c=64 thrust CPU ms/req | – | – |
| c=64 docker-proxy CPU ms/req | – | – |
| c=64 loadgen cores busy | 0.12 [0.11–0.16] | 0.12 [0.12–0.12] |
| c=64 campfire cores busy | 2.82 [2.80–2.82] | 2.67 [2.65–2.71] |
| avg response bytes | 1,993 [1,992–1,993] | 1,992 [1,992–1,993] |
