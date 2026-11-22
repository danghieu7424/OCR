# Nhật ký thay đổi

## [Unreleased]

### Thêm
- Thêm ví dụ `playback_history` minh họa schema version, migration, chuẩn hóa path và upsert nguyên tử bằng API bảng/transaction tổng quát.

### Thay đổi
- Đồng bộ reader và checkpoint giữa các process bằng shared lock trên file database; `checkpoint_if_needed` trả `Deferred` khi reader ở process khác giữ database mở.
- Chuẩn hóa xung đột khóa Windows `ERROR_LOCK_VIOLATION` thành `VfsError::LockBusy`.
- Chuyển schema và helper playback history khỏi core; `Database::create`/`open` không còn tự tạo hoặc migrate schema ứng dụng.
- Kiểm thử backup/restore với WAL có committed frames và checkpoint bị trì hoãn bởi reader snapshot.
- Đồng bộ `Wal::open` với khóa writer trong lúc đọc và khôi phục WAL để tránh quét frame đang được append.
- Gộp việc đọc header và payload thành một lần VFS read cho mỗi frame khi phục hồi WAL.
- Tối ưu `frame_checksums` và `crc32c_append` bằng bảng CRC32C và một lượt quét cho hai checksum frame, không đổi định dạng WAL.

### Gỡ bỏ
- Gỡ bỏ playback-specific API khỏi core và facade; giữ nguyên bảng history cũ như bảng ứng dụng để tương thích dữ liệu.

### Thay đổi phá vỡ tương thích
- API bị gỡ khỏi `ferritedb_core` và `ferritedb`: `PlaybackHistoryEntry`, `PLAYBACK_HISTORY_SCHEMA_VERSION`, `normalize_media_path`, `upsert_playback_history`, `get_playback_history`, `schema_version` và các lỗi riêng cho playback history.
- Chưa chốt version bump tiếp theo: cần xác nhận trạng thái phát hành các version hiện tại và chạy thành công các gate biên dịch/kiểm thử.

## Lịch sử version manifest (chưa xác nhận phát hành)

Các heading dưới đây ghi lại thay đổi theo version trong `Cargo.toml`, không xác nhận
ngày phát hành. Crates.io lookup không tìm thấy `ferritedb-core@0.4.0` hoặc
`ferritedb@0.2.0`; remote GitHub hiện trả `Repository not found`, nên trạng thái
phát hành qua nguồn khác vẫn chưa xác minh.

## [0.4.0] (Cargo.toml commit `66fc12a`; release status/date unverified)

### Thêm
- Thêm `PlaybackHistoryEntry`, `normalize_media_path`, `upsert_playback_history` và `get_playback_history`, cùng migration catalog lên schema version 1.
- Thêm `WalStats`, `CheckpointOutcome`, `checkpoint_if_needed` và API `backup_to`/`restore_from` cho cặp database/WAL.
- Thêm benchmark `open_bench` so sánh mở database sau checkpoint và khi còn WAL.

### Thay đổi
- Bổ sung fault-injection tests quanh commit và checkpoint WAL; benchmark và xác minh Android trên thiết bị chưa có kết quả.

## [0.3.3] (Cargo.toml commit `6a33612`; release status/date unverified)

### Thêm
- Thêm crate facade `ferritedb` với `DatabaseBuilder`, VFS mặc định, API SQL/cache và lối truy cập core.
- Thêm index B-tree khóa ghép và đồng bộ index trong cùng transaction với insert/update/delete.
- Thêm `ferrite-vdbe` cho SQL cơ bản và `ferrite-cache` cho Cache-Aside với Valkey/Redis.
- Thêm `CommitHook` để phối hợp trạng thái cache với vòng đời commit/rollback transaction.
- Khai báo giấy phép kép `MIT OR Apache-2.0`.

## [0.3.2] (Cargo.toml commit `0983a6f`; release status/date unverified)

### Thêm
- Bổ sung tài liệu README về API Rust, transaction, benchmark và giới hạn hiện tại.

## [0.3.1] (Cargo.toml commit `8322014`; release status/date unverified)

### Thay đổi
- Bổ sung test phục hồi WAL khi commit bị ngắt giữa chừng bằng `SimVfs`.

## [0.3.0] (Cargo.toml commit `1db2254`; release status/date unverified)

### Thêm
- Thêm `DatabaseTransaction` để nhóm nhiều thay đổi trong một commit WAL và rollback khi transaction bị drop.
- Thêm benchmark truy vấn.

## [0.2.0] (Cargo.toml commit `1a8c09f`; release status/date unverified)

### Thêm
- Thêm API `Database` lưu trữ bền vững, truy vấn record theo rowid và checkpoint WAL.

## [0.1.0] (Cargo.toml commit `1fdcea8`; release status/date unverified)

### Thêm
- Khởi tạo các crate VFS, pager, B-tree và record codec; bổ sung page 4096 byte, WAL/checksum/snapshot, `SimVfs` fault injection và kiểm tra toàn vẹn B-tree.

### Sửa
- Sửa mã hóa Varint 9 byte để khôi phục chính xác giá trị 64-bit lớn.
