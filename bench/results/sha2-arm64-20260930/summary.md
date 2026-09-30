# SHA-256 instructions on arm64 (2026-09-30)

sha2 0.10.9 picks its SHA-256 backend at compile time (`src/sha256.rs`): on x86_64 it always builds
the SHA-NI backend, which checks for the instructions at runtime; on aarch64 it builds its ARMv8
backend (the `sha2` instructions when `cpufeatures` finds them, the software rounds otherwise) only
with the `asm` feature, and otherwise the software rounds alone. So the arm64 image hashed every
page's text parts, ETags, cookie HMACs and asset digests in software. `00f9744` turns on `asm` for
`cfg(target_arch = "aarch64")` in crates/kit/Cargo.toml; features unify, so every sha2 user gets
it.

- On aarch64, `asm` also builds sha2-asm 0.6.4 (MIT, RustCrypto), whose build script assembles one
  `.S` file with `cc -march=armv8-a+crypto`. The Dockerfile's `rust:1.98.1-trixie` build stage has
  gcc. sha2 0.10.9 doesn't call that object for SHA-256 on aarch64 (its `aarch64` module uses
  inline intrinsics); building it is the price of the feature.
- x86_64 is unchanged: `cargo tree --target x86_64-unknown-linux-gnu -e features -i sha2` lists
  `default` and `std`, before and after; for aarch64 it adds `asm`. Cargo.lock gains the sha2-asm
  entry (its one dependency, `cc`, was already there), which only aarch64 builds compile.
- The digests are the same, so nothing stored or signed changes.

## On ARM: still to be measured

This host has no ARM CPU, and the numbers that decide this change have to come from one (the
`ubuntu-24.04-arm` runner the publish workflow uses, a Raspberry Pi 5, or a Graviton instance). At
`62394bb` or later, three interleaved rounds of:

    cargo run --release -p campfire_kit --example sha256_bench --features sha2/force-soft   # before
    cargo run --release -p campfire_kit --example sha256_bench                              # after

`force-soft` selects exactly the code aarch64 ran before (`soft::compress`, what sha2 uses there
without `asm`). Both must print the same "digest of every length to 2 KB" line, and the after run's
first line should say the CPU has the SHA-256 instructions: Graviton, the Cobalt 100 behind GitHub's
arm runners and the Pi 5's Cortex-A76 have them; a Pi 4 doesn't, and keeps the software rounds at
runtime. Then once, on the same machine:

    CAMPFIRE_REQUIRE_SEED=1 cargo test --workspace --exclude html5ever

(with a seed from `parity/bin/seed build`, or a copy of another machine's `parity/.seed`), or build
the arm64 image. Put the medians of the
three rounds in the table below. Until then this change is unmeasured on the hardware it's for.

| Size | Software (before) | ARMv8 SHA-256 (after) | Ratio |
|---|---|---|---|
| cookie HMAC input (64 B) | | | |
| message fragment (1 KB) | | | |
| room page text (34 KB) | | | |
| whole room page (416 KB) | | | |

## x86_64 as a proxy

The same example on this host's AMD Ryzen AI Max+ 395, `force-soft` against the default build
(SHA-NI): the gap between software rounds and the instructions, for scale. Built once each, run
interleaved, three rounds, with other agents' builds keeping the load average at 3–6. Medians of
the three rounds; raw output in [`runs.txt`](runs.txt).

| Size | Software | SHA-NI | Ratio |
|---|---|---|---|
| cookie HMAC input (64 B) | 226 ns | 57 ns | 4.0× |
| message fragment (1 KB) | 1.95 µs | 444 ns | 4.4× |
| room page text (34 KB) | 58.0 µs | 14.1 µs | 4.1× |
| whole room page (416 KB) | 705 µs | 172 µs | 4.1× |

How much ARM's instructions gain over its software rounds, and how much of a request that is,
depends on the core, which is why the table above needs filling in.

## On emulated arm64

This checks the build and the digests, not the speed. At `62394bb` the whole app was cross-built
for aarch64 in `rust:1.98.1-trixie` (rustup's aarch64 target, Debian's cross gcc and
`libvips-dev:arm64`, jemalloc with 16 KB pages as the Dockerfile sets for arm64), and the example
and the full test suite ran under qemu-aarch64, whose CPU has the SHA-256 instructions, so the asm
build took the instruction path:

- `cargo build --locked --release -p campfire --target aarch64-unknown-linux-gnu` compiles sha2-asm
  and sha2 with `asm` and links the app.
- The example prints the same digest line from the asm and force-soft builds, and the same as on
  x86_64 (`60a76f1a…664f58a14`). qemu runs the SHA-256 instructions through helper calls, so its
  timings (asm slower than soft) say nothing about real ARM CPUs.
- `CAMPFIRE_REQUIRE_SEED=1 cargo test --release --workspace --exclude html5ever`: 713 passed,
  4 failed, and the four are the emulator's: qemu-user rejects `PR_SET_THP_DISABLE`, two richtext
  hardening tests exceed their time budgets under emulation, and the storage vectors need an
  `ffprobe` the build container didn't have. Everything that hashes passed: the Rails vectors
  (message verifiers and encryptors, signed and encrypted cookies), ETags and page parts, asset
  digests, and the app's seeded tests.

The commands and output are in [`runs.txt`](runs.txt).

## Over HTTP

Only on ARM: the x86_64 build doesn't change. To be added by the coordinator with the ARM numbers.
