# Code-quality and library review (2026-09-29)

A review of the whole workspace at `3ab666e` for idiomatic Rust and library choices: 12 area
reviewers plus 4 follow-ups a completeness critic asked for, 219 findings, each checked by one
agent against the code and by another for cost and risk (Rails parity, data compatibility,
performance, security). What follows is the synthesized plan; section 5 is the order of work.

## 1. Verdict

The workspace is in good shape. Crates are small and well commented, and each one cites the Rails, Go or Rack source it follows. Libraries use thiserror and the binary uses anyhow. Statics use std `LazyLock`/`OnceLock` throughout. Outside the libvips FFI, every `unsafe` block has a SAFETY comment; the 18 in vips.rs have none yet (§3.2). The direct dependency list is short, and nothing in Cargo.lock matches an active RustSec advisory.

Nearly every hand-written piece a reader would question should stay. That covers params, cookies, the Journey router, the WebSocket code, the sanitizer, the useragent port, rqrcode, Marcel and Marshal, the libvips FFI, the HTTP client, the Web Push crypto and the caches. Each exists because no crate reproduces the Rails, Go or performance behaviour the app needs.

The debt is elsewhere:

- **Duplicated Ruby and Rails behaviour across crates.** There are 5 ERB escapers, 7 ActiveSupport JSON escapers, 4–5 copies of `String#to_i`, 3 Rack byte-range parsers, 2 MessageVerifiers, 2 bcrypt wrappers and 3 unverified-SGID fallbacks. Several copies already disagree with Rails or with each other, and in two cases the golden vectors test the copy production doesn't use.
- **Unused feature flags and a few stale versions.**
- **A few real bugs to fix before any style work:**
  - Hostile message bodies cost quadratic CPU inside the SQLite writer.
  - The bundled SQLite 3.50.2 has the WAL-reset corruption bug, and the app's off-writer checkpointer is the exact setup it needs.
  - libvips error messages get polluted and eventually drowned.
  - The response cache keys the Active Storage proxy by a normalized query, so a crafted URL can poison it.
  - The single-user cable benchmarks can't see several kilobytes of memory per user.

## 2. Libraries

### Remove or trim

| What | Where | Why | Effort |
|---|---|---|---|
| axum default, `ws` and `multipart` features. Use `default-features = false, features = ["tokio"]`, put `http1` in the cable/campfire dev-deps and `ws` in the kit dev-deps | Cargo.toml:22, crates/kit/tests/front.rs:23 | Production uses no axum extractor and not axum's WebSocket (cable upgrades through hyper, kit calls multer directly). The release tree loses tokio-tungstenite, tungstenite, serde_urlencoded, serde_path_to_error, ryu, mime_guess, unicase and http-range-header (262 → 254 crates; clippy and tests verified clean). Commit Cargo.lock, because Docker builds with `--locked`. | trivial |
| tower-http: remove it (or at least drop `fs`/`limit`) | Cargo.toml:24, kit/src/adapter.rs:388 | Its only use is `TimeoutLayer` behind `KitConfig::request_timeout`, which production never sets. The front server already enforces HTTP_READ/WRITE_TIMEOUT. | trivial |
| rusqlite `functions` and `serde_json` features | Cargo.toml:29 | Nothing uses them. | trivial |
| Registry html5ever duplicate: point `[workspace.dependencies] html5ever` at the vendored path | Cargo.toml:45, crates/richtext/Cargo.toml:9 | views' dev-dependency compiles a second html5ever 0.35. The tokenizer is identical in both. | trivial |
| Unused declarations: db `serde`, richtext dev `serde`, storage dev `hex`. Redundant dev-deps: campfire `futures-util`/`tempfile`/`rusqlite`, kit `tower`/`tempfile`/`x509-parser`. Make db `crc32fast` optional under `test-support`. | crates/db/Cargo.toml:11,21; richtext/Cargo.toml:16; storage/Cargo.toml:24; campfire/Cargo.toml:57; kit/Cargo.toml:52-56 | Manifest accuracy. | trivial |
| storage `hmac` plus dev `pbkdf2`/`sha2`; db `bcrypt` | via §4.2 | These go away once storage uses the production verifier and db uses `rails_compat::password`. | small |
| loadgen `hdrhistogram` default features. Add flate2 `runtime_detection` in the same change. | bench/loadgen/Cargo.toml:18 | −6 crates. Without `runtime_detection`, zlib-rs loses the `std` feature it gets by accident today and its SIMD detection with it (the level-6 gzip numbers shift). | trivial |

### Swap

| From → to | Where | Why | Effort |
|---|---|---|---|
| rusqlite 0.37 → 0.40.2 with `default-features = false, features = ["bundled","cache","hooks"]` (campfire keeps `backup`) | Cargo.toml:29 | Moves bundled SQLite from 3.50.2 to 3.53.2, which fixes the WAL-reset corruption bug (affects 3.7.0–3.51.2). That bug needs a checkpoint racing a commit that resets the WAL, which is exactly the dedicated checkpointer in db/src/database.rs:349-395. It also removes the hashbrown 0.15 duplicate. Verified: no source changes, 655 tests pass including 162 seed-backed. Bench before and after, because the query planner changed. | small |
| fancy-regex → `regex::bytes` in the Propshaft port | crates/assets/build/propshaft.rs:75-103, :245 | The two lookarounds can be emulated exactly. This deletes the four Latin-1 helpers and drops fancy-regex, bit-set and bit-vec 0.8. A prototype produced a byte-identical OUT_DIR, and the debug build script went from 1.59 s to 0.15 s. This overrides the "keep fancy-regex" finding. | small |
| x509-parser → x509-cert | kit/src/front/acme.rs:443,464; tests/front.rs:539 | Only the validity window and SAN DNS names are read. Net −11 crates, including nom 7 and synstructure 0.13. Use 0.2.5 now (shares der 0.7 with p256 0.13) or 0.3 together with p256 0.14. First add tests for a positive wildcard, expired and not-yet-valid certs, and a Let's Encrypt-shaped leaf fixture: the strict DER parser also runs on freshly issued chains (acme.rs:299), so a rejection there would fail issuance on every retry. | small |
| serde_yaml (deprecated) → yaml-rust2 0.13 with `default-features = false` | crates/db/src/fixtures.rs:21,175,300 | Test-only. Do it after the rusqlite bump so both share hashlink 0.12. The default `encoding` feature would add a second encoding_rs. Low priority. | small |
| ICU4X → pin `idna_adapter = "~1.1"` (workspace plus kit) | kit/src/front/acme.rs:382, the only `url::Host::parse` | Net −15 crates, including synstructure 0.14. Costs about +120 KB of binary, and characters added in Unicode 17 are rejected. Optional. | trivial |
| askama 0.14 → 0.16.1 | Cargo.toml:30; views/src/helpers/filters.rs; views/src/rooms.rs:338 | Moves to the maintained line. Needs `#[askama::filter_fn]` on 13 filters with named generics (0.16 rejects `impl Trait`), a `'a` lifetime on `room_form`, and dropping the `blocks` feature. The goldens stay byte-identical (verified). SIZE_HINTs roughly double, so bench it. | small |
| zstd 0.13 → 0.14 (same libzstd 1.5.7). tikv-jemallocator and -sys 0.6 → 0.7.1, bumped together because of `links` | Cargo.toml:77,79 | Keeps them on maintained lines. Bench memory and RSS for jemalloc. | small |
| Later, as one batch: hmac 0.13, sha1/sha2 0.11, pbkdf2 0.13, aes-gcm 0.11, hkdf 0.13, md-5 0.11, p256 0.14, rand 0.10, bcrypt 0.19.3 (not 0.19.0/0.19.1: RUSTSEC-2026-0199), base64 0.23, x509-cert 0.3 | workspace | Collapses rand_core to 0.10 and getrandom to 0.2 (ring) plus 0.4. Only do it after axum `ws` is dev-only: with `ws` still on, the tree grows from 262 to 274 crates (measured). No runtime benefit. | medium |

