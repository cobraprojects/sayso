# /// script
# requires-python = ">=3.11"
# dependencies = ["pyarrow", "soundfile", "numpy", "soxr"]
# ///
"""Make the benchmark sample: 16 kHz mono WAV clips and a manifest.

The clips come from the Open ASR Leaderboard test sets on Hugging Face
(hf-audio/open-asr-leaderboard). A set is sorted by clip length and split into
files, so the sample takes clips from the first, middle, and last file of each
set. The seed is fixed, so the same command always gives the same sample.

    uv run scripts/benchmark/prepare.py [--out ~/.cache/sayso-bench] [--per-set 200]
"""
import argparse
import io
import json
import random
import urllib.request
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq
import soundfile as sf
import soxr

BASE = "https://huggingface.co/datasets/hf-audio/open-asr-leaderboard/resolve/main"
# Set name -> the files of the set. Seven English sets: TED-LIUM is not in this dataset.
SETS = {
    "ami": [f"ami/test-{i:05d}-of-00015.parquet" for i in range(15)],
    "earnings22": [f"earnings22/test-{i:05d}-of-00005.parquet" for i in range(5)],
    "gigaspeech": [f"gigaspeech/test-{i:05d}-of-00019.parquet" for i in range(19)],
    "librispeech_clean": ["librispeech/test.clean-00000-of-00001.parquet"],
    "librispeech_other": ["librispeech/test.other-00000-of-00001.parquet"],
    "spgispeech": [f"spgispeech/test-{i:05d}-of-00038.parquet" for i in range(38)],
    "voxpopuli": [f"voxpopuli/test-{i:05d}-of-00004.parquet" for i in range(4)],
}
SEED = 20261015


def spread(files):
    """The first, middle, and last file of a set (fewer for a short set)."""
    picks = [files[0], files[len(files) // 2], files[-1]]
    return list(dict.fromkeys(picks))


def fetch(rel, raw):
    path = raw / rel
    if not path.exists():
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(".part")
        print(f"  download {rel}", flush=True)
        urllib.request.urlretrieve(f"{BASE}/{rel}", tmp)
        tmp.rename(path)
    return path


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path.home() / ".cache/sayso-bench"))
    ap.add_argument("--per-set", type=int, default=200)
    args = ap.parse_args()
    out = Path(args.out).expanduser()
    raw, clips = out / "raw", out / "clips"
    clips.mkdir(parents=True, exist_ok=True)
    rng = random.Random(SEED)
    manifest = []
    for name, files in SETS.items():
        chosen = spread(files)
        per_file = [args.per_set // len(chosen)] * len(chosen)
        per_file[0] += args.per_set - sum(per_file)
        for rel, count in zip(chosen, per_file):
            table = pq.read_table(fetch(rel, raw), columns=["audio", "text", "id", "audio_length_s"])
            # Clips with an empty reference do not count in WER.
            rows = [i for i, t in enumerate(table.column("text").to_pylist()) if t and t.strip()]
            for i in sorted(rng.sample(rows, min(count, len(rows)))):
                row = table.slice(i, 1).to_pylist()[0]
                audio, rate = sf.read(io.BytesIO(row["audio"]["bytes"]), dtype="float32", always_2d=True)
                audio = audio.mean(axis=1)
                if rate != 16000:
                    audio = soxr.resample(audio, rate, 16000)
                # FluidAudio rejects audio under 1 s. Silence after the speech does not
                # change the words, so short clips get it.
                if len(audio) < 16000:
                    audio = np.pad(audio, (0, 16000 - len(audio)))
                clip_id = f"{name}-{row['id']}".replace("/", "_")
                wav = clips / f"{clip_id}.wav"
                sf.write(wav, np.clip(audio, -1, 1), 16000, subtype="PCM_16")
                manifest.append({"id": clip_id, "set": name, "path": str(wav), "text": row["text"], "seconds": len(audio) / 16000})
        print(f"{name}: {sum(1 for m in manifest if m['set'] == name)} clips", flush=True)
    with open(out / "manifest.jsonl", "w") as f:
        for m in manifest:
            f.write(json.dumps(m) + "\n")
    hours = sum(m["seconds"] for m in manifest) / 3600
    print(f"{len(manifest)} clips, {hours:.2f} h of audio -> {out / 'manifest.jsonl'}")


if __name__ == "__main__":
    main()
