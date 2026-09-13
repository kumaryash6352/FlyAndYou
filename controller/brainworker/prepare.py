"""Import the official traced-neuron graph; never create a dense NxN matrix."""

import hashlib
import json
import time
from collections import Counter
from dataclasses import asdict
from pathlib import Path

import numpy as np
import pyarrow as pa
import pyarrow.feather as feather
import pyarrow.ipc as ipc
from scipy import sparse

from .chromatic import READOUT_SCALE, SENSOR_PROFILE, derive_readout
from .model import Brain
from .motor import MotorConfig

ROOT = Path(__file__).resolve().parents[2]
SOURCE = "https://storage.googleapis.com/flyem-male-cns/v1.0/connectome-data/flat-connectome/"
FILES = {
    "annotations.feather": "body-annotations-male-cns-v1.0-minconf-0.5.feather",
    "neurotransmitters.feather": "body-neurotransmitters-male-cns-v1.0.feather",
    "connections.feather": "connectome-weights-male-cns-v1.0-minconf-0.5.feather",
}


def digest(p):
    with open(p, "rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def main():
    start = time.perf_counter()
    out = ROOT / "data/cache/malecns-v1"
    out.mkdir(parents=True, exist_ok=True)
    rows = feather.read_table(
        ROOT / "data/raw/annotations.feather",
        columns=[
            "bodyId",
            "status",
            "superclass",
            "type",
            "somaSide",
            "rootSide",
            "assignedOlHex1",
            "assignedOlHex2",
        ],
    ).to_pylist()
    kept = sorted(
        (r for r in rows if r["status"] == "Traced" and r["superclass"]),
        key=lambda r: r["bodyId"],
    )
    ids = np.array([r["bodyId"] for r in kept], np.int64)
    n = len(ids)
    nt = feather.read_table(
        ROOT / "data/raw/neurotransmitters.feather", columns=["body", "consensus_nt"]
    ).to_pylist()
    nt = {r["body"]: r["consensus_nt"] for r in nt}
    signs = np.array(
        [-1 if nt.get(int(i)) in ("gaba", "glutamate") else 1 for i in ids], np.float32
    )
    print("Retained neurons", n, flush=True)
    pre_all = []
    post_all = []
    weights = []
    with pa.memory_map(str(ROOT / "data/raw/connections.feather"), "r") as source:
        reader = ipc.open_file(source)
        for bi in range(reader.num_record_batches):
            b = reader.get_batch(bi)
            pre = b.column("body_pre").to_numpy()
            post = b.column("body_post").to_numpy()
            weight = b.column("weight").to_numpy()
            pi = np.searchsorted(ids, pre)
            qi = np.searchsorted(ids, post)
            valid = (pi < n) & (qi < n)
            pi = np.minimum(pi, n - 1)
            qi = np.minimum(qi, n - 1)
            valid &= (ids[pi] == pre) & (ids[qi] == post)
            pre_all.append(pi[valid].astype(np.int32))
            post_all.append(qi[valid].astype(np.int32))
            weights.append(weight[valid].astype(np.float32))
            if bi % 500 == 0:
                print("Edge batches", bi, "/", reader.num_record_batches, flush=True)
    pre = np.concatenate(pre_all)
    post = np.concatenate(post_all)
    values = np.concatenate(weights)
    del pre_all, post_all, weights
    totals = np.bincount(post, weights=values, minlength=n).astype(np.float32)
    values = values / np.maximum(totals[post], 1) * signs[pre]
    w = sparse.coo_matrix((values, (post, pre)), shape=(n, n)).tocsr()
    w.sum_duplicates()
    w.sort_indices()
    del pre, post, values
    inputs = []
    samples = []
    left = []
    right = []
    sidecounts = Counter()
    groups = []
    for i, r in enumerate(kept):
        side = r["rootSide"] or r["somaSide"]
        typ = r["type"] or ""
        if (
            r["superclass"] == "ol_sensory"
            and typ.startswith("R")
            and side in ("L", "R")
        ):
            rank = sidecounts[side]
            sidecounts[side] += 1
            inputs.append(i)
            # No column coordinates are supplied for these receptors. Explicit stable
            # stratified fallback within the corresponding image half; no anatomical claim.
            x = (rank * 37) % 64 + (64 if side == "R" else 0)
            y = (rank * 53 + rank // 64 * 7) % 96
            samples.append([x, y])
        if r["superclass"] == "visual_projection":
            if r["somaSide"] == "L":
                left.append(i)
            if r["somaSide"] == "R":
                right.append(i)
        groups.append(r["superclass"])
    inputs = np.array(inputs, np.int32)
    samples = np.array(samples, np.int32)
    left = np.array(left, np.int32)
    right = np.array(right, np.int32)
    if not len(inputs) or not len(left) or not len(right):
        raise ValueError("Missing required populations")
    # Retain bilateral visual samples for inspection; motor evidence uses relays below.
    telemetry = np.concatenate(
        [
            left[np.linspace(0, len(left) - 1, 64, dtype=int)],
            right[np.linspace(0, len(right) - 1, 64, dtype=int)],
        ]
    )
    readout = derive_readout(w, inputs)
    calibration = Brain(w, inputs, samples, left, right, readout=readout)
    neutral = np.full((96, 128, 3), 160, np.uint8)
    rates = []
    for _ in range(10):
        calibration.step_image(neutral)
        rates.append(calibration.population_rates())
    baseline = np.mean(rates[-5:], axis=0).tolist()
    del calibration
    sparse.save_npz(out / "weights.npz", w)
    np.savez_compressed(
        out / "mapping.npz",
        ids=ids,
        inputs=inputs,
        samples=samples,
        left=left,
        right=right,
        telemetry=telemetry,
        **readout,
    )
    provenance = {
        "dataset": "male-cns:v1.0",
        "retrieved": "2026-09-12",
        "license": "CC-BY; see https://male-cns.janelia.org/download/",
        "selection": "status == Traced AND nonempty superclass",
        "excluded_status_counts": dict(
            Counter(
                r["status"] or "missing"
                for r in rows
                if (r["status"] != "Traced" or not r["superclass"])
            )
        ),
        "source_files": {
            k: {"url": SOURCE + v, "sha256": digest(ROOT / "data/raw" / k)}
            for k, v in FILES.items()
        },
        "nodes": n,
        "edges": w.nnz,
        "orientation": "W[post,pre]",
        "ordering": "ascending bodyId",
        "sign_rule": "GABA and glutamate negative; all others positive, a modeling approximation",
        "unknown_transmitter_count": sum(nt.get(int(i)) is None for i in ids),
    }
    (ROOT / "data/source.lock.json").write_text(json.dumps(provenance, indent=2))
    manifest = {
        "schema": 2,
        "mode": "MaleCNS fixed controller",
        "nodes": n,
        "edges": w.nnz,
        "sensor": SENSOR_PROFILE,
        "color_rule": "Unpainted surfaces are grayscale. Yellow ink attracts; red ink repels. These are engineered game valences, not biological color preferences.",
        "input_population": "Traced ol_sensory, R-prefixed type, rootSide or somaSide L/R",
        "readout_population": "Non-input relay neurons; positive weight from channel > 0.05, channel share of positive retinal input > 0.9",
        "input_counts": dict(sidecounts),
        "mapping_fallback_count": len(inputs),
        "mapping_rule": "bodyId-sorted stratified samples in corresponding half; unresolved anatomical coordinates",
        "current": {
            "yellow": "clip((min(R,G)-B-24)/160,0,1)",
            "red": "clip((R-max(G,B)-48)/160,0,1)",
            "channel_assignment": "Even input rank yellow, odd red; deterministic artificial partition, not anatomical spectral assignments",
        },
        "readout": {
            "approach_count": len(readout["approach_indices"]),
            "avoid_count": len(readout["avoid_indices"]),
            "neutral_baseline": baseline,
            "scale": READOUT_SCALE,
            "calibration": "Seed 7; 10 neutral observations; mean of final 5 population rates; calibration state discarded before play",
        },
        "motor": asdict(MotorConfig()),
        "dynamics": {
            "kind": "nonnegative leaky rate approximation",
            "dt_ms": 20,
            "retention": 0.7,
            "recurrence_gain": 0.9,
            "noise": 0.0005,
        },
        "learning": False,
        "files": {
            p.name: digest(p) for p in (out / "weights.npz", out / "mapping.npz")
        },
        "source_lock_sha256": digest(ROOT / "data/source.lock.json"),
        "prepare_seconds": time.perf_counter() - start,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest, indent=2), flush=True)


if __name__ == "__main__":
    main()
