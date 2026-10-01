```
date: 2026-10-01T20:38:33+02:00
host: 7.2.5-4-omarchy, AMD RYZEN AI MAX+ 395 w/ Radeon 8060S, 32 threads, 30GB
server cpus: 8-11 (nproc 4); loadgen cpus: 12-15; network: host
env: WEB_CONCURRENCY=3 JOB_CONCURRENCY=3 RAILS_MAX_THREADS=5
rust extra env:
user agent: (none)
reference image: campfire-reference:bench-898653e sha256:d91fdb852402e2d3393ca262e50b657ea0cf861fc7d4150aaf3773c91d41c2fa 2026-09-27T22:01:29.746709947+02:00
rust image: campfire-rust:bench-emoon-pr43 sha256:2ef6125fcd3f33531ef4c0bc9f14db1d6432d9517267bac956f7d268dd76f4e4 2026-10-01T20:36:46.055072504+02:00
rust HEAD: 1ea6d6f (dirty: 0 files)
```

Reps: reference 3, rust 3. Cells: median [min–max].

### Startup and memory

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
| cold start: docker run → /up 200 (ms) | 2,731 [2,608–2,901] | 157 [148–169] | 17.4× |
| idle memory.current (MB) | 393 [301–484] | 20.0 [15.0–31.0] | 19.6× |
| idle anon (MB) | 284 [282–299] | 13.0 [13.0–13.0] | 21.8× |
| peak memory.current under load (MB) | 3,356 [3,219–3,506] | 946 [929–1,005] | 3.5× |
| peak anon under load (MB) | 3,137 [2,975–3,352] | 381 [375–392] | 8.2× |

