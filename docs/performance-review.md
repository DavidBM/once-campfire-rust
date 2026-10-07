# Campfire performance PR review, 2026-10-07

22 pull requests are confirmed merged on GitHub. All seven current benchmark tables are published. The useful cache work from the separately closed Laravel #1 is also retained in main. Local merge author and committer are `GPT on behalf of DHH`, while contributor commits and partial-work co-author credits are preserved.

38 proposals reviewed across Rails, Rust, Go, Elixir, Laravel and Express. Django has no open performance PRs and is included in the current-state benchmark.

Positive speed claims are accepted only after independent checks. Leave-open findings below come from source review unless an explicit runtime regression test is listed; their advertised speeds are not represented as independently reproduced.

| Pull request | Author | Review outcome |
|---|---|---|
| [once-campfire #334](https://github.com/basecamp/once-campfire/pull/334) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #331](https://github.com/basecamp/once-campfire/pull/331) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #330](https://github.com/basecamp/once-campfire/pull/330) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #329](https://github.com/basecamp/once-campfire/pull/329) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #328](https://github.com/basecamp/once-campfire/pull/328) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #326](https://github.com/basecamp/once-campfire/pull/326) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #325](https://github.com/basecamp/once-campfire/pull/325) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #324](https://github.com/basecamp/once-campfire/pull/324) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #323](https://github.com/basecamp/once-campfire/pull/323) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #322](https://github.com/basecamp/once-campfire/pull/322) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #321](https://github.com/basecamp/once-campfire/pull/321) | thomasklemm | Leave open: counter-cache totals can become stale after another implementation writes messages to the shared schema. |
| [once-campfire #319](https://github.com/basecamp/once-campfire/pull/319) | thomasklemm | Leave open: disables auto-checkpointing globally without a writer-side fallback in console/rake writers; also includes unrelated dependency/test/CI removals. |
| [once-campfire #318](https://github.com/basecamp/once-campfire/pull/318) | thomasklemm | Merged: independently checked; integrated production build measured below. |
| [once-campfire #316](https://github.com/basecamp/once-campfire/pull/316) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #312](https://github.com/basecamp/once-campfire/pull/312) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #311](https://github.com/basecamp/once-campfire/pull/311) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #310](https://github.com/basecamp/once-campfire/pull/310) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #296](https://github.com/basecamp/once-campfire/pull/296) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire #164](https://github.com/basecamp/once-campfire/pull/164) | ashwin47 | Leave open: members_hash can become stale after membership changes; #310 provides the direct lookup without a redundant membership hash. |
| [once-campfire-rust #44](https://github.com/basecamp/once-campfire-rust/pull/44) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire-rust #45](https://github.com/basecamp/once-campfire-rust/pull/45) | namespaceMarcello | Merged: independently checked; integrated production build measured below. |
| [once-campfire-go #2](https://github.com/basecamp/once-campfire-go/pull/2) | chemshit | Leave open: ENV GCGO is not Go's GOGC setting and has no GC tuning effect. |
| [once-campfire-go #4](https://github.com/basecamp/once-campfire-go/pull/4) | sernle | Competing implementation independently built/preflighted/timed. Full-sidebar correction extracted and credited; Selected #9 for stronger room/message/search performance; #4 posts faster, as measured below. |
| [once-campfire-go #5](https://github.com/basecamp/once-campfire-go/pull/5) | kidandcat | Leave open: generation caches advance only for writes through this DB instance and can reuse stale data after an external SQLite writer commits. |
| [once-campfire-go #6](https://github.com/basecamp/once-campfire-go/pull/6) | nick-potts | Competing implementation independently built/preflighted/timed. Guarded renderer already incorporated and credited in #9; Selected #9 for stronger room/message/search performance; #4 posts faster, as measured below. |
| [once-campfire-go #7](https://github.com/basecamp/once-campfire-go/pull/7) | riscdanger | Competing implementation independently built/preflighted/timed. Alternative compression approach to #9; Selected #9 for stronger room/message/search performance; #4 posts faster, as measured below. |
| [once-campfire-go #8](https://github.com/basecamp/once-campfire-go/pull/8) | borovikovd | Leave open: publication relies on subscribe-time authorization/own-write disconnects; external membership revocation does not receive the same fresh check as #9. |
| [once-campfire-go #9](https://github.com/basecamp/once-campfire-go/pull/9) | nijaru | Merged: independently checked; integrated production build measured below. |
| [once-campfire-elixir #1](https://github.com/basecamp/once-campfire-elixir/pull/1) | zachdaniel | Leave open: competing rewrite removes durable Resque jobs/live Rails interop and moves native parsing into the BEAM. #4 retains those contracts and isolates parsing. Claimed throughput not independently reproduced. |
| [once-campfire-elixir #2](https://github.com/basecamp/once-campfire-elixir/pull/2) | kurtome | Leave open: broad response caching/optional Redis rewrite has not completed its own browser, mutation, frozen-reference and rollback gates. Selected narrower pooled implementation #4; claimed 8–10x not independently reproduced. |
| [once-campfire-elixir #3](https://github.com/basecamp/once-campfire-elixir/pull/3) | aloukissas | Leave open: alternative reader/writer pool superseded by verified #4 integration. No measured throughput claim in the PR. |
| [once-campfire-elixir #4](https://github.com/basecamp/once-campfire-elixir/pull/4) | oliver-kriska | Merged: independently checked; integrated production build measured below. |
| [once-campfire-elixir #5](https://github.com/basecamp/once-campfire-elixir/pull/5) | lau | Leave open: written(tables) replaces the saved WAL header after bumping only local table generations. An external change followed by an unrelated local write can be absorbed without invalidating cached external data. |
| [once-campfire-elixir #6](https://github.com/basecamp/once-campfire-elixir/pull/6) | pasilastbot | Partial adoption: literal-regex cache extracted, tested and credited. Native SQLite reads run on ordinary BEAM schedulers; previous-run slow-query detection does not bound the first slow call. Rest depends on unsafe #5 cache. |
| [once-campfire-laravel #1](https://github.com/basecamp/once-campfire-laravel/pull/1) | sneycampos | Code adopted and credited; author closed this PR without merging. |
| [once-campfire-laravel #2](https://github.com/basecamp/once-campfire-laravel/pull/2) | JackEllis | Partial ideas credited, full PR left open: APCu caches authenticated session/user rows for 300 seconds without immediate session/ban revocation invalidation. |
| [once-campfire-laravel #4](https://github.com/basecamp/once-campfire-laravel/pull/4) | SilentKernel | Merged: independently checked; integrated production build measured below. |
| [once-campfire-express #3](https://github.com/basecamp/once-campfire-express/pull/3) | pstachula-dev | Merged: independently checked; integrated production build measured below. |

## Independent validation

- Rails: 486 tests, 1,703 assertions, no failures/errors (two skips); 27 Chromium system tests, 197 assertions, no failures/errors/skips. Rubocop and Brakeman clean. Current upstream message-ID security fix included.
- Rust: complete seeded workspace tests with CAMPFIRE_REQUIRE_SEED=1; clippy with warnings denied, formatting and unused-dependency checks passed. Search authorization and sparse-membership fallback have regression tests.
- Go: bin/check, all tests under the race detector, vet, generated assets, websocket fork tests, and cache identity/layout/cursor regressions passed.
- Elixir: 1,943 tests passed, strict compilation and formatting clean. Live mutation, session/cookie interoperability, message cache invalidation, storage, room deletion, Cable/interoperability, user lifecycle, jobs, worker lifecycle, and Chromium login/rooms/messages/uploads/profile/logout checks passed. Full 65-gate parity and production rollback are not claimed.
- Laravel: 31 tests, 609 assertions passed. Checks exercise one booted Octane worker across users, guests, CSRF tokens and abandoned transactions; warmed cached messages reflect creator, room and booster changes. Fragment text retains real CSRF-like strings while form tokens use the current viewer.
- Express: 142 tests passed, exact pinned formatter passed. External SQLite membership revocation takes effect immediately for warmed Cable auth; notification jobs are committed before HTTP acknowledgement; every writer retains a WAL auto-checkpoint backstop.
- Django: 46 tests passed, including complete sidebar page and viewer identity.

## Benchmark method

16 concurrent HTTP clients; AMD Ryzen AI MAX+ 395, 32 GB RAM; four pinned hardware threads per app (8–11) and four separate generator threads (12–15). Native production images, public HTTP listener, gzip, two alternating rounds, two-second warmups and eight-second samples. Applications are timed serially, with builds and other tests stopped. No synthetic stub server or C benchmark seed is used.

Every app receives a fresh copy of the actual full parity seed. Push/webhook fixture endpoints point to a closed local port. Preflight validates decompressed complete pages, message result windows, authenticated content, assets/avatar/health responses. Each completed sample must have zero errors/invalid responses; every acknowledged HTTP post must exist with rich text and an FTS row; SQLite integrity is checked. Search result IDs are compared as sets because historical ports order their matches differently.

Go, Express, Laravel and Django baseline/variant sidebar pages were corrected to render their complete layout before timing. That correction is applied on both sides, so missing output is not counted as an optimization. Baseline source revisions/dirty status and production image identities are recorded with the local raw evidence.

Raw results are kept locally in benchmark/final; none are added to the repositories. One interrupted run after a newly discovered Laravel cache issue is excluded except for earlier completed, validated application samples; the affected app was not timed before its corrective build.

## Large-history Rust evidence

Two million generated messages on top of the real seed; SQLite 3.53.4; six alternating SQL samples. Broad search: coffee 43.2 → 0.28 ms, common message 677.7 → 0.26 ms. The original PR's global reverse scan regressed no-access common-word searches to about 384 ms. A 1,000-row fast-probe limit and membership-scoped fallback bound the corrected case to about 0.7 ms; sparse/no-access checks return no unauthorized results. This bounded overhead is higher than the old ~0.02 ms empty-membership query, so this is not a universal improvement.

Incremental refresh with 0/23/1,000 updated rows: ~151–155 ms → 0.07/0.07/0.36 ms. A refresh requesting the entire two-million-message history regresses ~419 → 571 ms. The optimization targets incremental refresh, not complete-history scans. These are illustrative SQL microbenchmarks, separate from the small-seed HTTP comparison.

## Measured production HTTP results

| HTTP workload (requests/sec) | Rails | [Django](https://github.com/basecamp/once-campfire-django) | [Laravel](https://github.com/basecamp/once-campfire-laravel) | [Express](https://github.com/basecamp/once-campfire-express) | [Elixir](https://github.com/basecamp/once-campfire-elixir) | [Go](https://github.com/basecamp/once-campfire-go) | [Rust](https://github.com/basecamp/once-campfire-rust) |
|---|---:|---:|---:|---:|---:|---:|---:|
| Room page | 236 | 62 | 764 | 2,702 | 981 | 32,132 | 35,056 |
| Messages page | 384 | 70 | 922 | 3,183 | 1,341 | 31,564 | 40,481 |
| Sidebar | 474 | 230 | 1,399 | 34,595 | 2,546 | 17,993 | 33,924 |
| Search | 415 | 120 | 1,291 | 6,725 | 1,907 | 29,775 | 34,199 |
| Post a message | 244 | 113 | 498 | 2,183 | 1,431 | 9,442 | 8,995 |

Rates are medians of two runs. The Go column was remeasured after the contributor’s final storage-key validation and fresh sidebar layout commits. Express changed from one default HTTP worker to four on the assigned CPU set. Laravel changed from PHP-FPM to eight persistent Octane workers. Django uses its current one-worker default; its earlier published numbers used an explicitly configured multiworker setup. These differences are part of the configurations tested, so the table does not isolate language speed.

| Implementation | Room page before → after | Search before → after | Post before → after |
|---|---:|---:|---:|
| Rails | 235 → 236 | 410 → 415 | 243 → 244 |
| Elixir | 720 → 981 | 1,164 → 1,907 | 866 → 1,431 |
| Go | 3,847 → 32,132 | 7,042 → 29,775 | 7,352 → 9,442 |
| Rust | 35,835 → 35,056 | 33,535 → 34,199 | 9,241 → 8,995 |
| Express | 239 → 2,702 | 490 → 6,725 | 852 → 2,183 |
| Laravel | 168 → 764 | 323 → 1,291 | 202 → 498 |

The warm Rails fixture does not show an overall throughput improvement; its sidebar is about 8% slower in this run. Its selected changes address query counts, growing histories, bounded external work and notification fan-out, rather than proving a blanket page-speed gain. Rust’s warm read/post rates are within roughly 3% of baseline, with its large-history query results reported separately above.

| Go build | Room | Messages | Sidebar | Search | Post |
|---|---:|---:|---:|---:|---:|
| go-before | 3,847 | 5,520 | 13,753 | 7,042 | 7,352 |
| go-pr4 | 4,202 | 5,855 | 16,351 | 9,623 | 11,975 |
| go-pr6 | 5,005 | 8,114 | 19,617 | 12,415 | 8,944 |
| go-pr7 | 24,646 | 26,291 | 23,214 | 14,670 | 7,374 |
| go | 32,132 | 31,564 | 17,993 | 29,775 | 9,442 |

## Attribution and scope

Contributor commits are retained in the merge history. Partial contributions are credited with Co-authored-by trailers: sernle’s full-sidebar correction, Nick Potts’s renderer incorporated by nijaru, Pasi Vuorio’s literal-regex cache, and Jack Ellis’s cache identity/token ideas. Laravel #1 was closed by its author during review; its useful fragment-cache work is retained and credited, rather than treating its entire superseded runtime proposal as a fresh open PR.

The new public [C fork](https://github.com/basecamp/once-campfire-c) retains mrsaraiva’s history. Its current build has not yet been measured on this full fixture and is not included in the table. The prior C POST claim used an invalid membership seed and cannot be reused; the corrected importer and persistent-write validation are published in the fork.

The architecture transfer pass follows this PR review. The table represents these reviewed versions; further changes will be checked and remeasured before replacing it. WebSocket throughput is not inferred from HTTP rates, and old WebSocket figures have been removed from the current READMEs.
