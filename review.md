Tôi sẽ đối chiếu bốn tài liệu với luồng xử lý/code hiện có, rồi kiểm tra một vài mẫu VTT đầu vào–đầu ra để đánh giá trên dữ liệu thực tế; không dùng OCR_LLM.

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\readme.md"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\plan.md"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\CHANGELOG.md"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md"
}

Read , lines 1 to 220
Completed with input: {
  "path": "C:\\DATA\\OCR\\examples\\extract_and_merge.rs",
  "view_range": [
    1,
    220
  ]
}

Ran terminal command: git status --short; Get-ChildItem input -File | Select-Object Name,Length; Get-ChildItem output -File | Select-Object Name,Length; Get-ChildItem final -File | Select-Object Name,Length

Search for `^#{1,4} |OCR_LLM|LLM|100%|99\.?|ROI|0\.85|0\.80|70%|10%|85%|không dùng|hard.s...`

Read , lines 1 to 220
Completed with input: {
  "path": "C:\\DATA\\OCR\\plan.md",
  "view_range": [
    1,
    220
  ]
}

Read , lines 1 to 180
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md",
  "view_range": [
    1,
    180
  ]
}

Read , lines 181 to 370
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md",
  "view_range": [
    181,
    370
  ]
}

Read , lines 1 to 180
Completed with input: {
  "path": "C:\\DATA\\OCR\\input\\第001集.zh.vtt",
  "view_range": [
    1,
    180
  ]
}

Read , lines 1 to 180
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第001集.zh.vtt",
  "view_range": [
    1,
    180
  ]
}

Read , lines 1 to 100
Completed with input: {
  "path": "C:\\DATA\\OCR\\final\\第001集.zh.vtt",
  "view_range": [
    1,
    100
  ]
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第001集.captions.zh.vtt"
}

Các tài liệu có mâu thuẫn đáng kể với yêu cầu hiện tại: [plan.md] còn đề xuất LLM, còn số liệu đánh giá ghi caption chỉ kiểm mắt 24/25 và chưa có reference cho phần lớn tập. Tôi đang rà phần kết luận mới nhất và các file hiện hành để tách rõ điều đã chứng minh với điều mới chỉ là giả thuyết.

Read , lines 371 to 560
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md",
  "view_range": [
    371,
    560
  ]
}

Read , lines 561 to 750
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md",
  "view_range": [
    561,
    750
  ]
}

Read , lines 751 to 980
Completed with input: {
  "path": "C:\\DATA\\OCR\\evaluate.md",
  "view_range": [
    751,
    980
  ]
}

Find files matching `crates/OCR_*/src/**/*.rs`

Search for `CaptionRoi|y_min|y_max|x_min|x_max|OcrExtractorConfig|accuracy_mode|fps|confi...`

Ran terminal command: Get-ChildItem scratch -Recurse -File -ErrorAction SilentlyContinue | Select-Object -First 60 FullName,Length; Get-Command ffmpeg -ErrorAction SilentlyContinue | Select-Object Source; Get-Command python -ErrorAction SilentlyContinue | Select-Object Source

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\final\\第002集.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第002集.captions.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\final\\第003集.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第003集.captions.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\final\\第004集.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第004集.captions.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_03_he_hong.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep04_01_shixianjian.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_03_zhidao.png"
}

Find files matching `tools/*`

Read , lines 1 to 160
Completed with input: {
  "path": "C:\\DATA\\OCR\\tools\\compare_subs.py",
  "view_range": [
    1,
    160
  ]
}

Read , lines 180 to 469
Completed with input: {
  "path": "C:\\DATA\\OCR\\examples\\extract_and_merge.rs",
  "view_range": [
    180,
    469
  ]
}

Read , lines 45 to 150
Completed with input: {
  "path": "C:\\DATA\\OCR\\crates\\OCR_Subtitles\\src\\lib.rs",
  "view_range": [
    45,
    150
  ]
}

Read , lines 15 to 135
Completed with input: {
  "path": "C:\\DATA\\OCR\\crates\\OCR_Captions\\src\\lib.rs",
  "view_range": [
    15,
    135
  ]
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第001集.captions.zh.vtt"
}

Read , lines 1 to 90
Completed with input: {
  "path": "C:\\DATA\\OCR\\final\\第001集.zh.vtt",
  "view_range": [
    1,
    90
  ]
}