### HTTP (signed in as david; keep-alive; c = concurrent connections)

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
| room_show c=1 req/s | 92.0 [90.1–92.3] | 8,825 [8,738–9,032] | 95.9× |
| room_show c=1 p50 ms | 10.5 [10.2–10.8] | 0.11 [0.11–0.11] | 96.3× |
| room_show c=1 p99 ms | 13.6 [13.5–14.1] | 0.17 [0.17–0.17] | 80.8× |
| room_show c=16 req/s | 217 [204–222] | 36,120 [35,906–36,153] | 166.4× |
| room_show c=16 p50 ms | 71.2 [68.7–78.5] | 0.43 [0.43–0.43] | 166.0× |
| room_show c=16 p99 ms | 163 [146–181] | 0.79 [0.79–0.80] | 206.5× |
| room_show c=64 req/s | 183 [181–185] | 36,649 [36,577–37,032] | 200.2× |
| room_show c=64 p50 ms | 343 [336–353] | 1.70 [1.69–1.71] | 201.6× |
| room_show c=64 p99 ms | 463 [441–484] | 3.06 [3.04–3.08] | 151.3× |
| messages_page c=1 req/s | 168 [167–173] | 10,636 [10,590–10,713] | 63.2× |
| messages_page c=1 p50 ms | 5.62 [5.62–5.64] | 0.09 [0.09–0.09] | 62.5× |
| messages_page c=1 p99 ms | 10.8 [7.5–14.2] | 0.14 [0.14–0.14] | 76.0× |
| messages_page c=16 req/s | 403 [393–408] | 41,352 [41,085–41,375] | 102.5× |
| messages_page c=16 p50 ms | 36.9 [36.9–40.4] | 0.38 [0.38–0.38] | 97.9× |
| messages_page c=16 p99 ms | 86.6 [86.3–100.4] | 0.65 [0.65–0.66] | 133.2× |
| messages_page c=64 req/s | 375 [374–383] | 41,915 [41,733–42,048] | 111.7× |
| messages_page c=64 p50 ms | 162 [161–164] | 1.50 [1.49–1.50] | 108.3× |
| messages_page c=64 p99 ms | 261 [261–263] | 2.58 [2.57–2.58] | 101.4× |
| sidebar c=1 req/s | 213 [211–214] | 8,016 [7,964–8,031] | 37.6× |
| sidebar c=1 p50 ms | 4.50 [4.49–4.57] | 0.12 [0.12–0.12] | 37.5× |
| sidebar c=1 p99 ms | 6.64 [6.50–6.66] | 0.19 [0.19–0.20] | 34.2× |
| sidebar c=16 req/s | 524 [513–539] | 34,339 [34,181–34,449] | 65.5× |
| sidebar c=16 p50 ms | 30.5 [29.3–31.0] | 0.45 [0.45–0.45] | 68.0× |
| sidebar c=16 p99 ms | 57.1 [52.8–60.6] | 0.85 [0.85–0.86] | 66.9× |
| sidebar c=64 req/s | 502 [483–525] | 35,258 [35,188–35,381] | 70.2× |
| sidebar c=64 p50 ms | 117 [117–124] | 1.76 [1.76–1.77] | 66.6× |
| sidebar c=64 p99 ms | 246 [215–247] | 3.32 [3.31–3.33] | 74.0× |
| search c=1 req/s | 171 [169–171] | 9,840 [9,686–9,953] | 57.6× |
| search c=1 p50 ms | 5.68 [5.66–5.79] | 0.10 [0.10–0.10] | 57.9× |
| search c=1 p99 ms | 7.85 [7.79–7.94] | 0.15 [0.15–0.16] | 52.0× |
| search c=16 req/s | 376 [364–385] | 33,510 [33,446–33,589] | 89.1× |
| search c=16 p50 ms | 43.4 [40.4–43.5] | 0.44 [0.44–0.44] | 98.5× |
| search c=16 p99 ms | 72.4 [69.9–75.7] | 1.00 [1.00–1.01] | 72.1× |
| search c=64 req/s | 370 [353–378] | 38,836 [38,687–38,969] | 105.1× |
| search c=64 p50 ms | 171 [163–178] | 1.56 [1.55–1.56] | 109.4× |
| search c=64 p99 ms | 288 [229–302] | 3.14 [3.13–3.19] | 91.9× |
| avatar c=1 req/s | 28,356 [27,774–28,533] | 63,028 [62,186–63,730] | 2.2× |
| avatar c=1 p50 ms | 0.03 [0.03–0.03] | 0.01 [0.01–0.01] | 2.1× |
| avatar c=1 p99 ms | 0.09 [0.09–0.09] | 0.02 [0.02–0.02] | 4.6× |
| avatar c=16 req/s | 95,492 [93,870–96,791] | 389,632 [383,336–392,030] | 4.1× |
| avatar c=16 p50 ms | 0.10 [0.10–0.10] | 0.04 [0.04–0.04] | 2.7× |
| avatar c=16 p99 ms | 0.90 [0.88–0.91] | 0.08 [0.07–0.09] | 11.7× |
| avatar c=64 req/s | 76,361 [76,086–76,911] | 425,126 [415,666–426,909] | 5.6× |
| avatar c=64 p50 ms | 0.30 [0.30–0.30] | 0.14 [0.14–0.14] | 2.1× |
| avatar c=64 p99 ms | 5.02 [4.96–5.04] | 0.31 [0.30–0.33] | 16.3× |
| static_css c=1 req/s | 33,998 [33,950–34,524] | 67,764 [66,300–68,083] | 2.0× |
| static_css c=1 p50 ms | 0.03 [0.03–0.03] | 0.01 [0.01–0.01] | 1.9× |
| static_css c=1 p99 ms | 0.07 [0.07–0.07] | 0.02 [0.02–0.02] | 3.6× |
| static_css c=16 req/s | 129,800 [129,045–131,510] | 406,724 [405,332–425,595] | 3.1× |
| static_css c=16 p50 ms | 0.08 [0.08–0.09] | 0.04 [0.04–0.04] | 2.3× |
| static_css c=16 p99 ms | 0.66 [0.63–0.66] | 0.07 [0.06–0.07] | 9.6× |
| static_css c=64 req/s | 106,742 [106,045–107,162] | 434,614 [423,856–437,094] | 4.1× |
| static_css c=64 p50 ms | 0.29 [0.28–0.29] | 0.14 [0.14–0.14] | 2.0× |
| static_css c=64 p99 ms | 3.51 [3.48–3.52] | 0.30 [0.30–0.32] | 11.7× |
| up c=1 req/s | 1,794 [1,793–1,800] | 41,365 [41,320–41,379] | 23.1× |
| up c=1 p50 ms | 0.52 [0.52–0.53] | 0.02 [0.02–0.02] | 22.7× |
| up c=1 p99 ms | 1.10 [0.97–1.12] | 0.03 [0.03–0.03] | 34.3× |
| up c=16 req/s | 4,068 [3,961–4,079] | 233,085 [231,892–233,440] | 57.3× |
| up c=16 p50 ms | 3.80 [3.79–3.88] | 0.07 [0.07–0.07] | 55.9× |
| up c=16 p99 ms | 7.91 [7.82–8.32] | 0.12 [0.12–0.12] | 68.2× |
| up c=64 req/s | 4,023 [3,890–4,035] | 239,915 [238,220–243,215] | 59.6× |
| up c=64 p50 ms | 15.8 [15.7–16.3] | 0.26 [0.26–0.26] | 61.2× |
| up c=64 p99 ms | 24.2 [23.8–26.6] | 0.53 [0.53–0.54] | 45.5× |
| post_message c=1 req/s | 144 [142–146] | 2,787 [2,762–2,808] | 19.3× |
| post_message c=1 p50 ms | 6.46 [6.39–6.49] | 0.32 [0.32–0.33] | 20.0× |
| post_message c=1 p99 ms | 12.6 [12.6–13.6] | 1.71 [1.70–1.72] | 7.4× |
| post_message c=16 req/s | 269 [264–274] | 6,817 [6,772–6,875] | 25.4× |
| post_message c=16 p50 ms | 56.3 [49.1–60.1] | 2.06 [2.05–2.08] | 27.2× |
| post_message c=16 p99 ms | 156 [151–169] | 6.17 [6.15–6.29] | 25.2× |
| post_message c=64 req/s | 270 [260–271] | 6,840 [6,500–6,874] | 25.3× |
| post_message c=64 p50 ms | 237 [231–265] | 9.06 [9.04–9.49] | 26.1× |
| post_message c=64 p99 ms | 381 [351–408] | 13.9 [13.8–15.4] | 27.4× |

