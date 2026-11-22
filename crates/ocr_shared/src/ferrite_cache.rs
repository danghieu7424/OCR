//! Cache pipeline studio trên FerriteDB (không dùng SQL).
//!
//! Mỗi record FerriteDB phải nằm gọn trong một page 4096 byte và FerriteDB chưa
//! hỗ trợ overflow page, nên text được giới hạn [`MAX_TEXT_CHARS`] ký tự và ảnh
//! crop bằng chứng được lưu ra file `.png` trên đĩa (DB chỉ giữ đường dẫn).

use anyhow::{Result, anyhow};
use ferrite_btree::{RecordRef, Value, ValueRef};
use ferrite_vfs::OsVfs;
use ferritedb_core::{Database, DatabaseError};
use std::path::Path;

pub const TBL_META: &str = "pipeline_meta";
pub const TBL_RAW_CUES: &str = "raw_cues";
pub const TBL_VLM_JOBS: &str = "vlm_jobs";
pub const TBL_GLOSSARY: &str = "entity_glossary";
pub const TBL_STUDIO_CUES: &str = "studio_cues";

const MAX_TEXT_CHARS: usize = 500;
const CACHE_CAPACITY: usize = 128;
const WAL_CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;

const IDX_RAW_CUES_CHUNK: &str = "idx_raw_cues_chunk";
const IDX_STUDIO_CUES_CHUNK: &str = "idx_studio_cues_chunk";

/// Một cue thô sau Stage 1 (đã gắn mốc VAD để chống gộp sai nhịp TTS)
#[derive(Debug, Clone, PartialEq)]
pub struct CachedRawCue {
    pub chunk_id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    /// "SUB" (sub đáy) hoặc "CAP" (thẻ tên/địa danh/cấp bậc)
    pub cue_kind: String,
    /// Nhãn từ Light-ASD / active speaker, rỗng nếu người nói ngoài khung hình
    pub speaker_hint: String,
    pub raw_zh: String,
    pub confidence: f64,
    /// Tỷ lệ có tiếng nói thật trong khoảng [start_ms, end_ms]
    pub vad_speech_ratio: f64,
}

/// Một crop caption cần Stage 2 (VLM) đọc lại; gắn với cue bắt đầu tại `start_ms`.
#[derive(Debug, Clone, PartialEq)]
pub struct VlmJobRequest {
    pub start_ms: i64,
    pub crop_png_path: String,
    pub reason: String,
}

/// Quản lý trạng thái pipeline trên FerriteDB, tối ưu cho page 4096 byte.
pub struct StudioCacheDb {
    vfs: OsVfs,
    db: Database,
}

impl StudioCacheDb {
    pub fn open_or_create(db_path: &Path) -> Result<Self> {
        let vfs = OsVfs;
        let is_empty_file = db_path
            .metadata()
            .map(|metadata| metadata.len() == 0)
            .unwrap_or(false);
        let mut db = if db_path.exists() && !is_empty_file {
            Database::open(&vfs, db_path, CACHE_CAPACITY)
        } else {
            Database::create(&vfs, db_path, CACHE_CAPACITY)
        }
        .map_err(|e| anyhow!("Không thể mở FerriteDB {:?}: {}", db_path, e))?;

        Self::ensure_schema(&mut db)?;
        Ok(Self { vfs, db })
    }

