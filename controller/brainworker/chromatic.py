"""Engineered chromatic channels over existing MaleCNS sensory and relay cells."""

import numpy as np

SENSOR_PROFILE = "eye-level-v2 / ChromaticValenceV1"
READOUT_SCALE = 0.025


def derive_readout(w, inputs):
    """Select by sparse connectivity, without assigning biological color valence."""
    yellow_mask = np.arange(len(inputs)) % 2 == 0
    yellow = np.asarray(w[:, inputs[yellow_mask]].maximum(0).sum(axis=1)).ravel()
    red = np.asarray(w[:, inputs[~yellow_mask]].maximum(0).sum(axis=1)).ravel()
    relay = np.ones(w.shape[0], dtype=bool)
    relay[inputs] = False
    total = yellow + red + 1e-9
    return {
        "yellow_mask": yellow_mask,
        "approach_indices": np.flatnonzero(
            relay & (yellow > 0.05) & (yellow / total > 0.9)
        ),
        "avoid_indices": np.flatnonzero(relay & (red > 0.05) & (red / total > 0.9)),
    }


def encode(rgb, yellow_mask):
    """The renderer supplies shaded RGB8 samples; gray has zero chromatic drive."""
    rgb = rgb.astype(np.float32)
    yellow = np.clip((np.minimum(rgb[:, 0], rgb[:, 1]) - rgb[:, 2] - 24) / 160, 0, 1)
    red = np.clip((rgb[:, 0] - np.maximum(rgb[:, 1], rgb[:, 2]) - 48) / 160, 0, 1)
    return np.where(yellow_mask, yellow, red).astype(np.float32)