Ran terminal command: python tools\compare_subs.py --norm none; python tools\compare_subs.py --norm t2s
Tool execution failed with exit code 1

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; python tools\compare_subs.py --norm none; git status --short

Trong dữ liệu hiện tại, script đối chiếu dừng ở tập 002 vì không tìm thấy `output/第002集.zh.vtt`; khác với bố cục được mô tả trong nhật ký. Tôi sẽ kiểm tra tên file đang có và chạy đối chiếu theo đúng các file hiện diện, không ghi đè hay tạo bản sao vào workspace.

Ran terminal command: Get-ChildItem output -File | Where-Object { $_.Name -match '第00[1-4]集' } | Select-Object Name,Length; Get-ChildItem input -Filter '*.mp4' | Select-Object -ExpandProperty Name

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第001集.captions.zh.vtt"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\output\\第004集.captions.zh.vtt"
}

Search for `def main|total\[|Total|Tổng|print\(`

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt, align, levenshtein
base='output'; total={'ref':0,'ours':0,'match':0,'exact':0,'miss':0,'extra':0,'err':0,'chars':0}
print('ep | source | ref | ocr | paired | exact | missing | extra | char-acc')
for video in sorted(glob.glob('input/*.mp4')):
    ep=os.path.basename(video)[1:4]
    ref=parse_vtt(f'input/第{ep}集.zh.vtt')
    p=f'{base}/第{ep}集.zh.vtt'
    if not os.path.exists(p): p=f'{base}/第{ep}集.subtitles.zh.vtt'
    if not os.path.exists(p):
        print(ep, 'NO OCR FILE'); continue
    ours=parse_vtt(p); pairs,missing,extra=align(ref,ours)
    err=chars=exact=0
    for r,o in pairs:
        chars+=len(r[2])
        if r[2]==o[2]: exact+=1
        else: err+=levenshtein(r[2],o[2])
    acc=100*(1-err/max(chars,1))
    print(f'{ep} | {os.path.basename(p).split("集")[-1]} | {len(ref)} | {len(ours)} | {len(pairs)} | {exact} | {len(missing)} | {len(extra)} | {acc:.2f}')
    for k,v in [('ref',len(ref)),('ours',len(ours)),('match',len(pairs)),('exact',exact),('miss',len(missing)),('extra',len(extra)),('err',err),('chars',chars)]: total[k]+=v
print('TOTAL', total, f"char-acc={100*(1-total['err']/max(total['chars'],1)):.2f}%")
'@ | python -
Tool execution failed with exit code 1

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt, align, levenshtein
base='output'; total={'ref':0,'ours':0,'match':0,'exact':0,'miss':0,'extra':0,'err':0,'chars':0}
print('ep | source | ref | ocr | paired | exact | missing | extra | char-acc')
for video in sorted(glob.glob('input/*.mp4')):
    stem=os.path.basename(video)[:-4]
    ep=stem[1:4]
    ref=parse_vtt(os.path.join('input',stem+'.zh.vtt'))
    p=os.path.join(base,stem+'.zh.vtt')
    if not os.path.exists(p): p=os.path.join(base,stem+'.subtitles.zh.vtt')
    if not os.path.exists(p):
        print(ep, 'NO OCR FILE'); continue
    ours=parse_vtt(p); pairs,missing,extra=align(ref,ours)
    err=chars=exact=0
    for r,o in pairs:
        chars+=len(r[2])
        if r[2]==o[2]: exact+=1
        else: err+=levenshtein(r[2],o[2])
    acc=100*(1-err/max(chars,1))
    print(f'{ep} | {os.path.basename(p)[len(stem):]} | {len(ref)} | {len(ours)} | {len(pairs)} | {exact} | {len(missing)} | {len(extra)} | {acc:.2f}')
    for k,v in [('ref',len(ref)),('ours',len(ours)),('match',len(pairs)),('exact',exact),('miss',len(missing)),('extra',len(extra)),('err',err),('chars',chars)]: total[k]+=v
print('TOTAL', total, f"char-acc={100*(1-total['err']/max(total['chars'],1)):.2f}%")
'@ | python -

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_01_luqingan.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_02_xitong.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_04_linqingxue.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_05_xuqingying.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_06_danver.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep01_07_chuanyin.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_01_xiao_nv_er.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_02_lao_er.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_04_didi.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_05_meimei.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep02_06_jia.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep03_01_jiyi.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep03_02_jiaojiechu.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep03_03_moyu.png"
}