    fn ensure_schema(db: &mut Database) -> Result<()> {
        let tables: &[(&str, &[&str])] = &[
            (TBL_META, &["key", "val"]),
            (
                TBL_RAW_CUES,
                &[
                    "chunk_id",
                    "start_ms",
                    "end_ms",
                    "cue_kind",
                    "speaker_hint",
                    "raw_zh",
                    "confidence",
                    "vad_speech_ratio",
                ],
            ),
            (
                TBL_VLM_JOBS,
                &["raw_cue_rowid", "crop_png_path", "reason", "resolved_zh"],
            ),
            (
                TBL_GLOSSARY,
                &["zh_term", "vi_term", "category", "pronoun_rule"],
            ),
            (
                TBL_STUDIO_CUES,
                &[
                    "chunk_id",
                    "start_ms",
                    "end_ms",
                    "cue_kind",
                    "speaker",
                    "clean_zh",
                    "translated_vi",
                    "tts_pause_after_ms",
                ],
            ),
        ];

        let mut created = false;
        for (table, columns) in tables {
            // table_columns lỗi sẽ abort transaction nên mỗi lần kiểm tra phải
            // chạy trong transaction riêng.
            match db.table_columns(table) {
                Ok(_) => {}
                Err(DatabaseError::TableNotFound(_)) => {
                    db.create_table(table, columns)
                        .map_err(|e| anyhow!("Lỗi tạo bảng {}: {}", table, e))?;
                    created = true;
                }
                Err(e) => return Err(anyhow!("Lỗi kiểm tra bảng {}: {}", table, e)),
            }
        }

        if created {
            // Cột chunk_id là cột 0 của cả hai bảng.
            db.create_index_on(IDX_RAW_CUES_CHUNK, TBL_RAW_CUES, &[0])
                .map_err(|e| anyhow!("Lỗi tạo index raw_cues: {}", e))?;
            db.create_index_on(IDX_STUDIO_CUES_CHUNK, TBL_STUDIO_CUES, &[0])
                .map_err(|e| anyhow!("Lỗi tạo index studio_cues: {}", e))?;
        }
        Ok(())
    }

    /// Ghi trọn gói một chunk trong 1 transaction, rồi checkpoint nếu WAL > 4MB.
    ///
    /// `vlm_crops` là `(vị trí cue trong `cues`, đường dẫn crop PNG, lý do đẩy VLM)`.
    pub fn save_chunk_raw_cues(
        &mut self,
        chunk_id: i64,
        cues: &[CachedRawCue],
        vlm_crops: &[(usize, String, String)],
    ) -> Result<()> {
        let mut tx = self.db.begin_transaction();
        let mut inserted_rowids = Vec::with_capacity(cues.len());

        for cue in cues {
            let safe_zh = clamp_text(&cue.raw_zh);
            let rowid = tx
                .next_rowid(TBL_RAW_CUES)
                .map_err(|e| anyhow!("next_rowid raw_cues error: {}", e))?;
            tx.insert_into(
                TBL_RAW_CUES,
                rowid,
                &[
                    Value::Integer(chunk_id),
                    Value::Integer(cue.start_ms),
                    Value::Integer(cue.end_ms),
                    Value::Text(cue.cue_kind.as_str()),
                    Value::Text(cue.speaker_hint.as_str()),
                    Value::Text(&safe_zh),
                    Value::Real(cue.confidence),
                    Value::Real(cue.vad_speech_ratio),
                ],
            )
            .map_err(|e| anyhow!("insert raw_cue error: {}", e))?;
            inserted_rowids.push(rowid);
        }

        for (cue_index, crop_path, reason) in vlm_crops {
            let Some(&raw_rowid) = inserted_rowids.get(*cue_index) else {
                continue;
            };
            let job_id = tx
                .next_rowid(TBL_VLM_JOBS)
                .map_err(|e| anyhow!("next_rowid vlm_jobs error: {}", e))?;
            tx.insert_into(
                TBL_VLM_JOBS,
                job_id,
                &[
                    Value::Integer(raw_rowid),
                    Value::Text(crop_path.as_str()),
                    Value::Text(reason.as_str()),
                    Value::Text(""),
                ],
            )
            .map_err(|e| anyhow!("insert vlm_job error: {}", e))?;
        }

        tx.commit()
            .map_err(|e| anyhow!("commit chunk {} error: {}", chunk_id, e))?;

        let _ = self
            .db
            .checkpoint_if_needed(&self.vfs, WAL_CHECKPOINT_BYTES);
        Ok(())
    }

