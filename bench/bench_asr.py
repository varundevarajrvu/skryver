"""M0 benchmark: RTF + latency of candidate local ASR models on this machine.

Measures, per model and thread count:
  - cold-start (model load) time
  - RTF on a ~60s passage (processing_time / audio_time; lower is better, <1 = real-time)
  - end-to-end latency on a ~5s utterance (the hold-to-talk release -> text delay users feel)
Peak RSS is sampled after each model run via psutil if available.

Usage:  python bench_asr.py [--threads 4,8] [--runs 3]
Results printed as a markdown table and written to results.json.
"""

import argparse
import gc
import json
import sys
import time
from pathlib import Path

import numpy as np
import soundfile as sf
import sherpa_onnx

HERE = Path(__file__).parent
MODELS = HERE / "models"


def load_wav(path):
    samples, sr = sf.read(path, dtype="float32", always_2d=False)
    assert sr == 16000, f"{path}: expected 16kHz, got {sr}"
    return samples, sr


def find(dirname, pattern):
    d = MODELS / dirname
    hits = sorted(d.glob(pattern))
    if not hits:
        raise FileNotFoundError(f"{d}\\{pattern}")
    return str(hits[0])


def make_recognizer(name, threads):
    if name == "parakeet-tdt-0.6b-v2-int8":
        d = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8"
        return sherpa_onnx.OfflineRecognizer.from_transducer(
            encoder=find(d, "encoder*.onnx"),
            decoder=find(d, "decoder*.onnx"),
            joiner=find(d, "joiner*.onnx"),
            tokens=find(d, "tokens.txt"),
            num_threads=threads,
            model_type="nemo_transducer",
        )
    if name in ("whisper-base.en", "whisper-base.en-int8", "whisper-small.en", "whisper-small.en-int8"):
        size = "base" if "base" in name else "small"
        d = f"sherpa-onnx-whisper-{size}.en"
        enc_pat = f"{size}.en-encoder.int8.onnx" if "int8" in name else f"{size}.en-encoder.onnx"
        dec_pat = f"{size}.en-decoder.int8.onnx" if "int8" in name else f"{size}.en-decoder.onnx"
        return sherpa_onnx.OfflineRecognizer.from_whisper(
            encoder=find(d, enc_pat),
            decoder=find(d, dec_pat),
            tokens=find(d, f"{size}.en-tokens.txt"),
            num_threads=threads,
            language="en",
            task="transcribe",
        )
    if name == "moonshine-base-en-int8":
        d = "sherpa-onnx-moonshine-base-en-int8"
        return sherpa_onnx.OfflineRecognizer.from_moonshine(
            preprocessor=find(d, "preprocess*.onnx"),
            encoder=find(d, "encode*.onnx"),
            uncached_decoder=find(d, "uncached_decode*.onnx"),
            cached_decoder=find(d, "cached_decode*.onnx"),
            tokens=find(d, "tokens.txt"),
            num_threads=threads,
        )
    raise ValueError(name)


def transcribe(rec, samples, sr):
    t0 = time.perf_counter()
    s = rec.create_stream()
    s.accept_waveform(sr, samples)
    rec.decode_stream(s)
    return time.perf_counter() - t0, s.result.text.strip()


def rss_mb():
    try:
        import psutil
        return psutil.Process().memory_info().rss / 1e6
    except ImportError:
        return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--threads", default="4,8")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--models", default="parakeet-tdt-0.6b-v2-int8,whisper-base.en-int8,whisper-small.en-int8,moonshine-base-en-int8")
    args = ap.parse_args()

    long_wav, sr = load_wav(HERE / "test_long.wav")
    short_wav, _ = load_wav(HERE / "test_short.wav")
    long_dur = len(long_wav) / sr
    short_dur = len(short_wav) / sr
    print(f"audio: long={long_dur:.1f}s short={short_dur:.1f}s\n", file=sys.stderr)

    results = []
    for name in args.models.split(","):
        for threads in [int(t) for t in args.threads.split(",")]:
            gc.collect()
            base_rss = rss_mb()
            t0 = time.perf_counter()
            try:
                rec = make_recognizer(name, threads)
            except Exception as e:
                print(f"SKIP {name} t={threads}: {e}", file=sys.stderr)
                continue
            load_s = time.perf_counter() - t0

            # warmup on short clip, then timed runs
            transcribe(rec, short_wav, sr)
            long_times, short_times, text_long, text_short = [], [], "", ""
            for _ in range(args.runs):
                dt, text_long = transcribe(rec, long_wav, sr)
                long_times.append(dt)
                dt, text_short = transcribe(rec, short_wav, sr)
                short_times.append(dt)

            peak_rss = rss_mb()
            row = {
                "model": name,
                "threads": threads,
                "load_s": round(load_s, 2),
                "rtf_long": round(min(long_times) / long_dur, 3),
                "latency_short_ms": round(min(short_times) * 1000),
                "rss_delta_mb": round(peak_rss - base_rss) if peak_rss and base_rss else None,
                "text_long": text_long,
                "text_short": text_short,
            }
            results.append(row)
            print(f"{name} t={threads}: load={row['load_s']}s RTF={row['rtf_long']} "
                  f"short={row['latency_short_ms']}ms dRSS={row['rss_delta_mb']}MB", file=sys.stderr)
            del rec

    (HERE / "results.json").write_text(json.dumps(results, indent=2))

    print("\n| Model | Threads | Load (s) | RTF (60s clip) | Latency 5s utterance | RAM delta |")
    print("|---|---|---|---|---|---|")
    for r in results:
        print(f"| {r['model']} | {r['threads']} | {r['load_s']} | {r['rtf_long']} "
              f"| {r['latency_short_ms']} ms | {r['rss_delta_mb']} MB |")


if __name__ == "__main__":
    main()
