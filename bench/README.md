# Benchmarks

Response contracts, the load generator, write audits and shared browser flows live in
[once-campfire-verification](https://github.com/basecamp/once-campfire-verification).
Clone it alongside this repo, follow its setup instructions, then run:

```sh
ruby bench/compare.rb --apps rails,django,laravel,express,elixir,go,rust,c
node parity/browser-smoke.mjs --base http://127.0.0.1:8080
```

Set `VERIFICATION_ROOT` for another checkout location. Shared browser flows require a
fresh disposable server. The local screenshot inventory and Rails parity tools remain
in `parity/`.

`bench/run` retains the detailed Rails/Rust HTTP, Action Cable and upload comparison,
using the shared validated client. Its default results directory is ignored `tmp/bench`.
Historical results preserve the commands and evidence used at the time.
