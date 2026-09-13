"""Prepare a small display atlas from official MaleCNS neuron skeletons (CC BY)."""

import concurrent.futures
import hashlib
import json
import urllib.request
from pathlib import Path

import numpy as np
import pyarrow.feather as feather

ROOT = Path(__file__).resolve().parents[2]
SOURCE = "https://storage.googleapis.com/flyem-male-cns/v1.0/segmentation/skeletons-malecns/skeletons-swc/"


def main():
    rows = sorted(
        (
            r
            for r in feather.read_table(
                ROOT / "data/raw/annotations.feather"
            ).to_pylist()
            if r["status"] == "Traced" and r["superclass"]
        ),
        key=lambda r: r["bodyId"],
    )
    selected = []
    for cls, count in [
        ("ol_sensory", 48),
        ("ol_intrinsic", 80),
        ("visual_projection", 80),
        ("cb_intrinsic", 96),
        ("descending_neuron", 16),
        ("cb_motor", 16),
    ]:
        for side in ["L", "R"]:
            group = [
                (i, r)
                for i, r in enumerate(rows)
                if r["superclass"] == cls and (r["rootSide"] or r["somaSide"]) == side
            ]
            if group:
                selected.extend(
                    group[i]
                    for i in np.linspace(0, len(group) - 1, count // 2, dtype=int)
                )
    cache = ROOT / "data/raw/skeletons"
    cache.mkdir(exist_ok=True)

    def fetch(item):
        index, row = item
        p = cache / f"{row['bodyId']}.swc"
        try:
            if not p.exists():
                with urllib.request.urlopen(SOURCE + p.name, timeout=20) as response:
                    p.write_bytes(response.read())
            raw = p.read_bytes()
            nodes = {}
            for line in raw.decode().splitlines():
                if line and not line.startswith("#"):
                    v = line.split()
                    nodes[int(v[0])] = ([float(x) for x in v[2:5]], int(v[6]))
            segments = []
            for point, parent in nodes.values():
                if parent in nodes:
                    end = nodes[parent][0]
                    if all(
                        0 <= q[0] <= 97000 and 0 <= q[1] <= 55000 and 0 <= q[2] <= 62000
                        for q in [point, end]
                    ):
                        segments.append(
                            [
                                [
                                    round((q[0] - 48500) / 48500, 5),
                                    round((q[1] - 27000) / 48500, 5),
                                    round((q[2] - 30000) / 48500, 5),
                                ]
                                for q in [point, end]
                            ]
                        )
            # This is a display sample, not a complete morphology export.
            if len(segments) > 600:
                segments = [
                    segments[i]
                    for i in np.linspace(0, len(segments) - 1, 600, dtype=int)
                ]
            return dict(
                index=index,
                body_id=row["bodyId"],
                group=row["superclass"],
                sha256=hashlib.sha256(raw).hexdigest(),
                segments=segments,
            )
        except Exception as e:
            print("Unavailable", row["bodyId"], str(e), flush=True)
            return None

    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        neurons = []
        for n in pool.map(fetch, selected):
            if n and n["segments"]:
                neurons.append(n)
            if len(neurons) % 48 == 0:
                print("Skeletons", len(neurons), flush=True)
    out = dict(
        source=SOURCE,
        license="CC BY 4.0",
        coordinate_space="MaleCNS EM 8nm; normalized equally on each axis",
        selection="Stratified by class and side in ascending bodyId order; brain bounding box; at most 600 segments per neuron",
        neurons=neurons,
    )
    (ROOT / "assets/brain_atlas.json").write_text(
        json.dumps(out, separators=(",", ":"))
    )
    print(
        "Atlas",
        len(neurons),
        "neurons;",
        sum(len(n["segments"]) for n in neurons),
        "segments",
        flush=True,
    )


if __name__ == "__main__":
    main()
