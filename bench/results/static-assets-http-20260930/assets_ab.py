#!/usr/bin/env python3
"""Static assets A/B for the wave-4 static-body and flate2 branches: native builds interleaved per
rep, each on a fresh seed, loadgen pinned as bench/attrib does. Writes <config>-<rep>.json."""
import json, os, subprocess, sys, time
ROOT = "/home/dhh/Work/basecamp/once-campfire-rust"
sys.path.insert(0, os.path.join(ROOT, "bench", "lib"))
import benchlib as b

configs = sys.argv[1].split(",")
reps = int(sys.argv[2])
out = sys.argv[3]
secs = "8"
os.makedirs(out, exist_ok=True)
b.build_loadgen()
labels = b.snapshot_seed(os.path.join(b.WORK + "-seed"))
bare = f"http://127.0.0.1:{b.PORT + 1}"
JS, MAP, CSS = "/assets/lexxy-a21f41d4.js", "/assets/lexxy-cf040c43.js.map", "/assets/_reset-9c3efd7b.css"
CASES = [  # (name, base, path, gzip)
    ("bare js identity", "bare", JS, "0"), ("bare map identity", "bare", MAP, "0"), ("bare css identity", "bare", CSS, "0"),
    ("front map identity", "front", MAP, "0"),
    ("bare js gzip", "bare", JS, "1"), ("bare map gzip", "bare", MAP, "1"),
]

def run(app, sess, base, path, gzip, conc):
    c0 = app.cpu()
    res, lg_cpu = b.lg("http", "--base", base, "--cookie", sess["cookie"], "--path", path, "--gzip", gzip, "--conc", conc, "--duration", secs)
    c1 = app.cpu()
    n = max(res["latency"].get("n", 0), 1)
    res["cpu_ms_per_req"] = {k: round((c1[k] - c0[k]) * 1000 / n, 4) for k in c0}
    return res

with open(os.path.join(out, "env.txt"), "w") as f:
    f.write(f"date: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}\n")
    for c in configs:
        f.write(f"{c}: {os.environ.get('NATIVE_' + c[len('native-'):].upper() + '_BIN')}\n")
    f.write(f"cases: {CASES}; concs 1,16; secs {secs}\n")
try:
    for rep in range(1, reps + 1):
        for config in configs:
            b.log(f"{config} rep {rep}: starting (load {b.loadavg()})")
            app = b.start(config, os.path.join(b.WORK + "-seed"))
            result = {"config": config, "rep": rep, "load_start": b.loadavg(), "http": []}
            try:
                time.sleep(2)
                sess = b.session(app, labels)
                for name, where, path, gzip in CASES:
                    base = bare if where == "bare" else app.base
                    b.lg("http", "--base", base, "--cookie", sess["cookie"], "--path", path, "--gzip", gzip, "--conc", 4, "--duration", 2)
                    for conc in ("1", "16"):
                        r = run(app, sess, base, path, gzip, conc)
                        r["case"], r["conc"] = name, conc
                        result["http"].append(r)
                        b.log(f"{config} rep {rep}: {name} c={conc} {r['rps']} rps p99 {r['latency'].get('p99_ms')} cpu {r['cpu_ms_per_req']} statuses {r['statuses']}")
            finally:
                app.stop()
            json.dump(result, open(os.path.join(out, f"{config}-{rep}.json"), "w"), indent=1)
finally:
    subprocess.run(["docker", "rm", "-f", b.CONTAINER], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