### Add (already compiled, becoming direct dependencies)

- **tokio-util.** Use `ReaderStream::with_capacity(file.take(len), 64K)` in place of the zero-filling unfold at kit/src/adapter.rs:207. Replace the hand-rolled front shutdown counter (front/conn.rs:56-148) with `CancellationToken` + `TaskTracker`. Use `drop_guard` so a dropped signal task still closes the server. The `rt` feature adds futures-macro. Effort: small.
- **sec1 0.7 (`pem`)** for the autocert SEC1 key, replacing the hand-assembled DER bytes at acme.rs:409. The existing test pins the output byte for byte. Effort: trivial.
- **sha2 `asm` for `cfg(target_arch = "aarch64")`** in kit. The published arm64 image computes SHA-256 in software today, because sha2 0.10 only uses the ARMv8 SHA2 instructions with `asm`. That covers text parts, ETags and HMAC. Measure on real ARM. Effort: trivial.
- **rustix** (`process`, `thread`, `time`) to replace the libc `unsafe` in kit/src/server.rs:31, kit/src/front/conn.rs:242 and campfire/src/main.rs:37. The vDSO clock still bypasses libfaketime; add a comment saying rustix's `use-libc` feature would break that. Effort: small.
- **parking_lot** (optional, for consistency). It would replace 42 `.lock().unwrap()` and about 10 `unwrap_or_else(into_inner)` sites with one policy. std `nonpoison` is still unstable. Frame this as removing boilerplate, not as fixing an outage risk.
- **futures-channel** (optional) for the cable reader task. tokio's mpsc allocates a 1,568-byte block up front; futures-channel saves about 1.37 KB per socket.
- **Existing crates in place of hand loops:** `base64`, `hex`, `form_urlencoded` and `percent-encoding` (§3.1).
- **CI tools, not crates:** the rustfmt component, cargo-shear (with ignores for the assets build script's `#[path]` modules and db's self dev-dependency), and a scheduled, non-blocking `cargo deny check advisories` with `multiple-versions = "warn"`.

### Keep, with the reason