### HTTP errors / non-2xx-3xx (first rep, per app)

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
- reference: none
- rust: none

### Action Cable fan-out (one room; chatter.js subscriptions per client)

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
| 100 clients: subscribed | 100 [100–100] | 100 [100–100] | 1.0× |
| 100 clients: connect+subscribe all (s) | 0.27 [0.27–0.29] | 0.06 [0.06–0.06] | 4.5× |
| 100 clients: paced post→one client p50 ms | 14.6 [14.2–14.6] | 1.83 [1.78–1.88] | 8.0× |
| 100 clients: paced post→all clients p50 ms | 19.7 [19.7–20.3] | 2.06 [2.03–2.08] | 9.6× |
| 100 clients: paced post→all clients p99 ms | 61.9 [56.5–84.9] | 5.12 [3.29–12.93] | 12.1× |
| 100 clients: max sustained msgs/s (delivered to all) | 81.1 [79.7–81.6] | 3,811 [3,787–3,887] | 47.0× |
| 100 clients: deliveries/s (client×message) | 8,107 [7,966–8,161] | 381,065 [378,679–388,668] | 47.0× |
| 100 clients: saturated post→all p50 ms | 47.1 [43.6–49.5] | 1.13 [1.12–1.13] | 41.8× |
| 100 clients: saturated POST p50 ms | 41.3 [37.3–45.5] | 0.86 [0.85–0.87] | 47.7× |
| 1000 clients: subscribed | 1,000 [1,000–1,000] | 1,000 [1,000–1,000] | 1.0× |
| 1000 clients: connect+subscribe all (s) | 1.43 [1.40–1.46] | 0.13 [0.12–0.13] | 11.0× |
| 1000 clients: paced post→one client p50 ms | 44.5 [40.9–51.7] | 4.04 [3.97–4.05] | 11.0× |
| 1000 clients: paced post→all clients p50 ms | 97.8 [85.8–186.0] | 6.52 [6.52–6.56] | 15.0× |
| 1000 clients: paced post→all clients p99 ms | 201 [185–218] | 8.93 [8.25–9.58] | 22.5× |
| 1000 clients: max sustained msgs/s (delivered to all) | 11.5 [10.7–13.0] | 551 [551–564] | 47.9× |
| 1000 clients: deliveries/s (client×message) | 11,473 [10,705–13,014] | 551,187 [550,969–564,414] | 48.0× |
| 1000 clients: saturated post→all p50 ms | 1,659 [275–2,374] | 15.7 [15.3–15.8] | 105.7× |
| 1000 clients: saturated POST p50 ms | 168 [88–231] | 6.90 [6.85–6.91] | 24.3× |
| 5000 clients: subscribed | 5,000 [5,000–5,000] | 5,000 [5,000–5,000] | 1.0× |
| 5000 clients: connect+subscribe all (s) | 8.58 [8.50–8.59] | 0.51 [0.26–1.20] | 16.8× |
| 5000 clients: paced post→one client p50 ms | 393 [245–479] | 12.2 [11.7–12.4] | 32.3× |
| 5000 clients: paced post→all clients p50 ms | 1,818 [599–2,096] | 21.5 [20.8–22.2] | 84.6× |
| 5000 clients: paced post→all clients p99 ms | 3,467 [786–4,231] | 32.0 [31.9–36.6] | 108.2× |
| 5000 clients: max sustained msgs/s (delivered to all) | 2.30 [2.10–2.70] | 126 [126–126] | 54.8× |
| 5000 clients: deliveries/s (client×message) | 11,523 [10,443–13,620] | 629,848 [629,311–632,528] | 54.7× |
| 5000 clients: saturated post→all p50 ms | 4,039 [1,299–5,538] | 123 [120–125] | 32.8× |
| 5000 clients: saturated POST p50 ms | 408 [359–669] | 17.0 [16.1–17.1] | 24.0× |
| 10000 clients: subscribed | 10,000 [10,000–10,000] | 10,000 [10,000–10,000] | 1.0× |
| 10000 clients: connect+subscribe all (s) | 29.5 [29.4–29.9] | 1.42 [1.21–1.47] | 20.8× |
| 10000 clients: paced post→one client p50 ms | 815 [614–923] | 22.5 [21.1–22.5] | 36.3× |
| 10000 clients: paced post→all clients p50 ms | 1,823 [1,374–3,109] | 39.6 [38.8–41.0] | 46.0× |
| 10000 clients: paced post→all clients p99 ms | 2,644 [2,351–4,932] | 61.1 [57.1–75.0] | 43.3× |
| 10000 clients: max sustained msgs/s (delivered to all) | 0.90 [0.80–1.00] | 67.2 [65.1–67.8] | 74.7× |
| 10000 clients: deliveries/s (client×message) | 9,167 [8,350–9,679] | 671,674 [651,262–677,656] | 73.3× |
| 10000 clients: saturated post→all p50 ms | 4,227 [3,056–4,432] | 269 [256–306] | 15.7× |
| 10000 clients: saturated POST p50 ms | 3,830 [2,693–3,865] | 23.7 [23.1–23.8] | 161.7× |