Read 
Completed with input: {
  "path": "C:\\DATA\\OCR\\scratch\\caption_frames\\ep04_02_zhanzhuang.png"
}

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt
for ref_path in sorted(glob.glob('final/*.vtt')):
    stem=os.path.basename(ref_path)[:-4]
    cand=os.path.join('output',stem.replace('.zh','.captions.zh')+'.vtt')
    ref=parse_vtt(ref_path); out=parse_vtt(cand)
    print(f'[{stem}] ref={len(ref)} output={len(out)} exists={os.path.exists(cand)}')
    if len(ref)!=len(out): print('  COUNT MISMATCH')
    unmatched=[]
    for i,(r,o) in enumerate(zip(ref,out)):
        if r[2]!=o[2]: unmatched.append((i,r[2],o[2]))
        print(f'  {i+1}: same_text={r[2]==o[2]} start_delta={o[0]-r[0]:+.3f}s end_delta={o[1]-r[1]:+.3f}s')
    if unmatched: print('  TEXT MISMATCHES:',unmatched)
'@ | python -

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob, re
from tools.compare_subs import parse_vtt
for ref_path in sorted(glob.glob('final/*.vtt')):
    stem=os.path.basename(ref_path)[:-4]
    out_path=os.path.join('output',stem.replace('.zh','.captions.zh')+'.vtt')
    ref=[c for c in parse_vtt(ref_path) if '《' in c[2] and '》' in c[2]]
    out=parse_vtt(out_path)
    strip=lambda s: re.sub(r'[.…]+$', '', s)
    matched=0
    print(f'[{stem}] final_caption={len(ref)} OCR_caption={len(out)}')
    for i,(r,o) in enumerate(zip(ref,out)):
        same=strip(r[2])==strip(o[2]); matched+=same
        print(f'  {i+1}: same_after_ellipsis={same} start_delta={o[0]-r[0]:+.3f}s end_delta={o[1]-r[1]:+.3f}s')
        if not same: print(f'    final={r[2]!r} OCR={o[2]!r}')
    print(f'  matched={matched}/{len(ref)} (paired by order)')
'@ | python -

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt
open_l, close_l, ellipsis = chr(0x300a), chr(0x300b), chr(0x2026)
def is_caption(c): return open_l in c[2] and close_l in c[2]
def clean(s): return s.rstrip('.'+ellipsis)
for ref_path in sorted(glob.glob('final/*.vtt')):
    stem=os.path.basename(ref_path)[:-4]
    out_path=os.path.join('output',stem.replace('.zh','.captions.zh')+'.vtt')
    ref=[c for c in parse_vtt(ref_path) if is_caption(c)]
    out=parse_vtt(out_path)
    matched=0
    print(f'[{stem}] final_caption={len(ref)} OCR_caption={len(out)}')
    for i,(r,o) in enumerate(zip(ref,out)):
        same=clean(r[2])==clean(o[2]); matched+=same
        print(f'  {i+1}: same_after_ellipsis={same} start_delta={o[0]-r[0]:+.3f}s end_delta={o[1]-r[1]:+.3f}s')
        if not same: print(f'    final={r[2]!r} OCR={o[2]!r}')
    print(f'  matched={matched}/{len(ref)} (paired by order)')
'@ | python -

Search for `Industrial-Grade|>99%|100% Sub|config\.roi\.y_min|Qwen2\.5-VL|OCR_LLM|0\.85|2...`

Search for `is_valid_vtt|unwrap_or_default|Proofreader::with_fast_mode|for video in &vide...`

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt
open_l,close_l,ell=chr(0x300a),chr(0x300b),chr(0x2026)
rows=[]
for rp in sorted(glob.glob('final/*.vtt')):
    stem=os.path.basename(rp)[:-4]; op=os.path.join('output',stem.replace('.zh','.captions.zh')+'.vtt')
    refs=[c for c in parse_vtt(rp) if open_l in c[2] and close_l in c[2]]; outs=parse_vtt(op)
    for r,o in zip(refs,outs):
        a=r[2].replace('...','').replace(ell,''); b=o[2].replace('...','').replace(ell,'')
        rows.append((a==b,abs(o[0]-r[0]),abs(o[1]-r[1]),stem))
print('cues',len(rows),'text_match',sum(x[0] for x in rows),'max_abs_start_drift_s',max(x[1] for x in rows),'max_abs_end_drift_s',max(x[2] for x in rows))
print('start_drift_over_0_3s',sum(x[1]>.3 for x in rows),'end_drift_over_0_3s',sum(x[2]>.3 for x in rows))
for x in rows:
    if x[1]>.3 or x[2]>.3: print(x)