    /// Ghi hàng đợi `vlm_jobs` cho các crop caption cần Stage 2 đọc lại.
    ///
    /// `raw_cue_rowid` được tra từ `raw_cues` theo `(chunk_id, start_ms)`. Cue
    /// không có trong cache và job đã tồn tại đều bị bỏ qua, nên gọi lặp lại vẫn
    /// idempotent. Trả về số job mới đã ghi.
    pub fn save_vlm_jobs(&mut self, chunk_id: i64, jobs: &[VlmJobRequest]) -> Result<usize> {
        if jobs.is_empty() {
            return Ok(0);
        }

        let mut rowid_by_start: Vec<(i64, i64)> = Vec::new();
        for (rowid, bytes) in self
            .db
            .scan_table(TBL_RAW_CUES)
            .map_err(|e| anyhow!("scan raw_cues error: {}", e))?
        {
            let cue = decode_raw_cue(&bytes)?;
            if cue.chunk_id == chunk_id {
                rowid_by_start.push((cue.start_ms, rowid));
            }
        }

        let mut queued_rowids: Vec<i64> = Vec::new();
        for (_, bytes) in self
            .db
            .scan_table(TBL_VLM_JOBS)
            .map_err(|e| anyhow!("scan vlm_jobs error: {}", e))?
        {
            if let Ok(record) = RecordRef::new(&bytes)
                && let Ok(Some(ValueRef::Integer(rowid))) = record.get(0)
            {
                queued_rowids.push(rowid);
            }
        }

        let mut tx = self.db.begin_transaction();
        let mut inserted = 0usize;
        for job in jobs {
            let Some(&(_, raw_rowid)) = rowid_by_start
                .iter()
                .find(|(start_ms, _)| *start_ms == job.start_ms)
            else {
                continue;
            };
            if queued_rowids.contains(&raw_rowid) {
                continue;
            }
            let job_id = tx
                .next_rowid(TBL_VLM_JOBS)
                .map_err(|e| anyhow!("next_rowid vlm_jobs error: {}", e))?;
            tx.insert_into(
                TBL_VLM_JOBS,
                job_id,
                &[
                    Value::Integer(raw_rowid),
                    Value::Text(job.crop_png_path.as_str()),
                    Value::Text(job.reason.as_str()),
                    Value::Text(""),
                ],
            )
            .map_err(|e| anyhow!("insert vlm_job error: {}", e))?;
            queued_rowids.push(raw_rowid);
            inserted += 1;
        }

        tx.commit()
            .map_err(|e| anyhow!("commit vlm_jobs chunk {} error: {}", chunk_id, e))?;
        let _ = self
            .db
            .checkpoint_if_needed(&self.vfs, WAL_CHECKPOINT_BYTES);
        Ok(inserted)
    }

    /// Chunk đã OCR xong chưa, để chạy lại (resume) bỏ qua chunk đó.
    pub fn has_chunk_raw_cues(&mut self, chunk_id: i64) -> Result<bool> {
        let rows = self
            .db
            .find_index_on(
                IDX_RAW_CUES_CHUNK,
                TBL_RAW_CUES,
                &[Value::Integer(chunk_id)],
            )
            .map_err(|e| anyhow!("Truy vấn chunk {} error: {}", chunk_id, e))?;
        Ok(!rows.is_empty())
    }
}

fn clamp_text(text: &str) -> String {
    text.chars().take(MAX_TEXT_CHARS).collect()
}

