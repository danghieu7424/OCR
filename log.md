```powershell
 OCR  ./ocr-cli/ocr-cli.exe -i "in/S02E001.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E003.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E005.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E006.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E007.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E017.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
 >  ./ocr-cli/ocr-cli.exe -i "in/S02E019.mp4" -o in  --caption-evidence `
 >     --subtitle-roi 0.85,1.0,0.05,0.95 `
 >     --caption-roi 0,0.80,0,1 `
 >     --subtitle-line 90% `
 >     --caption-line 20% `
 >     --force
========================================
 [OCR] in/S02E001.mp4 -> "in\\S02E001.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 134.8s   
 >>> [CAP: WARN] 《三位神明境強者》 | 1.10s | confidence=0.962 | thời lượng gần ngưỡng tối thiểu 1.00s
 >>> [CAP: WARN] 《骆丹茲》 | 0.90s | confidence=0.826 | confidence dưới ngưỡng 0.90, thời lượng gần ngưỡng tối thiểu 1.00s
 >>> [CAP: WARN] 《肖清逸》 | 0.90s | confidence=1.000 | thời lượng gần ngưỡng tối thiểu 1.00s
 >>> Evidence caption: "in\\.tmp_S02E001\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E001\\subtitle_evidence"
 [MERGE: DONE] 58 cues (52 subtitle + 6 caption); raw VTT: "in\\.tmp_S02E001"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E003.mp4 -> "in\\S02E003.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 129.8s   
 >>> Evidence caption: "in\\.tmp_S02E003\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E003\\subtitle_evidence"
 [MERGE: DONE] 52 cues (52 subtitle + 0 caption); raw VTT: "in\\.tmp_S02E003"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E005.mp4 -> "in\\S02E005.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 44.8s   
 >>> Evidence caption: "in\\.tmp_S02E005\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E005\\subtitle_evidence"
 [MERGE: DONE] 15 cues (15 subtitle + 0 caption); raw VTT: "in\\.tmp_S02E005"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E006.mp4 -> "in\\S02E006.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 49.8s   
 >>> Evidence caption: "in\\.tmp_S02E006\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E006\\subtitle_evidence"
 [MERGE: DONE] 27 cues (26 subtitle + 1 caption); raw VTT: "in\\.tmp_S02E006"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E007.mp4 -> "in\\S02E007.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 49.8s   
 >>> Evidence caption: "in\\.tmp_S02E007\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E007\\subtitle_evidence"
 [MERGE: DONE] 27 cues (27 subtitle + 0 caption); raw VTT: "in\\.tmp_S02E007"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E017.mp4 -> "in\\S02E017.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 49.8s   
 >>> [CAP: WARN] 《何訇》 | 1.50s | confidence=0.657 | confidence dưới ngưỡng 0.90
 >>> [CAP: WARN] 《王锻山》 | 1.70s | confidence=0.850 | confidence dưới ngưỡng 0.90
 >>> Evidence caption: "in\\.tmp_S02E017\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E017\\subtitle_evidence"
 [MERGE: DONE] 28 cues (26 subtitle + 2 caption); raw VTT: "in\\.tmp_S02E017"
========================================
>>> DONE ALL <<<
========================================
 [OCR] in/S02E019.mp4 -> "in\\S02E019.merged.zh.vtt"
  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.
 [SUB]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | [CAP]: ▰▰▰▰▰▰▰▰▰▰▰▰ 100.0% | 39.8s   
 >>> [CAP: WARN] 《骆丹茲》 | 1.50s | confidence=0.785 | confidence dưới ngưỡng 0.90
 >>> Evidence caption: "in\\.tmp_S02E019\\caption_evidence"
 >>> Evidence subtitle: "in\\.tmp_S02E019\\subtitle_evidence"
 [MERGE: DONE] 22 cues (21 subtitle + 1 caption); raw VTT: "in\\.tmp_S02E019"
========================================
>>> DONE ALL <<<
```

```powershell
./ocr-cli/ocr-cli.exe -i "in/S02E001.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E003.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E005.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E006.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E007.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E017.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
./ocr-cli/ocr-cli.exe -i "in/S02E019.mp4" -o in  --caption-evidence `
   --subtitle-roi 0.85,1.0,0.05,0.95 `
   --caption-roi 0,0.80,0,1 `
   --subtitle-line 90% `
   --caption-line 20% `
   --force
```