'@ | python -

Ran terminal command: $env:PYTHONIOENCODING = 'utf-8'; @'
import os, glob
from tools.compare_subs import parse_vtt, align, levenshtein
err=refchars=misschars=extrachars=0; missing=extra=0
for video in sorted(glob.glob('input/*.mp4')):
    stem=os.path.basename(video)[:-4]
    r=parse_vtt(os.path.join('input',stem+'.zh.vtt'))
    p=os.path.join('output',stem+'.zh.vtt')
    if not os.path.exists(p): p=os.path.join('output',stem+'.subtitles.zh.vtt')
    o=parse_vtt(p); pairs,ms,xs=align(r,o)
    for a,b in pairs:
        refchars+=len(a[2]); err+=levenshtein(a[2],b[2])
    misschars+=sum(len(x[2]) for x in ms); extrachars+=sum(len(x[2]) for x in xs)
    missing+=len(ms); extra+=len(xs)
print(f'paired_edit={err}, paired_ref_chars={refchars}, missing_cues={missing}, missing_chars={misschars}, extra_cues={extra}, extra_chars={extrachars}')
print(f'rough_CER_including_unpaired_cue_chars={(err+misschars+extrachars)/max(1,refchars+misschars)*100:.3f}%')
'@ | python -

## Đánh giá

Tôi đã đọc cả bốn tài liệu, đối chiếu VTT hiện có và xem các frame caption lưu trong `scratch`. **Không gọi OCR_LLM và không sửa file.** Tôi cũng không chạy lại `extract_and_merge.rs`: ví dụ này có thể ghi lại `refined/merged`, hiện shell không tìm thấy `ffmpeg`, và quan trọng hơn, code đang gọi `Proofreader::with_fast_mode()` từ crate OCR_LLM dù đã tắt LLM.

- **`readme.md` đang khẳng định quá mức:** các tuyên bố “>99%” và “100% sub một ký tự” không phải bảo đảm cho video bất kỳ; chúng dựa trên một bộ video tiếng Trung cụ thể. `readme.md`
- **`plan.md` không phù hợp yêu cầu hiện tại:** còn đề xuất Qwen/VLM, tự sửa chữ theo ngữ cảnh và hứa hẹn độ chính xác 100%. Cũng không nên tự động sửa phụ đề chỉ vì một từ gần giống trong glossary. `plan.md`
- **`CHANGELOG.md` và pipeline vẫn phụ thuộc OCR_LLM:** ví dụ hiện tại gọi `EntityGlossary`/`Proofreader` ngay cả ở Fast Mode. Nếu “không dùng OCR_LLM” là yêu cầu nghiêm ngặt, không nên chạy ví dụ này nguyên trạng. `CHANGELOG.md` · `extract_and_merge.rs`
- **`evaluate.md` trộn nhiều lần đánh giá:** có báo cáo 99.18%, báo cáo chuẩn hóa 99.82%, caption 24/25 và báo cáo 18/18 cho bốn tập đầu. Đây là các lần đo khác nhau, không phải một bảo đảm tổng quát. `evaluate.md`

### Kết quả kiểm tra dữ liệu hiện có

**Subtitle:** tôi đối chiếu 20 tập với `input`, dùng `.zh.vtt` khi có và `.subtitles.zh.vtt` cho tập 002 vì file `.zh.vtt` của tập này đang thiếu. Kết quả: 758 cue tham chiếu, 761 cue OCR; căn được 753 cặp, trong đó 709 cặp khớp nguyên văn; còn 5 cue thiếu, 8 cue thừa và 44 edit ký tự trên các cặp đã căn. Con số 99.20% là **độ khớp ký tự có điều kiện trên các cue đã căn**, không tính đầy đủ cue thiếu/thừa; tính thô cả chữ trong cue không ghép được thì lỗi khoảng 1.18%—không thay thế cho CER chuẩn vì còn phụ thuộc thuật toán căn cue.

Công cụ hiện có không chạy được nguyên trạng trên toàn bộ output: `tools/compare_subs.py` yêu cầu `output/第002集.zh.vtt`, trong khi hiện chỉ có `第002集.subtitles.zh.vtt`; chế độ `--norm t2s` cũng cần `zhconv` chưa được cài. Vì bạn xác nhận `input` là bản đã kiểm chuẩn, nên đánh giá vận hành nên lấy nó làm ground truth; không tự loại lỗi khỏi điểm số dựa trên nhận xét cũ trong `evaluate.md`. `compare_subs.py`

