#!/usr/bin/env python3
"""Upload A/B for perf-media with curl: binary files sent as application/octet-stream (so they're
stored as files, with no image processing), and two 2 s H.264 clips as video/mp4 (ffprobe and the
ffmpeg preview run on the request path), to the bare app, sequentially; native builds interleaved
per rep on a fresh seed. Writes <config>-<rep>.json with each POST's status and total time."""
import json, os, statistics, subprocess, sys, time, uuid
ROOT = "/home/dhh/Work/basecamp/once-campfire-rust"
sys.path.insert(0, os.path.join(ROOT, "bench", "lib"))
import benchlib as b

configs, reps, out = sys.argv[1].split(","), int(sys.argv[2]), sys.argv[3]
here = os.path.dirname(os.path.abspath(__file__))
FILES = [("1 MB binary", "data-1mb.bin", 15), ("10 MB binary", "data-10mb.bin", 11), ("100 MB binary", "data-100mb.bin", 5),
         ("2 s 320x240 clip", "clip.mp4", 15), ("2 s 1080p clip", "clip-1080p.mp4", 15)]
os.makedirs(out, exist_ok=True)
b.build_loadgen()
labels = b.snapshot_seed(os.path.join(b.WORK + "-seed"))
bare = f"http://127.0.0.1:{b.PORT + 1}"

def post(sess, path):
    cmd = ["taskset", "-c", b.LOADGEN_CPUS, "curl", "-s", "-o", "/dev/null", "-w", "%{http_code} %{time_total}",
           "-H", f"Cookie: {sess['cookie']}", "-H", f"X-CSRF-Token: {sess['csrf']}", "-H", "Accept: text/vnd.turbo-stream.html, text/html",
           "-H", "Sec-Fetch-Site: same-origin",
           "-F", f"authenticity_token={sess['csrf']}", "-F", f"message[client_message_id]={uuid.uuid4()}",
           "-F", f"message[attachment]=@{path};type={'video/mp4' if path.endswith('.mp4') else 'application/octet-stream'}", f"{bare}/rooms/{sess['write_room']}/messages"]
    code, secs = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.split()
    return int(code), float(secs) * 1000

try:
    for rep in range(1, reps + 1):
        for config in configs:
            app = b.start(config, os.path.join(b.WORK + "-seed"), {"TMPDIR": os.path.join(ROOT, "target", "tmp-bench-app")})
            result = {"config": config, "rep": rep, "load_start": b.loadavg(), "uploads": []}
            try:
                time.sleep(2)
                sess = b.session(app, labels)
                for name, f, n in FILES:
                    runs = [post(sess, os.path.join(here, f)) for _ in range(n)]
                    ms = [t for _, t in runs]
                    statuses = sorted({c for c, _ in runs})
                    result["uploads"].append({"name": name, "ms": ms, "median_ms": statistics.median(ms), "statuses": statuses})
                    b.log(f"{config} rep {rep}: {name} median {statistics.median(ms):.1f} ms statuses {statuses}")
            finally:
                app.stop()
            json.dump(result, open(os.path.join(out, f"{config}-{rep}.json"), "w"), indent=1)
finally:
    subprocess.run(["docker", "rm", "-f", b.CONTAINER], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
