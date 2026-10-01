# 100,000 cable clients, and a Raspberry Pi 5's budget

How many chat clients one Campfire can hold, with a Raspberry Pi 5 (4 × Cortex-A76, 8 GB, NVMe,
gigabit Ethernet) as the target. No Pi was available: this is a resource-budget approximation,
not hardware emulation. The app runs on 4 pinned
cores of a Ryzen AI Max+ 395 under a cgroup quota of 1.2 cores (`CPUQuota=120%`,
`PI=1` in [`run100k.sh`](run100k.sh)) and 8 GB. By Geekbench 6 a Pi 5 core is about 3.5× slower than one
of these, so its four cores are worth roughly 1.2 here. The network can't be throttled without
root, so the bytes on the wire are measured and compared with gigabit Ethernet instead.

## Method

[`run100k.sh`](run100k.sh): the app (native release build) on a database filled by
[`../splice-20260927/fill.py`](../splice-20260927/fill.py) (80 messages in one room), and
`loadgen cable` on 12 other cores:
- 100,000 clients, spread over several loopback source addresses (one address has only ~28k
  ports towards one server port). Each subscribes to six channels, as a room page does: presence,
  unread rooms, heartbeat and three Turbo streams, all for one room.
- The phases are connect, a 30 s idle hold, 10 paced posts (one every 200 ms) for delivery
  latency, then 10–15 s of two closed-loop posters for throughput. Every post is a ~10 KB message
  that reaches all 100,000 clients.
- With `--deflate 1`, a client offers `permessage-deflate` as browsers do, inflates what it gets
  and counts bytes on the wire.

## Results, in order of the changes

| Build | Memory, idle | Memory, peak | Deliveries/s | Post reaches all, p50 | POST p50 |
|---|---|---|---|---|---|
| `main` (tungstenite, axum's WebSocket) | 3.2 GB | 5.9 GB | 300k | 411 ms | 637 ms |
| Own framing, shared frames | 2.9 GB | 2.9 GB | 410k | 311 ms | 470 ms |
| + drop the upgrade request after connecting | 1.95 GB | 1.95 GB | 420k | 316 ms | 458 ms |
| + identifiers kept once, params parsed on demand | 1.44 GB | 1.44 GB | 430k | 308 ms | 463 ms |
| + connections on their own runtime (compressed clients) | 1.48 GB | 1.60 GB | 1.08M | 316 ms | 43 ms |
| **Same, on a Pi 5's CPU budget** (compressed clients) | 1.52 GB | 1.63 GB | 1.16M | 1,270 ms | 49 ms |

Every run connected all 100,000 clients with no failures, in 13–17 s. The deliveries/s from 1.08M
up are the load generator's limit (11–12 cores busy); on the Pi budget the app used 0.71 of its
1.2 cores at that rate. Post-to-everyone latency on the Pi budget is mostly the load generator
reading 100,000 sockets, not the app. Raw results are the `cable-*.json` files here.

On the Pi budget:
- idle 100,000 clients (heartbeats every 3 s, nothing else) cost 0.16 of the 1.2 cores;
- connecting all 100,000 took 12.9 s at 0.9 cores;
- a message frame was 2,284 bytes on the wire compressed, against ~10 KB uncompressed.

Memory figures above are the application's Pss from `/proc/PID/smaps_rollup`, not total system
memory: they exclude kernel socket structures and TCP buffers. The 8 GB cgroup limit also accounts
for kernel memory and TCP buffers, but their usage and total cgroup memory were not recorded.
The run used loopback networking, plain HTTP without TLS, and one user for all clients.

## Estimated network budget

At an assumed usable ~117 MB/s, gigabit Ethernet allows about 51,000 compressed deliveries a
second (12,000 uncompressed) for these message sizes. This is a bandwidth estimate; it does not
establish that a Pi's CPU, memory or network stack can sustain those rates.

A delivery is one message to one client, so the load is messages/s × people in the room.

| 100,000 chatters | Deliveries/s at one message per person every 5 minutes | Fits gigabit? |
|---|---|---|
| 1,000 rooms of 100 | 33,000 | yes (65%) |
| 100 rooms of 1,000 | 333,000 | no |
| Everyone in one room | at most 0.5 messages/s in total | — |

The tested Linux host held 100,000 connections under the stated resource caps. Capacity on a
real Pi 5 remains unverified, including conversations spread across smaller rooms. A single room
of 100,000 would require about 230 MB of traffic per message with these compressed frame sizes.

Linux has no 65,535-connection ceiling for a listening port: TCP connections are distinguished
by local and remote address/port pairs. The app raises its soft file-descriptor limit to its hard
limit; the hard limit must exceed 100,000 with room for other files. System file-handle limits,
kernel/socket memory, TLS and real network conditions also need checking on the target Pi.
See the [TCP specification](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.4.1),
[Linux file-handle limits](https://docs.kernel.org/admin-guide/sysctl/fs.html#nr-open), and
[cgroup memory accounting](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory).

## Not yet measured

- On real Pi hardware, with TLS, external load generators and total kernel/socket memory measured.
  The current Dockerfile configures arm64 jemalloc for 16 KB pages, but that image was not tested here.
- With 100,000 distinct users (here all clients are one user, so presence writes hit one row).
- Page loads and posts from many users at once, on top of the fan-out.
