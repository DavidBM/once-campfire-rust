# Inline reads, recorded page parts and cheaper views, against main (2026-09-30)

The 7 commits of this branch (tip `1bb8d22`) against `main` at `4419b5f`. Both are native release builds with
`CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, on a Ryzen 9 9950X3D. The two builds were interleaved over
5 reps, 10 s per cell, 16 clients. The host was otherwise idle: the other CCD was 2.4% busy. Raw runs are in the
`native-*.json` files, and the full table is in [`report.md`](report.md).

    NATIVE_MAIN_BIN=<main build> NATIVE_PR_BIN=<this branch's build> \
      bench/attrib --configs native-main,native-pr --reps 5 --routes room_show,messages_page,sidebar,post_message \
        --concs 16 --secs 10 --cable ""

Medians:

| Route | CPU ms/req main | branch | Change | req/s main | branch | Change | p99 ms main | branch |
|---|---|---|---|---|---|---|---|---|
| room_show | 0.1202 | 0.0915 | −23.9% | 28,540 | 41,411 | +45.1% | 1.22 | 0.69 |
| messages_page | 0.1029 | 0.0784 | −23.8% | 33,498 | 47,926 | +43.1% | 1.04 | 0.56 |
| sidebar | 0.1031 | 0.0951 | −7.8% | 32,998 | 39,936 | +21.0% | 1.13 | 0.74 |
| post_message | 0.3717 | 0.3149 | −15.3% | 4,825 | 5,318 | +10.2% | 46.2 | 44.8 |

Every route's average response size is the same on both builds (24,231, 16,158, 5,910 and 1,993 bytes).
The pages are byte-identical, and parity against the Rails reference passes: 873 of 874 cells, 1 allowlisted.

Each commit message gives that commit's own figures. They were measured the same way (16 clients, the
commit's build against its parent's, runs interleaved), with a separate interleaving of runs per commit.
