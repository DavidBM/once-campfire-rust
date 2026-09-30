# User-Agent parsed once per request, 2026-09-30

Branch `perf-user-agent`: plan §4 item 6 "User-Agent" (finding `ua-parse-once`). Measured on
`ed75dc0`, against `d93d3dd` (wave 3) with the timing harness added (`063ec97`).

- `45af9c5`: the parser slices the header by byte offset. Before, it rebuilt a `Vec<char>` of the rest after every product.
- `b958f2e`: case-insensitive product-name comparisons skip lowercasing for ASCII names.
- `48ee53f`: `to_view` works out the gem's `browser` and `operating_system` once. Before, it did so five times and twice.
- `4d0f527`: `allow_browser` parses a present header once, checks it, and keeps the platform in the `Ctx` for the layout. A missing or blank header is still not parsed there.
- `ed75dc0`: a version is split into segments only when it is compared.

## In-process timing

The harness is `time_user_agent_work` in `crates/campfire/src/concerns/platform.rs`:

`cargo test --release -p campfire --bin campfire time_user_agent_work -- --ignored --nocapture`

It uses the release profile (fat LTO, one codegen unit) and runs pinned to CPUs 20–23. Each figure
is the median of 9 runs of 100,000 calls. The baseline and final binaries ran alternately, three
times each, and the tables show the median of those three. Other agents' builds kept the host's
load average at 6–12 throughout, so expect noise of about ±10% per cell. Raw runs: [`runs.txt`](runs.txt).

What each column times, in ns per call:
- **parse** is `user_agent::parse`.
- **allow_browser** is the before-action's User-Agent work. Before: `parse` and the check. After:
  the presence check, `ApplicationPlatform::new` (a copy of the header and `parse`) and the check.
  The `Ctx` insert isn't timed.
- **to_view** builds the layout's `Platform` from a platform that is already parsed.
- **page request** is `allow_browser` plus the layout's `platform`. Before, that parsed the header
  twice. After, it parses once, or once only when a page renders if there is no header.

| Agent | parse | allow_browser | to_view | page request |
|---|---|---|---|---|
| Chrome, macOS | 914 → 211 | 1,128 → 380 | 422 → 162 | 2,535 → 595 (4.3×) |
| Chrome, Windows | 902 → 232 | 1,154 → 393 | 360 → 133 | 2,506 → 558 (4.5×) |
| Chrome, Android | 1,090 → 265 | 1,355 → 454 | 423 → 145 | 2,915 → 638 (4.6×) |
| Safari, macOS | 1,055 → 240 | 1,353 → 521 | 561 → 212 | 3,051 → 821 (3.7×) |
| Safari, iPhone | 1,219 → 299 | 1,492 → 556 | 519 → 168 | 3,506 → 784 (4.5×) |
| Firefox, Windows | 782 → 217 | 1,019 → 389 | 497 → 117 | 2,411 → 554 (4.4×) |
| Edge, Windows | 1,189 → 253 | 1,385 → 454 | 410 → 135 | 3,024 → 622 (4.9×) |
| Googlebot | 475 → 183 | 737 → 269 | 663 → 165 | 1,958 → 504 (3.9×) |
| no header | 261 → 96 | 1 → 2 | 519 → 76 | 814 → 196 (4.2×) |

The page request after each commit, in ns. Each step is the median of three runs on that step's
own binary, taken as it landed, not interleaved:

| Agent | before | byte scan | ASCII compare | view once | parse once | lazy segments |
|---|---|---|---|---|---|---|
| Chrome, macOS | 2,494 | 1,315 | 1,106 | 960 | 604 | 577 |
| Safari, iPhone | 3,289 | 1,847 | 1,705 | 1,264 | 841 | 795 |
| Firefox, Windows | 2,337 | 1,537 | 1,052 | 913 | 551 | 578 |
| Googlebot | 1,912 | 1,546 | 906 | 730 | 496 | 516 |
| no header | 814 | 685 | 315 | 228 | 191 | 193 |

Two things didn't pay off, or paid off less than expected:
- A case-insensitive window scan over the whole header made `to_view` slower for Safari, 540 → 695
  ns, so `apple_messages` still lowercases the header once. Measured standalone (rustc -O),
  `to_lowercase` plus two `contains` take 15 ns on a 120-byte header. The window scan takes 40–130
  ns. On short product names and comments the scan wins, 5–30 ns against 40–60.
- Lazy version segments cut `parse` by a quarter, but the page request by only 0–7%, because the
  blocked check still splits the version it compares. The change is kept for the parse and for
  cheaper `Version` clones.

## What it means for a page

The wave-3 run measured about 0.20 ms of CPU per room page. Against that, the saving is about 1.9–2.7
µs per page request with a browser's User-Agent, 1–1.4%. Without a User-Agent, which is how every
benchmark so far ran, it is about 0.6 µs, 0.3%. A POST with a User-Agent saves allow_browser's
share, about 0.6–0.9 µs. That is below `bench/attrib-report`'s two-decimal ms/request resolution.
The raw `cpu_ms_per_req` in the per-run JSON has four decimals.

## Output unchanged

- The gem-generated vectors (`vectors/campfire_user_agents.json`) pass for all 385 agents. They
  cover the parse, the `ApplicationPlatform` predicates, the whole view (false or "" where Ruby
  raises) and `blocked`. This work appended 127 agents to `reference-tools/campfire/user_agents.rb`
  and regenerated the vectors with useragent 0.16.11 in the reference image; the 258 agents already
  there came out unchanged. The new agents are desktop and mobile Chrome, Safari, Firefox, Edge,
  Opera, Samsung Internet, in-app browsers, bots, HTTP clients, and blank, malformed and non-ASCII
  strings, including a U+212A Kelvin sign that lowercases to ASCII and a comma before a comment.
- New equivalence tests check the fast paths against the code they replaced, over every agent in
  the vectors:
  - The old char-by-char matcher, over every prefix of every agent.
  - Lowercase-then-compare and lowercase-then-contains, over every product name and comment.
  - The old WebKit comment matcher, over every suffix.
- A seed-backed test checks that the platform `allow_browser` parsed is the one the layout reads,
  that a missing or blank header is left to the layout, and that an action which skips the
  before-actions still gets the header's platform.
- Before the new agents were in the vectors, a snapshot of every answer for them and the vectors'
  agents was taken before the first change and diffed after each commit. It covered the products,
  `browser`, `version`, `platform`, `os`, `bot?`, `mobile?`, each predicate, the view and
  `blocked`. Every diff was identical, apart from `Version`'s `Debug` losing its segments in the
  last commit. The vectors now pin those answers, so the snapshot test isn't committed.

## HTTP A/B

To be added by the coordinator. The load generator now takes `--user-agent`, and so do
`bench/attrib`, `bench/run` and `bench/profile`.