| Keep | Why not the obvious crate |
|---|---|
| Own WebSocket (cable/src/socket.rs) | No maintained crate sends one precompressed frame to N sockets. Every permessage-deflate crate compresses per socket and keeps per-socket buffers. At 100k clients this took memory from 3.2 GB idle / 5.9 GB peak to 2.9 GB, and throughput from 300k to 410k deliveries/s. Validate it once with Autobahn against a small echo example, not against /cable. |
| async-trait | `Channel`, `Authenticate` and jobs `Handler` are used as `dyn`. Native `async fn` in a trait is E0038 on 1.98.1. instant-acme depends on async-trait anyway. |
| futures-util `SelectAll` / `Abortable` | tokio-stream's `StreamMap` polls every stream on each wake. |
| Journey route table, not matchit | Rails takes the first match in table order (`GET /rooms/opens` is rooms#show). `(.:format)`, `[^/.?]+` and lazy globs can't be expressed in matchit. The table is checked against vectors/campfire_routes.json. |
| Params builder, multipart disposition, Accept negotiation, CookieJar and session | These follow Rails' nested-key rules and Rack's quirks. The Rails signed/encrypted envelopes keep people signed in across an upgrade, and the jar only emits a cookie when it changes. |
| Axum as a thin base layer; HRTB `ActionFn`, `Err(Halt(Box<Response>))`, per-action `catch_unwind` | Swapping axum for raw hyper saves about 6 small crates. `AsyncFn` bounds still can't require `Send` futures on stable. |
| Hyper connections driven by hand, plus the Deadline/InFlight/Recording/Logged bodies | GracefulShutdown has no per-connection idle close. tower-http's `TimeoutBody` is an idle timeout, not Go's absolute deadline. `axum::serve` can't disable h2c or set these timeouts. |
| gzhttp and Rack::Deflater by hand; flate2 on zlib-rs; zstd | tower-http's CompressionLayer can't splice stored deflate pieces. zlib-rs is 2.3× faster than miniz_oxide. zstd matches Thruster's negotiation. (The bit-by-bit CRC-32C is cold; if it ever warms up, a 256-entry table fixes it.) |
| instant-acme + rcgen on ring, with autocert's DirCache written by hand | rustls-acme's cache layout, and its all-domains-up-front ordering, would lose existing certificates and accounts. |
| Front response cache and splice stores written by hand | moka would replace Thruster's sampled eviction. quick_cache has no per-entry TTL. Neither can express identity pinned by a `Weak`. |
| DB writer thread, checkpointer and in-house reader pool | The `wal_hook` takes a fn pointer and uses a thread_local, and a single writer means no SQLITE_BUSY. r2d2 and deadpool would add threads, mutexes and connection churn that throws away statement caches. |
| Name-based `row.get("col")` over `SELECT *` | Column order differs between fresh and migrated installs. There is a performance follow-up in §4.6. |
| Own `Timestamp` codec, not rusqlite's `jiff` feature | Active Record writes `%Y-%m-%d %H:%M:%S[.%6N]`. |
| uuid, crc32fast, rand `ThreadRng` | All fine. Note that rand 0.9's `random_range` uses Canon's method (slightly biased), not rejection sampling. That doesn't matter at these alphabet sizes. |
| libvips FFI (20 functions), not the `libvips` or `rs-vips` crates | libvips 2.3 always passes `input-profile`/`output-profile`, which libvips 8.16.1 rejects, so every variant would fail. Both crates ship about 84k–109k generated lines. |
| md-5, Marcel tables, Marshal writer, Rack `byte_ranges` | Existing checksums, `variation_digest` values and content types depend on them. |
| `ErbEscaper`, not askama's Html escaper | askama writes `&#34;`, which would change the bytes of every page and every ETag. |
| Sanitizer and arena DOM on vendored html5ever 0.35 | ammonia can't express Loofah's rules. html5ever 0.37+ adds customizable `<select>` parsing and escapes `<>` in attributes. |
| URI, CGI.unescapeHTML, Loofah numeric refs, JSON comments, base64 fallback (all Ruby semantics) | The `url` crate is WHATWG. Entity crates decode every named entity. Two parity bugs are listed in §3.1. |
| FragmentCache (MemoryStore semantics); `regex` (not fancy-regex) in richtext | These match Rails' eviction, and the TagIndex keeps autolinking linear. |
| RustCrypto beside ring | ring has no MD5, no ECDH with a fixed key, and no SEC1 import. |
| base64 0.22 | 0.23 comes only from pem, which instant-acme's `ring` feature forces on. It can't be removed from this side yet. |
| `include_bytes!` asset tables with binary search; macro-generated path helpers; the Ruby shims (Marshal reader, Rack unescape, urlsafe_decode) | All exact and fast. |
| rqrcode port | The qrcode crate reproduces 12 of the 39 vectors (15/39 when forced). |
| Addressable query re-encoding | `form_urlencoded` treats `+`, `*` and `~` differently. One bug is listed in §3.1. |
| Own HTTP/1.1 client over hyper | reqwest has no per-request IP pinning, follows only certain 3xx codes, and honours HTTP_PROXY by default. |
| RFC 8291 / VAPID by hand | The web-push crate goes through ece, whose only backend is OpenSSL. |
| libxml2-style meta scanner | HTML5 tokenizers differ on the pinned cases. Revisit if the reference moves to libxml2 2.14. |
| Surfguard port (guard.rs) | `is_global` is unstable and has a different policy. glibc numeric host forms (`0x7f.1`) need the hand parser. |
| `Config::from_lookup`, the per-kind bounded job runner, the useragent port, manual argv and runtime, jemalloc `malloc_conf`, the bounded backup loop, tokio `full` | clap, envy, figment, woothee and ua-parser all change semantics or add weight for no gain. |
| sha2, not blake3 | blake3 is slower on small inputs and on the ETag's many small updates, and the byte-keyed identity in §4.6 removes the hot hash anyway. |
| One contiguous gzip body; one-pass CRC | Multiple chunks mean more writev calls, chunk framing and DATA frames. Combining per-piece CRCs saves about 1%. |
| Plain `#[test]` loops over frozen, Ruby-generated corpora | libtest-mimic, datatest, insta, cargo-fuzz and proptest don't fit goldens that must come from the oracle. |
| Loadgen as a standalone crate; its hand-rolled form and cookie helpers; per-call regexes | Keeps the app's lockfile and fat-LTO profile out of the bench tool. Its shared crates are at the same versions as the app's. |
| Dependency inversions: db `RichText`, richtext `AttachableResolver`, `Arc<dyn EventSink>`, `dyn Resolver`/`Dialer`, views `Page`/`CacheSize`; the hub-and-spoke crate graph | Sound seams. Don't merge crates. |

## 3. Idiomatic Rust (by theme, highest value first)

### 3.1 One home for Ruby and Rails behaviour, and fix the drifts (medium overall)

Where the copies are, and where they disagree:

- **ActiveSupport JSON `<>&` escaping**, 7 copies: rails_compat/src/json.rs:18, cable/src/json.rs:10, views/src/helpers/html.rs:52, richtext/src/ruby.rs:87, db/src/models/webhook.rs:134, opengraph/metadata.rs:154, storage/src/json.rs:137.
  - storage escapes U+2028/U+2029 (json.rs:151). `load_defaults 8.2` leaves them raw (probed in the reference).
  - storage formats floats with `Float#to_s`. `ActiveSupport::JSON` actually uses json 2.21.2's fpconv, so `1e16` becomes `1e+16` and `1e-5` becomes `0.00001`.
  - The doc comment at json.rs:4 ("serde_json's Map sorts keys") is stale, because `preserve_order` is on.
- **`ERB::Util.html_escape`**, 5 copies: richtext ruby.rs:6, views html.rs:16 (plus the faster `ErbEscaper`), cable/src/turbo.rs:161, assets/src/tags.rs:80, assets/build.rs. Keep kit/front/tls.rs:165 separate: it is Go's `html.EscapeString` (`&#34;`).
- **`String#to_i` and the AR integer cast**: concerns.rs:510, concerns.rs:498 (`cast_integer`), channels/room.rs:52, storage/file_server.rs:182, assets/serve.rs:230.
  - They disagree on `"\v5"`, `"0d12"`, `"1_000"`, `"+5"`, a leading NBSP, and overflow.
  - The assets copy turns an overflowing end offset (`bytes=0-99999999999999999999`) into a 1-byte range.
- **`String#strip`**, 5 identical copies: richtext ruby.rs:27, concerns.rs:534, user_agent.rs:233, net/http.rs:316, storage/filename.rs:57.
- **`Float#to_s`**, 3 copies: richtext ruby.rs:227, views messages/support.rs:72, storage json.rs:162. All three switch to exponent form at 1e16, but Ruby 3.4.10 does it at 1e15 (probed: `1e15.to_s == "1.0e+15"`).
- **Rack `get_byte_ranges`**, 3 copies: assets serve.rs:182, storage file_server.rs:111 (the only one faithful to Rack 3.2.6), and kit response.rs:176/272, which nothing in production calls.
- **ContentDisposition**, 2 copies: kit/src/response.rs:101-160 has a Latin-1-only `transliterate`, so `Łódź.pdf` becomes `%3Fod%3F.pdf`. storage/disposition.rs uses the I18n table and is checked by vectors. The broken copy is only reachable on the unlinked proxy routes, so the bug is latent.
- **Percent encoders**, about 10 copies:
  - rails_compat/cookies.rs:97 is exactly `form_urlencoded::byte_serialize`.
  - storage/paths.rs:71 says it is `CGI.escape` but is actually `URI.encode_www_form_component`.
  - `ERB::Util.url_encode` (richtext ruby.rs:74, views links.rs:31) and Addressable UNRESERVED (pagination.rs:138) use the same set.
- **Crypto and identity copies:**
  - A second MessageVerifier (storage/src/verifier.rs:26) is shipped in the library but only the tests use it.
  - bcrypt is called directly in db (user.rs:289, 611, 683), while the vector-tested `rails_compat::password` has no callers.
  - The unverified-SGID rule exists three times. The rails_compat copy (global_id.rs:131) is dead but vector-tested. The richtext copy (attachables.rs:183) is the one that runs. db has a third (rich_text.rs:104).
- **Base64 and hex by hand:**
  - `urlsafe_decode64` in qr_code.rs:27-77 (50 lines), web_push.rs:239 and attachables.rs:229.
  - A hand-written URL-safe encoder in views application.rs:149.
  - `format!("{b:02x}")` loops in storage verifier.rs:42 (on the signed-URL path), config.rs:157 and active_storage.rs:649.

The plan:

1. **Fix the drifts in place first**, one small PR with values probed in the reference:
   - storage JSON: drop the U+2028/U+2029 escapes.
   - room.rs `to_i`: use Ruby's whitespace set (including `\v`) and accept `0d`.
   - `cast_integer`: run `/\A\s*[+-]?\d/`, then `to_i`, then a range check (the SQLite adapter's 8-byte integer, not `ActiveModel::Type::Integer.new`, which is 4 bytes).
   - assets: fix its own `byte_ranges` copy in place to match storage's Rack-faithful one. Depending on storage would pull the database and crypto stack into `campfire_assets` for one helper; the shared copy arrives with `ruby_compat` in step 3.
   - kit/format.rs:293 and deflater `to_f`: `q=0.5.1` should give 0.5.
   - `q=` with no value should mean 1.0 in format.rs:168-173 and deflater.rs:131-136. Today `identity;q=` gets a 406 where Rails gives 200.
   - storage `query_escape`: fix the set or the comment.
   - Proxy downloads: build Content-Disposition with storage's `format`.
   - richtext uri.rs:240: downcase the scheme. Today tweet normalization keeps `HTTPS://`.
   - `cgi_unescape_html`: count significant digits, so leading zeros don't block decoding.
   - pagination.rs:103: turn `+` into a space *before* unescaping, as Addressable does.
   - `Float#to_s`: switch to exponent form at 1e15.
2. **Rails contracts go into rails_compat** (make `json` and `encoding` public):
   - A generic `json::encode<T: Serialize>` plus `escape_html_entities(String)`, taken from cable with its no-allocation fast path. If you write it as a serde `Formatter`, its `write_f64` must follow fpconv, not `Float#to_s`.
   - `password::digest_with_cost` returning a `Result`.
   - A `content_disposition` module. APPROXIMATIONS is generated, so change reference-tools/storage/dump_tables.rb to write it.
   - The verifier storage uses directly (§4.2), and the clock (§4.3).
3. **Pure Ruby string behaviour goes into a zero-dependency leaf crate**, `crates/ruby` (package `ruby_compat`), so assets, richtext and views don't have to wait on the crypto crates:
   - `erb` (ErbEscaper's byte loop, generic over `fmt::Write`), `to_i`, `to_i_checked`, `integer_cast`, `to_f`, `strip`, `float_to_s`, `cgi_escape`, `url_encode`, `rack::byte_ranges`.
   - Check it with a new `reference-tools/ruby_core.rb` that writes vectors/ruby_core.json (all 256 single bytes, about 100 edge strings, and a set of Range headers).
4. **Use exact library equivalents:**
   - qr_code: `rails_compat::encoding::urlsafe_decode`. It uses base64's STANDARD engine after `tr`; don't use URL_SAFE, because Ruby also accepts `+` and `/`. Keep the qr_code test asserts.
   - cookie escaping: `form_urlencoded`.
   - `hex::encode`, `URL_SAFE.encode` and `crc32fast::hash` in place of views' hand-rolled base64 and CRC-32 (views helpers/users.rs:27).
   - Delete kit's `parse_range`, `RangeResult` and `SendOptions::ranges`.

About 250–400 lines disappear, and each Ruby behaviour is defined and tested once.

### 3.2 Error handling

- **`campfire_db::Error::Other(String)` flattens errors** (error.rs:20; about 30 construction sites, 10 of them in campfire). Change it to `#[error(transparent)] Other(Box<dyn Error + Send + Sync>)` with an `Error::other` constructor, like `io::Error::other`, and move `is_record_not_unique()` onto it from presenters/accounts.rs:290. The source chain only reaches the logs if kit/src/ctx.rs:647 logs `{error:#}`. Effort: small.
- **`storage_error` exists three times** (active_storage.rs:638, presenters.rs:461, attachments.rs:207). Keep the one that preserves `Sql` as `Sqlite`, and inline `storage_error_to_kit`. An `impl From` isn't possible because of the orphan rule. Effort: trivial.
- **One database helper for actions.** Add `AppState::read`/`write` returning `campfire_kit::Result` through `db_error`: `RecordNotFound` becomes 404, `RecordInvalid` becomes 422 (Rails' `rescue_responses`; this fixes today's 500 when an admin bans a user with a private IP), everything else 500. It replaces about 105 `.await.map_err(Error::internal | db_error)` tails and the hand-written RecordNotFound→Option→NotFound matches (accounts/bots.rs:107, accounts/users.rs:59). Effort: medium, mechanical.
- **kit `clone_error` doubles the log prefix** (adapter.rs:158: "bad request: bad request: …"). Take ownership with `split`/`.or()` instead. Effort: trivial.
- **WebSocket read errors** (socket.rs:35-315):
  - Use named header and partial structs instead of `(bool, bool, u8, Vec<u8>)`.
  - Add `ReadError::{Io, Protocol { code }}` and pass the code through the reader channel.
  - Close with websocket-driver's codes: 1003 for an unmasked frame, 1007 for bad UTF-8, 1009 for a message that is too large.
  - List the 1 MiB message cap in the README.
  - Effort: small.
- **libvips error buffer pollution** (vips.rs:195). `vips_object_get_argument_flags` logs "no property named `page'" for every JPEG and PNG. After about 280 variants the 10 KB buffer is full and real errors are cut off. Use `vips_object_get_args` instead. Also:
  - Replace the panicking `Once` with `OnceLock<Result<(), String>>`, and keep init lazy (initializing at boot costs 2.6–13.5 MB Pss).
  - Use `i32::try_from` at process.rs:52.
  - Add SAFETY comments to the 18 blocks.
  - Effort: small.
- **Web Push pool is not panic-safe** (pool.rs:31-138). A panic inside `deliver` leaks `pending`. Use a `QUEUE_SLOTS` semaphore with owned permits instead of the counter plus `Notify`. Effort: small.
- **`run_write` writes ROLLBACK by hand** (database.rs:143). Use `rusqlite::Transaction::new_unchecked(conn, Immediate)`, keep the writer's `catch_unwind` as a backstop, and add a panic-rollback test. fixtures.rs:147's own-transaction branch never runs; delete it. Effort: small.

### 3.3 Allocation-free idioms on render and request paths

- **View helpers.** 18 `push_str(&format!(..))` sites and a char-by-char escaper.
  - Add `Attrs::render_into` and build each tag in one buffer (tag.rs:190-240).
  - Make `push_escaped` use ErbEscaper's loop.
  - Write percent-encoding hex from a table.
  - Microbenchmark: 7 attributes go from 445 ns to 153 ns (system malloc).
  - Don't change `raw` to `impl Into<String>`: askama passes `&&str`, which is E0277. Use `|safe` in application.html and `Safe(..)` at the owned call sites.
  - Bench sidebar and room_show. Effort: small.
- **`Dom::attr` allocates a String for every attribute it compares** (dom.rs:175). Add `Attr::is_named` and make `ancestors` a `successors` iterator. This is 11× faster in isolation but invisible in the app profile, so it is a tidy-up. Effort: trivial.
- **Sanitizer allowlists are rebuilt as Vecs on every call** (sanitizer.rs:40-107). Use `LazyLock<SafeList>` accessors returning `&'static`. autolink.rs:245 also re-sanitizes the input it just sanitized. Effort: trivial.
- **Static assets are copied on every serve**: `Body::from(served.body)`, not `into_owned()` (app.rs:175). Effort: trivial.
- **Ctx.** Cache `formats` in a `OnceLock` and return `&[Format]`. Take `&self` in the render, head and send helpers (ctx.rs:240-339). Drop the unneeded `param_str` clones (active_storage.rs:37, 65). Skip caching host and port on `Request`. Effort: trivial.
- **kit file stream**: `ReaderStream` (above). Treat it as a readability change, not a performance one.

### 3.4 Std and small idioms

- **Semaphores.** `static PARSES: Semaphore = Semaphore::const_new(..)` (opengraph.rs:73), and `LazyLock<Semaphore>` without the `Arc` in active_storage.rs:232.
- **Cable.**
  - `Vec::extract_if` in `stop_stream_from` (channel.rs:139).
  - `std::mem::take` instead of `append`/`drain` (connection.rs:304, 356, 165), which frees about 2 KB per connection.
  - Make `pubsub` and `socket` `pub(crate)` (lib.rs:13, 15) and delete `Subscriber::broadcasting`.
- **`<[u8]>::utf8_chunks`** behind a `from_utf8` fast path (opengraph/html.rs:33).
- **std IP predicates** in Ban (ban.rs:128-152). Keep the hand-written `mapped_v4`, which matches Ruby's `1::ffff:10.0.0.1`, and add tests for it.
- **`rusqlite::OptionalExtension::optional()`** instead of four hand-written `QueryReturnedNoRows` mappings. Re-export db's `query_all`, `query_one` and `placeholders` for campfire. Never name a trait method `query_one`: rusqlite's inherent method of that name shadows it and errors on zero rows.
- **Timestamp.**
  - `Display` should write the fields directly; today pre-1970 values print `.-00001`.
  - Use `Offset::UTC.to_timestamp` (time.rs:62-129).
  - `iso8601_millis` should be `strftime("%Y-%m-%dT%H:%M:%S%.3fZ")` (metadata.rs:70).
- **AES-GCM detached, in-place API** (message_encryptor.rs:39-62). Keep the length check before `Tag::from_slice`.
- **kit front.**
  - Store the cache as `IndexMap<Box<str>, Entry>` (cache.rs:56). Use `Box<str>`, not `String`, so capacity stays charged against the size limit.
  - Give FrontConfig real types: `usize` and `Option<NonZeroU64>` (config.rs:22-100).
  - Add `header_get` and `request_host` helpers, but keep deflater's Option semantics.
- **jobs `Handler`** as `Box<dyn Fn(App, Event) -> BoxFuture<..>>` (jobs.rs:71-107).
- **Small cleanups:**
  - `hex::encode(rand::random::<[u8; 16]>())`.
  - `const LAST_MODIFIED = Timestamp::constant(1_293_840_000, 0)`.
  - thiserror for `NilInquiry`.
  - `.await??` at room_messages.rs:87.
  - Drop `..Default::default()` in platform.rs:135.
  - Remove the no-op `Ok::<_, askama::Error>` wrap in directs.rs:95.
- **Dead public API.** 20 unused `pub fn`s (for example kit request.rs:125, db user.rs:185/232, storage storage.rs:270, cable turbo.rs:203-219) and 5 unused `reload` methods. Delete them; decide the Turbo `broadcast_*` family as a set. rustc can't see these across the crate boundary.
- **kit `Crypto` trait, TestCrypto and `test-support` exist only for kit's tests** (crypto.rs:14, testing.rs:29). Hold `Arc<rails_compat::Secrets>` directly and drop the duplicate `crypto` field in `channels::Deps`.
- **Membership bulk inserts** (user.rs:692, room.rs:428). Use one cached `INSERT … SELECT … ON CONFLICT DO NOTHING` (json_each for explicit ids; the `WHERE true` is required). `trim_recent_searches` becomes a single DELETE with a subquery. Leave the per-message IN-list queries alone without a bench.
- **`integer_enum_sql!`** (user.rs:70): take the values from the enum discriminants instead of repeating the literals. Skip the proposed `sql_enum!` DSL.

### 3.5 Formatting and lints

- **rustfmt.**
  - Add a root rustfmt.toml with `max_width = 140` and `use_small_heuristics = "Max"`. That rewrites 847 hunks in 179 files, against 2,601 at the defaults. "Max" is a style choice: it flattens files currently written in default style.
  - Keep the vendored html5ever out with `crates/richtext/vendor/html5ever/rustfmt.toml` containing `disable_all_formatting = true` (stable), or with an explicit `-p` list.
  - Put `#[rustfmt::skip] mod tables;` in storage/src/lib.rs:22, and the same on the translations table.
  - Add `rustfmt` to Dockerfile:101.
  - Reformat in one commit listed in `.git-blame-ignore-revs`. This also fixes about 16 misindented let-chains.
- **`[workspace.lints]`, enforce only the unsafe surface.**
  - `unsafe_code = "deny"` with item-level allows in vips.rs, kit server.rs `raise_open_file_limit`, front/conn.rs `wall_clock`, and main.rs's jemalloc items. There would be almost none left if rustix lands.
  - `clippy::undocumented_unsafe_blocks = "warn"`.
  - Run the machine-applicable pedantic lints once with `clippy --fix` without enforcing them.
  - Add `[lints] workspace = true` and `license`/`publish.workspace` to the 7 manifests that lack them.

## 4. Structure and APIs

1. **Hostile rich text costs quadratic CPU inside the writer** (richtext/src/dom.rs:310, 508, 629; db message.rs:418). A 400 KB body of nested `<div>`s takes 22 s. The Gumbo depth and attribute limits are only checked after html5ever has built the whole tree, and this runs under the SQLite write transaction and again on every cache-miss render.
   - Enforce the limit the way Nokogiri does: check the open-elements stack length (MAX_TREE_DEPTH + 1 for fragments) before each token, and treat the rest of the input as EOF.
   - This needs a small documented vendored-html5ever patch (a read accessor on `open_elems`) plus a cap on the tokenizer's per-tag attribute scan.
   - Delete the redundant attribute dedupe in `Sink::create_element`.
   - Don't use the "final-tree depth with a 2× margin" idea: the adoption agency makes it reject input Gumbo accepts.
   - If you feed the parser in chunks instead, html5ever 0.35 drops a U+FEFF at the start of *every* feed.
   - Add limit cases to the richtext corpus. Today's final-tree check already disagrees with Gumbo in both directions.
   - Effort: medium. Risk: a second vendored patch.
2. **Make rails_compat the base crate for db, richtext, storage and views.** Measured, this costs no wall time: rails_compat finishes at 6.1 s, before any of those leaves starts. It enables:
   - storage using the production verifier. Delete `AppMessageVerifier` and storage's `hmac`, and the storage goldens then test the real signer.
   - db using `rails_compat::password`.
   - a single JSON encoder, ContentDisposition and clock.

   Risk: a change to rails_compat rebuilds more crates incrementally, in parallel with kit's 4 s.
3. **One Clock** (db time.rs:144-207, kit clock.rs:12-29, campfire app.rs:210 `DbClock`).
   - One trait returning `jiff::Timestamp` in rails_compat, with db truncating to microseconds in `Env::now`.
   - Merge TestClock and FrozenClock into one test clock and delete the adapter.
   - Today the channel test harness gives the database a TestClock and cable a SystemClock.
   - Effort: small; depends on item 2.
4. **Test seams.**
   - Move `impl Default for Env`, NullSink, RecordingSink and TestClock into a `db::testing` module behind `test-support`.
   - BasicRichText stays: it is `AppRichText::to_plain_text`'s production fallback (campfire rich_text.rs:42).
   - Either make that fallback return `""`, like `mentioned_user_ids` already does, or propagate a typed error. The typed error only works with presenter changes (presenters.rs:225), or one bad message turns whole room and search pages into 500s.
   - Drop the `UserNames` parameter.
   - Make BasicRichText's `mentioned_user_ids` return nothing. It accepts unsigned SGIDs that production rejects, so db's tests currently assert behaviour that doesn't ship; have them pass ids directly.
   - Delete the dead rails_compat unverified-SGID copy and move its golden test to richtext.
5. **Controllers.**
   - The database helper from §3.2.
   - A detached-render helper for the 5 broadcast sites plus jobs.rs:131. Leave out `messages::create`, which builds URLs with `url_for` including the port.
   - Delete `Partials`/`Rendered` (page.rs:117, broadcasts.rs:16). Broadcasts take `&str`. Direct rooms take an `FnMut(&Membership) -> Result<String>`, which also drops a duplicate memberships query. Involvements render only on Prepend, which is what Rails does.
   - `framed_page!` evaluates the template in two closures, which forces field clones. Make it one `FnOnce(&ViewContext, bool)` closure and move the values. This also replaces three hand-written expansions (users.rs:82, sidebars.rs:42, concerns.rs:418).
   - Move the SQL out of controllers into campfire_db.
     - `PushSubscription::find_for_user_by` should take a fixed column set. Today column names come from request keys after `permit`, and multi-parameter keys like `endpoint(1i)` reach the SQL.
     - Use the existing `Attachment::find_for`/`delete`.
   - `ctx.param_integer(k)` for the 15 `param_str(..).and_then(cast_integer)` sites. Keep `Option<Option<T>>` for patch fields.
   - kit owns the params invariant through `set_path_params`, and the fields become `pub(crate)`. Don't remount dispatch as the router fallback: that breaks `RawPathParams` and `kit::app`'s own fallback.
   - Sign-in rate limiter (sessions.rs:99): move it into AppState, sweep once per window plus a size trigger, and reset on read.
6. **Hot paths.** Each needs before-and-after numbers under bench/results.
   - **Routing** (controllers.rs:329). Add an `is_match` check before `captures` (trivial, most of the gain), then optionally one `RegexSet` per verb for about 10×. `recognize` is 1.4–1.7% of room_show, messages_page and search.
   - **Page parts** (kit deflater/splice.rs):
     - Identify text parts by their bytes (foldhash plus a compare) with the SHA stored beside them. `text_part`'s SHA-256 is 6.66% of room_show. Give the map a per-entry cap and a budget sized to the real working set; the bench is a 100% hit by construction.
     - In `locate` (:238), scan a short window after the previous fragment before building a memmem Finder. `Finder::new` is 2.2–2.9%; locate went from 14.3 µs to 6.6 µs.
     - Presize the page buffer with fragment lengths plus a margin. Realloc growth is about 2.3%.
     - Key a piece's predecessor by the fragment's SHA and delete the `_pin` Weak fields.
     - Bound FRAGMENTS with two generations instead of `retain` plus a `clear()` cliff at 8,192.
     - Have `pieces()` build `Vec<Bytes>` without the `expect`.
   - **Database.**
     - Run reads on reader threads, one per reader connection, that take them from one queue, instead of on `spawn_blocking` (database.rs:311), where a read that finds every connection busy parks a blocking thread. Waiting on an async semaphore before `spawn_blocking` also bounds the threads, but gives every contended read one more hand-off between threads, which cost read-heavy routes 13–22% of their throughput at 16 and 64 concurrent requests (bench/results/db-hops-20260930).
     - Merge `Layout::load`'s two reader trips (view_context.rs:39-55), and search's extra one.
     - Skip the re-read after a text-only `create_message` (messages.rs:249).
     - For the hot models (Message, and Membership with Room), use a column-list const plus positional indices generated by one macro. `column_index` is 3–5% of messages_page and room_show.
   - **User-Agent** is parsed twice per page and reallocates per product (concerns.rs:391, 430; user_agent.rs:23). Parse once and lazily, scan `&str`, and add an ASCII fast path to the case-insensitive compare. The loadgen sends no UA, so benchmark with one.
   - **Media subprocess polling**: cap the poll interval at 5 ms instead of 50 ms (storage/process.rs:139). Today it adds 29–39 ms to every preview.
   - **Uploads** read and MD5 the file twice. Pass `None` to `upload` for local files (storage.rs:89) and note it in the README.
7. **Cable memory and contention.** Every benchmark logs in as one user, so these costs are invisible today. Add a many-users loadgen mode first.
   - Every per-user stream allocates an 8.3 KB broadcast ring, 4 per user, about 33 KB per user.
     - Step 1: subscribe the internal channel with capacity 1 (connection.rs:83).
     - Step 2: a name-based capacity policy for `user_<id>_reads` and `<gid>:rooms`.
     - Keep `user_<id>_unreads` at 256, because it carries every post in every room the user is in.
     - Together about 25 KB per user.
   - Stop keeping the encoded identifier on each subscription, key the hub by `Arc<str>`, and adopt the group's `Arc`: about 1.2–1.9 KB per connection.
   - `Hub::broadcast` holds the hub mutex through the whole wake wave (pubsub.rs:58-75). Snapshot the groups and send after releasing it.
   - The heartbeat task runs on the app runtime (server.rs:259), so every 3 s an app worker stalls about 40–50 ms at 100k clients. Spawn it on `connections_runtime()`.
   - Bug: a `::RoomChannel` subscription streams from `:room:<gid>` (connection.rs:276). The registry should return its canonical `Arc<str>` name. This is visible for TypingNotifications.
8. **Front server.**
   - Key the response cache on the raw query (cache.rs:193). `encode_query` drops pairs containing `;`, so `/rails/active_storage/blobs/proxy/..?disposition=attachment;` gets cached under the bare URL for 100 years. The fix also deletes about 65 lines. Add a README line.
   - HTTP/1 keep-alive connections close after HTTP_READ_TIMEOUT (30 s), not HTTP_IDLE_TIMEOUT (60 s), because hyper's header timer also runs while idle. Fix the docs and the README, and change the test to idle = 3, read = 1. Don't build a Go-style read-deadline wrapper.
   - Turn on rustls session tickets for the main config only (tls.rs:28-42). This restores what Thruster did.
9. **Integrations and storage.**
   - `Endpoint::for_uri(&Uri, Option<IpAddr>) -> Result<Self, &'static str>` plus `uri_host()` (fetch.rs:83, webhook.rs:108). Leave web_push's literal, which forces TLS like the gem does.
   - opengraph `Metadata` as a `serde_json::Map` plus `rails_compat::json::encode` (metadata.rs:20-156).
   - Search sanitizing: `[^\w&&\p{Age=15.0}]` matches the 776-line generated `word_ranges.rs` exactly (0 mismatches over every scalar value), so the table and its oracle script can go.
   - Push: add `PushSubscription::permitted_endpoint_host()` and `validate(Option<IpAddr>)`. The controller's Mutex-and-HashMap resolve dance (push_subscriptions.rs:121-141) goes away.
   - Move Blob#destroy's SQL and `delete_attachment` into campfire_storage. Today it is duplicated at active_storage.rs:618 and attachments.rs:140.
   - Warm the ffmpeg probe at boot in `spawn_blocking`, with a warning when ffmpeg is missing. Leave libvips lazy: its only probe, `vips::version()`, initializes it (2.6–13.5 MB Pss, §3.2).

## 5. Suggested order of work

1. **SQLite upgrade.** rusqlite 0.40.2 with explicit features, dropping `functions` and `serde_json`.
   - Findings: rusqlite-0-40, db-dependency-hygiene.
   - Benefit: fixes the WAL-reset corruption bug and removes the hashbrown duplicate.
   - Risk: the query planner changed; bench room_show, POST, sidebar and messages_page.
2. **Hostile-input parse cost.** Gumbo-faithful open-elements check, the attribute-scan cap, and corpus cases for the limits.
   - Finding: gumbo-limits-after-parse.
   - Benefit: closes a DoS that holds the writer lock.
   - Risk: medium; one more documented vendored patch.
3. **Small correctness fixes.**
   - libvips error buffer and init as a `Result`.
   - Cable canonical class name.
   - Cache keyed on the raw query.
   - Proxy Content-Disposition through storage's formatter.
   - Panic-safe push pool.
   - `clone_error`.
   - HTTP/1 idle docs and test.
   - rustls session tickets.
   - Risk: low; almost all of these move toward Rails or Thruster.
4. **Manifest diet.**
   - axum features, tower-http, rusqlite features (if not done in step 1), the single html5ever, unused and redundant deps, optional `crc32fast`, hdrhistogram plus `runtime_detection`. Commit Cargo.lock.
   - Benefit: 8–9 fewer crates in the release build.
   - Risk: near zero.
5. **Tooling.**
   - rustfmt config, the vendor exclusion, the Dockerfile component and `.git-blame-ignore-revs`.
   - Minimal workspace lints plus SAFETY comments in vips.rs.
   - CI:
     - `cargo test --no-fail-fast`, a fmt check and cargo-shear.
     - Scheduled advisory scan and clippy for the loadgen (fix its 4 warnings).
     - Media-vector guard: pass `--env CI` into the container, or set `CAMPFIRE_REQUIRE_MEDIA_VECTORS` in the toolchain image. Without it the guard never fires.
     - A visible "seed not built" notice written directly to stderr, plus `CAMPFIRE_REQUIRE_SEED`.
   - Risk: one large mechanical diff; land it when few branches are open.
6. **Fix the Ruby-semantics drifts in place** (the list in §3.1, step 1), with values probed in the reference.
   - Risk: changes only unusual inputs, and always toward Rails.
7. **rails_compat as the base for Rails contracts.**
   - Public `json` and `encoding`, storage on the production verifier, db on `password`, the `content_disposition` module (plus the dump_tables.rb change), delete the dead SGID copy, one Clock, and remove kit's Crypto trait.
   - Benefit: the goldens test the code that runs; db drops `bcrypt` and storage drops `hmac`.
   - Risk: JSON on the cookie path must stay byte-identical; the vectors guard it.
8. **`ruby_compat` leaf crate.**
   - `erb`, `to_i`/`integer_cast`, `to_f`, `strip`, `float_to_s`, the escapers and Rack `byte_ranges` (delete kit's `parse_range`), with ruby_core oracle vectors.
   - Benefit: about 250–400 lines fewer.
9. **Errors and the database helper.**
   - `Error::other`, `is_record_not_unique`, one `storage_error`, `AppState::read`/`write` (404/422), Transaction RAII, `.optional()`, and jobs Handler closures.
   - Risk: about 105 mechanical sites. Review association accessors that can now return 404.
10. **Hot-path performance, one measured PR each.** Routing `is_match` (then RegexSet), the `locate` window, the presized page buffer, sha2 `asm` on arm64, text identity by bytes, predecessors by SHA plus FRAGMENTS generations, reader threads, merged reader trips, UA parsed once, positional columns for hot models, the subprocess poll cap, the single MD5, and the zero-copy static body.
11. **Cable memory.**
    - Many-users loadgen mode first. Then internal ring capacity 1 plus the name policy, `mem::take`, shared identifiers, futures-channel for the reader, the hub snapshot, the heartbeat on the connections runtime, and typed read errors with close codes.
    - Record Pss at 10k and 100k clients.
12. **Library swaps.** fancy-regex → `regex::bytes`, x509-cert (with new tests), the `idna_adapter` pin, askama 0.16 (bench), zstd 0.14, jemallocator 0.7 (bench memory), and serde_yaml → yaml-rust2 (after step 1).
13. **Idiom sweep** (§3.3 and §3.4):
    - Ctx receivers and `formats`; `render_into` and the escaper; `is_named`; static SafeLists.
    - tokio-util stream and shutdown; the IndexMap cache; typed FrontConfig; header helpers.
    - sec1; AES-GCM detached; Timestamp `Display`; ISO 8601 millis; std IP predicates; `utf8_chunks`; `extract_if`.
    - `pub(crate)` modules and dead `pub fn`s; rustix; parking_lot (optional).
    - The askama avatar macro, typed Involvement/RoomKind, and views' own base64 and CRC-32.
14. **Controllers and integrations structure.**
    - Detached-render helper; drop `Partials`; single-closure `framed_page!`.
    - SQL into db, with a fixed column set for the push lookup; membership `json_each`; `param_integer`; rate limiter in AppState.
    - `Endpoint::for_uri`; `Metadata` as a Map; the `Age=15.0` search regex; push `permitted_endpoint_host`; storage destroy SQL into campfire_storage.
15. **Test harness.**
    - Consolidate campfire's seed helpers and the typed session vectors.
    - An oracle CI job: fix reference-tools/db/differential.sh for `schema::ADDITIONS` first, and keep the job off the required checks.
    - Seeded Accept, Accept-Encoding and URI corpora. Expect the divergences found while verifying.
    - Remove the orphaned `csrf` vectors with `jq`; regenerating would churn every random IV. Add an index to vectors/README.md.
    - Merge views' four test binaries, gate db's test doubles, collect mismatches in the Marcel and routing loops, and run Autobahn once against an echo example.
16. **Loadgen.**
    - Record CPU per phase.
    - Strict `Args`. Fix the double hold by renaming the trailing sleep and keeping `--hold-secs` as the idle hold.
    - A `Fleet` struct and a real 50-handshake gate with a timeout. Do this in the next full benchmark round, because the connect numbers will change.
    - Closed-loop labels in the report.
    - The small idioms, the u64 length in the deflate client, and the stale gzip comments.
17. **Later: the RustCrypto, rand, bcrypt and p256 batch**, after step 4.

## 6. Not worth doing

- Web Push `as u16` port truncation (dropped). It can't happen: validation requires port 443 before that code runs.
- Splitting the loadgen's 977-line `main.rs` into modules (dropped). It costs churn for nothing.
- Replacing the hand-written pieces in §2 "Keep" with reqwest, the web-push crate, ammonia, scraper, lol_html, matchit, moka, quick_cache, r2d2, deadpool, the `libvips` crate, tower-http's CompressionLayer, rustls-acme, clap, envy, figment, woothee, ua-parser, blake3, serde_rusqlite, strum, libtest-mimic, datatest, insta, cargo-fuzz, proptest or pico-args.
- `clippy::pedantic` wholesale, or `unwrap_used`, `indexing_slicing`, `unreachable_pub` and `use_self` (about 1,600 edits with no value). Run the three cast lints once and triage instead.
- Making Ctx dispatch the router fallback, or moving routing into kit. The fallback breaks `RawPathParams` and collides with `kit::app`'s own fallback.
- Caching host and port on `Request`.
- The `sql_enum!` macro DSL; the finder and `reload` macro and a partial-UPDATE builder; `Deref for Tx`; switching to `Connection::open`'s default flags; renaming to a `ConnectionExt` trait.
- rusqlite's `jiff` feature; a cross-crate SecureRandom module; ERB escaping inside rails_compat; moving `rails_compat::encoding`.
- Replacing `storage::Json` with `serde_json::Value`, or porting fpconv's Grisu2 digit generation. Both are churn with no user-visible change.
- A Go-style HTTP/1 read-deadline IO wrapper. It touches the slowloris protection for about one saved reconnect per minute.
- Moving `is_previewable` onto `Storage`, or calling `vips_init` at boot (2.6–13.5 MB of extra Pss).
- Deleting BasicRichText (it is the production fallback), or giving db a dev-dependency on campfire_richtext (it would duplicate `DbResolver`).
- Cable: removing the per-socket reader task (an extra poll on every delivery), an mpsc publisher task for the hub, a single-frame fast path in `Writer::send`, making drop order depend on field order, replacing `SelectAll` with `pending` (the `Unfold` panics after returning None), and an `Identity` marker trait.
- Counter ids for fragments. Keying by SHA is simpler and survives `clear()`.
- A multi-chunk gzip body, combining per-piece CRCs, or blake3.
- A `DomId` Display type, or converting `descendants`/`element_children` to iterators.
- percent-encoding `AsciiSet`s for the Content-Disposition and Journey escapers. The byte-list closures read better.
- Pointing Autobahn at /cable directly, or differential tests against tungstenite.
- A `build.rs` cfg for seed-gated tests, collecting mismatches in the views parity loops, a `--rate` open-loop mode in the loadgen, or removing miniz_oxide from `loadgen gzip`.
- Merging crates, or splitting integrations out of the binary without measuring incremental builds first.
- `Env::new` or `Env::for_tests` constructors, `IpAddr` keys for the rate limiter, or `String` keys in the IndexMap cache (use `Box<str>`).