**Caption:** `final` là VTT đầy đủ có cả thoại lẫn caption, không chỉ có caption. Tôi lọc cue dạng `《...》` trong `final` rồi đối chiếu với `output/*.captions.zh.vtt`: đủ 18 cue; nội dung khớp 18/18 nếu bỏ qua ba dấu `...` được thêm trong bản bạn biên tập. Thời điểm chưa khớp hoàn toàn: lệch tối đa khoảng **0.60 giây ở đầu** và **0.40 giây ở cuối**. Trong các frame đã lưu, có một điểm cần xác minh lại: `ep01_06_danver.png` không thấy nameplate tương ứng với cue caption tập 1, dù VTT có cue đó. Tôi không kết luận OCR sai vì chưa xác nhận timestamp chính xác của ảnh; cần trích lại quanh thời điểm cue. Các frame khác nhìn thấy caption thư pháp rõ, kể cả chữ dọc và cảnh nền phức tạp. `final/第001集.zh.vtt` · `output/第001集.captions.zh.vtt`

Code hiện có đúng ROI caption bạn mô tả: **Y 10–80%, X 0–100%**. Nhưng các ngưỡng cố định như thời lượng tối thiểu 1 giây, confidence 0.80 và lọc watermark có thể bỏ caption ngắn hoặc nhầm chữ trong cảnh ở video khác. ROI rộng giúp bắt ứng viên, không tự phân biệt được nameplate với biển hiệu, watermark hay chữ cảnh. `OCR_Captions/src/lib.rs`

## Giải pháp tôi đề xuất — không dùng OCR_LLM

1. **Giữ kết quả OCR thô làm nguồn chính:** trích xuất subtitle và caption riêng, sau đó merge. Không tự sửa câu bằng LLM, glossary fuzzy-match hoặc luật từ vựng; nếu có gợi ý sửa thì xuất riêng để bạn duyệt, không ghi đè VTT thô.
2. **Thích nghi theo video:** đọc metadata, lấy frame mẫu để xác định vùng chữ và vị trí subtitle/caption; giữ ROI caption 10–80% × 0–100% như cấu hình ban đầu, nhưng dùng phát hiện box và bước duyệt ảnh để loại chữ trong cảnh. Subtitle không nên mặc định luôn ở đáy đối với mọi nguồn.
3. **Giữ bằng chứng thị giác:** với mỗi caption, lưu frame có timestamp và box/crop phóng lớn; tạo danh sách cần duyệt cho cue thiếu confidence, có nhiều cách đọc hoặc lệch thời gian. Vote nhiều frame nên căn chỉnh chuỗi có trọng số confidence, không bỏ phiếu ký tự theo vị trí đơn giản.
4. **Đánh giá riêng nội dung và thời gian:** với subtitle, báo CER có tính cue thiếu/thừa, exact-cue rate và sai số timestamp; với caption, báo precision/recall, exact text và sai số đầu/cuối, đồng thời duyệt ảnh cho ca nghi ngờ. Chuẩn hóa phồn/giản chỉ dùng khi trình bày thêm một điểm phụ, không sửa ground truth.
5. **Giới hạn cam kết theo model hỗ trợ:** pipeline hiện mặc định ngôn ngữ Trung và ROI/heuristic đã tinh chỉnh cho một kiểu video. Không thể bảo đảm “mọi video” nếu chưa biết ngôn ngữ, font, vị trí chữ và chất lượng hình; cần profile theo ngôn ngữ/nguồn và xác nhận thủ công khi nằm ngoài phạm vi đã kiểm chứng.

**Kết luận:** OCR subtitle hiện khá tốt trên bộ 20 tập này nhưng tỷ lệ 99.2% chưa bao gồm đầy đủ cue thiếu/thừa; caption có kết quả văn bản rất tốt trên 18 cue đầu, song cần kiểm lại một frame và timestamp trước khi gọi là xác nhận thị giác hoàn tất. Bước tiếp theo phù hợp là cập nhật bốn tài liệu để bỏ hướng LLM, chốt cách đo có tính cả miss/extra, rồi tách pipeline OCR–merge thuần khỏi `OCR_LLM`.

---
---

