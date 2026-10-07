# Verified comparisons

`compare.rb` runs the eight production implementations on fresh copies of the complete Rails
seed. Clone the implementations into sibling directories, build their current production images,
and build the shared client:

```sh
cargo build --release --manifest-path bench/loadgen/Cargo.toml --target-dir target/bench
ruby bench/compare.rb --apps rails,django,laravel,express,elixir,go,rust,c
```

Defaults: three alternating rounds, 16 concurrent clients, four server threads (`8-11`) and
separate client threads (`12-15`). `<APP>_IMAGE` overrides an image; `<APP>_BENCH_ENV` adds
runtime settings as JSON. `--help` lists seed, routes, duration and output options.

Every warmup and measured response must pass its route contract: status and content type,
valid gzip, complete pages, exact seeded message windows and DOM identities, visible rooms,
and unchanged static/avatar bytes. Each POST must render its unique request body into the
actual room container. The audit then verifies its exact message ID, room, stored body and
FTS entry; duplicate acknowledgements, missing writes and database corruption fail the run.
Identical wire bytes reuse a prior full validation by exact equality; changed responses are
always checked. Failed runs produce no comparison summary. Results stay in ignored `tmp/`.

`bench/run` retains the Rails/Rust memory, Cable and upload comparison and uses the same HTTP
contracts and write audit. `bench/report` refuses HTTP results without these contracts.

Tooling checks:

```sh
cargo test --manifest-path bench/loadgen/Cargo.toml
ruby bench/test_check_sample.rb
ruby bench/test_validate_acks.rb       # requires the sqlite3 CLI
```

The shared functional browser gate runs against a disposable fresh production server:

```sh
cd parity && npm ci && npx playwright install chromium && cd ..
node parity/browser-smoke.mjs --base http://127.0.0.1:8080
```

It creates users, rooms and messages; use an empty test installation. It checks live
two-tab delivery, edits, copied links, bots, custom CSS, device sign-in transfers,
invitations and direct pings, and fails on browser JavaScript errors.
