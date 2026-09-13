"""Export the validated legacy graph for the 25 Hz Rust worker; never recalibrate it."""
import hashlib
import json
from pathlib import Path

import numpy as np
from .model import Brain

ROOT = Path(__file__).resolve().parents[2]
BACKEND = "candle-metal-csr-f32-v1"
RNG = "numpy-pcg64-normal-f64-v1"


def main():
    source = ROOT / "data/cache/malecns-v1"
    out = ROOT / "data/cache/malecns-rust-v1"
    source_hash = hashlib.sha256((source / "manifest.json").read_bytes()).hexdigest()
    export_hash = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    atlas_hash = hashlib.sha256((ROOT / "assets/brain_atlas.json").read_bytes()).hexdigest()
    identity = dict(schema=1, backend=BACKEND, rng=RNG, source_profile_sha256=source_hash,
                    exporter_sha256=export_hash, atlas_sha256=atlas_hash,
                    sensor="eye-level-v2 / ChromaticValenceV1", neural_steps=2,
                    physics_ticks=4, decision_ms=40)
    manifest_path = out / "manifest.json"
    if manifest_path.exists():
        old = json.loads(manifest_path.read_text())
        if all(old.get(k) == v for k,v in identity.items()) and all(
            (out/name).is_file() and hashlib.sha256((out/name).read_bytes()).hexdigest() == digest
            for name,digest in old.get("files", {}).items()
        ) and set(old.get("files",{})) == {"values.f32", "columns.u32", "rows.u32", "mapping.json"}:
            print("Rust 25 Hz graph export is current.")
            return
    brain = Brain.load(source)
    out.mkdir(parents=True, exist_ok=True)
    mapping = np.load(source / "mapping.npz", allow_pickle=False)
    arrays = {k: mapping[k].tolist() for k in mapping.files}
    mapping.close()
    rng = brain.rng.bit_generator.state
    rng["state"] = {k: str(v) for k,v in rng["state"].items()}
    motor = dict(brain.profile["motor"], retreat_decisions=20, search_decisions=88,
                 search_commit_decisions=10, clear_decisions=8)
    blobs = {"values.f32":brain.w.data.astype("<f4").tobytes(),
             "columns.u32":brain.w.indices.astype("<u4").tobytes(),
             "rows.u32":brain.w.indptr.astype("<u4").tobytes(),
             "mapping.json":json.dumps(arrays,separators=(",",":")).encode()}
    for name,blob in blobs.items():
        temp = out / (name + ".tmp")
        temp.write_bytes(blob); temp.replace(out / name)
    manifest = dict(**identity, nodes=brain.w.shape[0], edges=brain.w.nnz,
                    readout=brain.profile["readout"], motor=motor,
                    dynamics=brain.profile["dynamics"], initial_rng=rng,
                    source_lock_sha256=brain.profile["source_lock_sha256"],
                    files={name:hashlib.sha256(blob).hexdigest() for name,blob in blobs.items()})
    temp = manifest_path.with_suffix(".tmp")
    temp.write_text(json.dumps(manifest, indent=2)); temp.replace(manifest_path)
    print(f"Exported {manifest['nodes']} neurons, {manifest['edges']} edges; 25 Hz, identical NumPy noise stream.")


if __name__ == "__main__": main()