### Upload + thumbnail (black_hole.jpg, 505 KB)

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
| POST with attachment (ms) | 107 [102–116] | 27.3 [26.8–28.1] | 3.9× |
| then GET thumb → 200 (ms) | 0.40 [0.40–0.40] | 0.30 [0.30–0.30] | 1.3× |
| POST → thumbnail served (ms) | 107 [103–116] | 27.6 [27.2–28.4] | 3.9× |

### Memory during cable fan-out, by process (MB, peak within the phase)

App process: Rails' Puma master and workers (Action Cable runs in them), or Rust's one campfire
process (its front server included). Pss counts pages shared between forked workers once;
RssAnon counts them in every process.

| Metric | Rails | Rust | Rust adv. |
|---|---|---|---|
| 100 clients, all subscribed, idle: app process Pss | 554 [553–565] | 122 [121–123] | 4.5× |
| 100 clients, all subscribed, idle: app process RssAnon | 700 [700–710] | 100 [100–102] | 7.0× |
| 100 clients, all subscribed, idle: app + Redis + Thruster Pss | 593 [592–605] | 122 [121–123] | 4.9× |
| 100 clients, all subscribed, idle: whole container Pss | 892 [875–906] | 122 [121–123] | 7.3× |
| 100 clients, saturated fan-out: app process Pss | 679 [669–701] | 121 [117–121] | 5.6× |
| 100 clients, saturated fan-out: app process RssAnon | 825 [815–844] | 99.5 [95.8–99.9] | 8.3× |
| 100 clients, saturated fan-out: app + Redis + Thruster Pss | 731 [720–753] | 121 [117–121] | 6.0× |
| 100 clients, saturated fan-out: whole container Pss | 1,032 [1,004–1,056] | 121 [117–121] | 8.5× |
| 1000 clients, all subscribed, idle: app process Pss | 657 [646–680] | 124 [123–127] | 5.3× |
| 1000 clients, all subscribed, idle: app process RssAnon | 801 [791–821] | 103 [102–106] | 7.8× |
| 1000 clients, all subscribed, idle: app + Redis + Thruster Pss | 758 [746–780] | 124 [123–127] | 6.1× |
| 1000 clients, all subscribed, idle: whole container Pss | 1,060 [1,029–1,083] | 124 [123–127] | 8.5× |
| 1000 clients, saturated fan-out: app process Pss | 898 [881–922] | 124 [124–128] | 7.3× |
| 1000 clients, saturated fan-out: app process RssAnon | 1,041 [1,021–1,067] | 102 [102–106] | 10.2× |
| 1000 clients, saturated fan-out: app + Redis + Thruster Pss | 1,007 [989–1,031] | 124 [124–128] | 8.1× |
| 1000 clients, saturated fan-out: whole container Pss | 1,308 [1,292–1,314] | 124 [124–128] | 10.6× |
| 5000 clients, all subscribed, idle: app process Pss | 1,002 [978–1,003] | 181 [180–184] | 5.5× |
| 5000 clients, all subscribed, idle: app process RssAnon | 1,140 [1,121–1,145] | 160 [159–163] | 7.1× |
| 5000 clients, all subscribed, idle: app + Redis + Thruster Pss | 1,380 [1,357–1,382] | 181 [180–184] | 7.6× |
| 5000 clients, all subscribed, idle: whole container Pss | 1,680 [1,638–1,682] | 181 [180–184] | 9.3× |
| 5000 clients, saturated fan-out: app process Pss | 1,796 [1,712–1,839] | 182 [181–183] | 9.9× |
| 5000 clients, saturated fan-out: app process RssAnon | 1,938 [1,854–1,977] | 160 [160–161] | 12.1× |
| 5000 clients, saturated fan-out: app + Redis + Thruster Pss | 2,211 [2,126–2,255] | 182 [181–183] | 12.2× |
| 5000 clients, saturated fan-out: whole container Pss | 2,489 [2,422–2,551] | 182 [181–183] | 13.7× |
| 10000 clients, all subscribed, idle: app process Pss | 1,485 [1,483–1,502] | 250 [249–254] | 5.9× |
| 10000 clients, all subscribed, idle: app process RssAnon | 1,625 [1,625–1,640] | 229 [227–233] | 7.1× |
| 10000 clients, all subscribed, idle: app + Redis + Thruster Pss | 2,292 [2,226–2,350] | 250 [249–254] | 9.2× |
| 10000 clients, all subscribed, idle: whole container Pss | 2,587 [2,502–2,645] | 250 [249–254] | 10.3× |
| 10000 clients, saturated fan-out: app process Pss | 2,096 [1,876–2,183] | 248 [247–250] | 8.5× |
| 10000 clients, saturated fan-out: app process RssAnon | 2,238 [2,013–2,323] | 227 [225–231] | 9.9× |
| 10000 clients, saturated fan-out: app + Redis + Thruster Pss | 2,906 [2,731–3,103] | 248 [247–250] | 11.7× |
| 10000 clients, saturated fan-out: whole container Pss | 3,182 [3,025–3,397] | 248 [247–250] | 12.8× |
