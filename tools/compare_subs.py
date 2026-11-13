#!/usr/bin/env python3
"""So sánh phụ đề OCR (`output/第XXX集.zh.vtt`) với reference (`input/第XXX集.zh.vtt`).

Căn câu theo thời gian: ưu tiên overlap >= 40% thời lượng câu ngắn hơn, nếu không
có thì nhận câu lệch start <= 2s. So ký tự bằng Levenshtein.

Chuẩn hoá biến thể chữ (tuỳ chọn) để điểm khớp chỉ phản ánh chất lượng OCR:
    --norm t2s   phồn thể -> giản thể (zhconv, cần: pip install zhconv)
    --norm s2t   giản thể -> phồn thể
    --norm none  so nguyên văn (mặc định)

Chạy: PYTHONIOENCODING=utf-8 python tools/compare_subs.py --norm t2s
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
    for raw in open(path, encoding="utf-8"):
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
    import zhconv  # pip install zhconv
    locale = {"t2s": "zh-cn", "s2t": "zh-tw"}[mode]
    cache = {}

    def norm(text):
        if text not in cache:
            cache[text] = zhconv.convert(text, locale)
        return cache[text]
    return norm


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--norm", default="none", choices=["none", "t2s", "s2t"])
    ap.add_argument("--input", default="input")
    ap.add_argument("--output", default="output")
    ap.add_argument("--show", default="", help="in chi tiết các câu lệch của tập này")
    args = ap.parse_args()
    norm = make_normalizer(args.norm)

    total = dict(ref=0, ours=0, match=0, exact=0, variant=0, miss=0, extra=0, err=0, chars=0)
    print(f"| Tập | ref | ours | khớp | đúng | lệch | thiếu | thừa | acc% |")
    print("|---|---|---|---|---|---|---|---|---|")
    for video in sorted(glob.glob(os.path.join(args.input, "*.mp4"))):
        ep = os.path.basename(video)[1:4]
        ref = parse_vtt(os.path.join(args.input, f"第{ep}集.zh.vtt"))
        ours = parse_vtt(os.path.join(args.output, f"第{ep}集.zh.vtt"))
        pairs, missing, extra = align(ref, ours)

        exact = variant = err = chars = 0
        detail = []
        for r, o in pairs:
            rn, on = norm(r[2]), norm(o[2])
            chars += len(rn)
            if rn == on:
                exact += 1
                if r[2] != o[2]:
                    variant += 1
            else:
                err += levenshtein(rn, on)
                detail.append(f"   SAI  ref='{r[2]}' ours='{o[2]}'")
        acc = 100.0 * (1 - err / max(chars, 1))

        total["ref"] += len(ref)
        total["ours"] += len(ours)
        total["match"] += len(pairs)
        total["exact"] += exact
        total["variant"] += variant
        total["miss"] += len(missing)
        total["extra"] += len(extra)
        total["err"] += err
        total["chars"] += chars

        print(f"| {ep} | {len(ref)} | {len(ours)} | {len(pairs)} | {exact} | "
              f"{len(pairs) - exact} | {len(missing)} | {len(extra)} | {acc:.2f} |")
        if args.show == ep:
            print("\n".join(detail))
            for m in missing:
                print(f"   THIEU {m[0]:.2f}-{m[1]:.2f} '{m[2]}'")
            for x in extra:
                print(f"   THUA  {x[0]:.2f}-{x[1]:.2f} '{x[2]}'")

    acc_all = 100.0 * (1 - total["err"] / max(total["chars"], 1))
    print(f"\n**Tổng** (norm={args.norm}): {total['ref']} câu ref, {total['ours']} câu OCR, "
          f"khớp {total['match']}, đúng {total['exact']} "
          f"(trong đó {total['variant']} chỉ khác biến thể chữ), "
          f"thiếu {total['miss']}, thừa {total['extra']}, "
          f"lỗi {total['err']}/{total['chars']} ký tự → **{acc_all:.2f}%**")
    return 0


if __name__ == "__main__":
    sys.exit(main())
