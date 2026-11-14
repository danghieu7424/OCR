#!/usr/bin/env python3
"""Compare OCR subtitles with reference WebVTT cues.

Căn câu theo thời gian: ưu tiên overlap >= 40% thời lượng câu ngắn hơn, nếu không
có thì nhận câu lệch start <= 2s. So ký tự bằng Levenshtein.

Chuẩn hoá biến thể chữ (tuỳ chọn) để điểm khớp chỉ phản ánh chất lượng OCR:
    --norm t2s   phồn thể -> giản thể (zhconv, cần: pip install zhconv)
    --norm s2t   giản thể -> phồn thể
    --norm none  so nguyên văn (mặc định)

Conditional Character Accuracy chỉ tính các cặp cue đã căn.
Overall CER cũng tính toàn bộ ký tự trong cue thiếu và cue thừa.

Chạy: python tools/compare_subs.py
"""
import argparse
import glob
import os
import sys

MIN_OVERLAP_RATIO = 0.40
MAX_START_DIFF = 2.0


def parse_vtt(path):
    segments = []
    start = end = None
    with open(path, encoding="utf-8") as vtt_file:
        for raw in vtt_file:
            line = raw.strip()
            if "-->" in line:
                left, right = line.split("-->")
                start = to_sec(left.strip().split(" ")[0])
                end = to_sec(right.strip().split(" ")[0])
            elif line and line != "WEBVTT" and start is not None:
                segments.append((start, end, line))
                start = end = None
    return segments


def to_sec(ts):
    h, m, rest = ts.split(":")
    s, ms = rest.split(".") if "." in rest else (rest, "0")
    return int(h) * 3600 + int(m) * 60 + int(s) + int(ms.ljust(3, "0")[:3]) / 1000.0


def levenshtein(a, b):
    if not a:
        return len(b)
    if not b:
        return len(a)
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def align(ref, ours):
    """Ghép câu theo overlap giảm dần toàn cục (tránh dây chuyền lệch khi có
    câu ngắn bị mất), sau đó bù bằng start gần nhất."""
    cands = []
    for i, r in enumerate(ref):
        for j, o in enumerate(ours):
            overlap = min(r[1], o[1]) - max(r[0], o[0])
            if overlap <= 0:
                continue
            ratio = overlap / max(1e-6, min(r[1] - r[0], o[1] - o[0]))
            if ratio >= MIN_OVERLAP_RATIO:
                cands.append((ratio, i, j))
    cands.sort(reverse=True)

    used_ref, used_ours, pairs = set(), set(), []
    for _, i, j in cands:
        if i in used_ref or j in used_ours:
            continue
        used_ref.add(i)
        used_ours.add(j)
        pairs.append((ref[i], ours[j]))

    for i, r in enumerate(ref):
        if i in used_ref:
            continue
        best = None
        for j, o in enumerate(ours):
            if j in used_ours:
                continue
            d = abs(o[0] - r[0])
            if d <= MAX_START_DIFF and (best is None or d < best[1]):
                best = (j, d)
        if best is not None:
            used_ref.add(i)
            used_ours.add(best[0])
            pairs.append((ref[i], ours[best[0]]))

    pairs.sort(key=lambda p: p[0][0])
    missing = [r for i, r in enumerate(ref) if i not in used_ref]
    extra = [o for j, o in enumerate(ours) if j not in used_ours]
    return pairs, missing, extra


def make_normalizer(mode):
    if mode == "none":
        return lambda s: s
    try:
        import zhconv  # pip install zhconv
    except ImportError as error:
        raise SystemExit(
            "Chuẩn hóa t2s/s2t cần gói zhconv; chạy mặc định --norm none không cần gói này."
        ) from error
    locale = {"t2s": "zh-cn", "s2t": "zh-tw"}[mode]
    cache = {}

    def norm(text):
        if text not in cache:
            cache[text] = zhconv.convert(text, locale)
        return cache[text]
    return norm


def output_path_for_video(video_path, output_dir):
    """Prefer current raw `.subtitles.zh.vtt`; accept legacy `.zh.vtt`."""
    stem = os.path.splitext(os.path.basename(video_path))[0]
    candidates = [
        os.path.join(output_dir, f"{stem}.subtitles.zh.vtt"),
        os.path.join(output_dir, f"{stem}.zh.vtt"),
    ]
    for path in candidates:
        if os.path.isfile(path):
            return path
    raise FileNotFoundError(
        f"Không tìm thấy VTT OCR cho {stem}; đã thử: {', '.join(candidates)}"
    )