/// Decode một record cue đã lưu (dùng khi đọc lại cache cho Stage 3/VTT).
pub fn decode_raw_cue(bytes: &[u8]) -> Result<CachedRawCue> {
    let record =
        RecordRef::new(bytes).map_err(|e| anyhow!("Record raw_cue lỗi định dạng: {}", e))?;
    let integer = |index: usize| -> Result<i64> {
        match record
            .get(index)
            .map_err(|e| anyhow!("Đọc cột {} lỗi: {}", index, e))?
        {
            Some(ValueRef::Integer(value)) => Ok(value),
            _ => Err(anyhow!("Cột {} không phải integer", index)),
        }
    };
    let real = |index: usize| -> Result<f64> {
        match record
            .get(index)
            .map_err(|e| anyhow!("Đọc cột {} lỗi: {}", index, e))?
        {
            Some(ValueRef::Real(value)) => Ok(value),
            Some(ValueRef::Integer(value)) => Ok(value as f64),
            _ => Err(anyhow!("Cột {} không phải real", index)),
        }
    };
    let text = |index: usize| -> Result<String> {
        match record
            .get(index)
            .map_err(|e| anyhow!("Đọc cột {} lỗi: {}", index, e))?
        {
            Some(ValueRef::Text(value)) => Ok(value.to_string()),
            _ => Err(anyhow!("Cột {} không phải text", index)),
        }
    };

    Ok(CachedRawCue {
        chunk_id: integer(0)?,
        start_ms: integer(1)?,
        end_ms: integer(2)?,
        cue_kind: text(3)?,
        speaker_hint: text(4)?,
        raw_zh: text(5)?,
        confidence: real(6)?,
        vad_speech_ratio: real(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ocr_studio_cache_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir.join("studio.ferrite")
    }

    fn cue(start_ms: i64, text: &str) -> CachedRawCue {
        CachedRawCue {
            chunk_id: 7,
            start_ms,
            end_ms: start_ms + 800,
            cue_kind: "SUB".to_string(),
            speaker_hint: String::new(),
            raw_zh: text.to_string(),
            confidence: 0.93,
            vad_speech_ratio: 0.85,
        }
    }

    #[test]
    fn chunk_cache_survives_reopen_and_resumes_by_chunk_id() {
        let path = temp_db_path();
        {
            let mut db = StudioCacheDb::open_or_create(&path).unwrap();
            assert!(!db.has_chunk_raw_cues(7).unwrap());
            db.save_chunk_raw_cues(
                7,
                &[cue(1_000, "啊"), cue(2_000, "我想离开这里")],
                &[(1, "crops/7-1.png".to_string(), "stylized".to_string())],
            )
            .unwrap();
            assert!(db.has_chunk_raw_cues(7).unwrap());
            assert!(!db.has_chunk_raw_cues(8).unwrap());
        }

        let mut db = StudioCacheDb::open_or_create(&path).unwrap();
        assert!(db.has_chunk_raw_cues(7).unwrap());

        let rows = db.db.scan_table(TBL_RAW_CUES).unwrap();
        assert_eq!(rows.len(), 2);
        let first = decode_raw_cue(&rows[0].1).unwrap();
        assert_eq!(first.raw_zh, "啊");
        assert_eq!(first.chunk_id, 7);
        assert_eq!(first.confidence, 0.93);
        let second = decode_raw_cue(&rows[1].1).unwrap();
        assert_eq!(second.raw_zh, "我想离开这里");

        assert_eq!(db.db.scan_table(TBL_VLM_JOBS).unwrap().len(), 1);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn vlm_jobs_are_queued_once_per_raw_cue() {
        let path = temp_db_path();
        let mut db = StudioCacheDb::open_or_create(&path).unwrap();
        db.save_chunk_raw_cues(
            7,
            &[cue(1_000, "《骆丹茲》"), cue(2_000, "我想离开这里")],
            &[],
        )
        .unwrap();

        let jobs = [VlmJobRequest {
            start_ms: 1_000,
            crop_png_path: "vlm_crops/vlm_001.png".to_string(),
            reason: "conf=0.846; stylized".to_string(),
        }];
        assert_eq!(db.save_vlm_jobs(7, &jobs).unwrap(), 1);
        // Gọi lại không nhân đôi job, và chunk khác không khớp.
        assert_eq!(db.save_vlm_jobs(7, &jobs).unwrap(), 0);
        assert_eq!(db.save_vlm_jobs(9, &jobs).unwrap(), 0);

        let rows = db.db.scan_table(TBL_VLM_JOBS).unwrap();
        assert_eq!(rows.len(), 1);
        let record = RecordRef::new(&rows[0].1).unwrap();
        match record.get(1).unwrap() {
            Some(ValueRef::Text(value)) => assert_eq!(value, "vlm_crops/vlm_001.png"),
            other => panic!("crop path sai: {other:?}"),
        }
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn oversized_cue_text_is_truncated_to_stay_inside_one_page() {
        let path = temp_db_path();
        let mut db = StudioCacheDb::open_or_create(&path).unwrap();
        let long_text = "字".repeat(MAX_TEXT_CHARS + 200);
        db.save_chunk_raw_cues(1, &[cue(0, &long_text)], &[])
            .unwrap();

        let rows = db.db.scan_table(TBL_RAW_CUES).unwrap();
        let stored = decode_raw_cue(&rows[0].1).unwrap();
        assert_eq!(stored.raw_zh.chars().count(), MAX_TEXT_CHARS);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