def score_alignment(ref, ours, normalizer):
    pairs, missing, extra = align(ref, ours)
    paired_ref_chars = 0
    paired_errors = 0
    exact = 0
    variant = 0
    for reference, recognized in pairs:
        ref_text = normalizer(reference[2])
        ocr_text = normalizer(recognized[2])
        paired_ref_chars += len(ref_text)
        if ref_text == ocr_text:
            exact += 1
            if reference[2] != recognized[2]:
                variant += 1
        else:
            paired_errors += levenshtein(ref_text, ocr_text)

    missing_chars = sum(len(normalizer(cue[2])) for cue in missing)
    extra_chars = sum(len(normalizer(cue[2])) for cue in extra)
    total_ref_chars = sum(len(normalizer(cue[2])) for cue in ref)
    overall_errors = paired_errors + missing_chars + extra_chars
    conditional_accuracy = (
        1.0 - paired_errors / paired_ref_chars if paired_ref_chars else None
    )
    overall_cer = overall_errors / total_ref_chars if total_ref_chars else None
    return {
        "pairs": pairs,
        "missing": missing,
        "extra": extra,
        "exact": exact,
        "variant": variant,
        "paired_ref_chars": paired_ref_chars,
        "paired_errors": paired_errors,
        "missing_chars": missing_chars,
        "extra_chars": extra_chars,
        "total_ref_chars": total_ref_chars,
        "overall_errors": overall_errors,
        "conditional_accuracy": conditional_accuracy,
        "overall_cer": overall_cer,
    }


def configure_stdout(stream=None):
    stream = sys.stdout if stream is None else stream
    reconfigure = getattr(stream, "reconfigure", None)
    if reconfigure:
        reconfigure(encoding="utf-8")


def main():
    configure_stdout()

    ap = argparse.ArgumentParser()
    ap.add_argument("--norm", default="none", choices=["none", "t2s", "s2t"])
    ap.add_argument("--input", default="input")
    ap.add_argument("--output", default="output")
    ap.add_argument("--show", default="", help="in chi tiết các câu lệch của tập này")
    args = ap.parse_args()
    norm = make_normalizer(args.norm)

    total = dict(
        ref_cues=0,
        ocr_cues=0,
        paired=0,
        exact=0,
        variant=0,
        missing=0,
        extra=0,
        paired_ref_chars=0,
        paired_errors=0,
        missing_chars=0,
        extra_chars=0,
        total_ref_chars=0,
        overall_errors=0,
    )
    print(
        "| Tập | ref | OCR | cặp | cue đúng | thiếu | thừa | "
        "Conditional Character Accuracy | Overall CER |"
    )
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for video in sorted(glob.glob(os.path.join(args.input, "*.mp4"))):
        stem = os.path.splitext(os.path.basename(video))[0]
        episode = stem[1:4] if len(stem) >= 4 else stem
        ref_path = os.path.join(args.input, f"{stem}.zh.vtt")
        ref = parse_vtt(ref_path)
        output_path = output_path_for_video(video, args.output)
        ours = parse_vtt(output_path)
        score = score_alignment(ref, ours, norm)
        conditional = score["conditional_accuracy"]
        conditional_text = "n/a" if conditional is None else f"{100 * conditional:.2f}%"
        overall_cer = score["overall_cer"]
        overall_cer_text = "n/a" if overall_cer is None else f"{100 * overall_cer:.2f}%"
        print(
            f"| {stem} | {len(ref)} | {len(ours)} | {len(score['pairs'])} | "
            f"{score['exact']} | {len(score['missing'])} | {len(score['extra'])} | "
            f"{conditional_text} | {overall_cer_text} |"
        )
        if args.show in (episode, stem):
            for r, o in score["pairs"]:
                if norm(r[2]) != norm(o[2]):
                    print(f"   SAI  ref='{r[2]}' ours='{o[2]}'")
            for m in score["missing"]:
                print(f"   THIEU {m[0]:.2f}-{m[1]:.2f} '{m[2]}'")
            for x in score["extra"]:
                print(f"   THUA  {x[0]:.2f}-{x[1]:.2f} '{x[2]}'")
        total["ref_cues"] += len(ref)
        total["ocr_cues"] += len(ours)
        total["paired"] += len(score["pairs"])
        total["exact"] += score["exact"]
        total["variant"] += score["variant"]
        total["missing"] += len(score["missing"])
        total["extra"] += len(score["extra"])
        for key in (
            "paired_ref_chars",
            "paired_errors",
            "missing_chars",
            "extra_chars",
            "total_ref_chars",
            "overall_errors",
        ):
            total[key] += score[key]

    conditional = (
        1.0 - total["paired_errors"] / total["paired_ref_chars"]
        if total["paired_ref_chars"]
        else None
    )
    print(f"\nNormalization: {args.norm}")
    print(
        "Conditional Character Accuracy (paired cues): "
        + ("n/a" if conditional is None else f"{100 * conditional:.2f}%")
        + f" ({total['paired_errors']}/{total['paired_ref_chars']} paired reference characters)"
    )
    overall_cer = (
        total["overall_errors"] / total["total_ref_chars"]
        if total["total_ref_chars"]
        else None
    )
    overall_cer_text = "n/a" if overall_cer is None else f"{100 * overall_cer:.2f}%"
    print(
        f"Overall CER: {overall_cer_text} "
        f"({total['overall_errors']}/{total['total_ref_chars']} reference characters; "
        f"paired edits={total['paired_errors']}, missing cue chars={total['missing_chars']}, "
        f"extra cue chars={total['extra_chars']})"
    )
    print(
        f"Cues: reference={total['ref_cues']}, OCR={total['ocr_cues']}, paired={total['paired']}, "
        f"exact={total['exact']} (orthographic variants={total['variant']}), "
        f"missing={total['missing']}, extra={total['extra']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
