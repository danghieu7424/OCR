# FerriteDB

FerriteDB là cơ sở dữ liệu nhúng viết bằng Rust, phân tầng VFS, Pager/WAL, B-tree và API Rust. Dự án lưu trữ record theo `rowid`, có VDBE cho một tập con SQL và crate cache kết nối Valkey/Redis.

## Tính năng hiện có

- Trang dữ liệu 4096 byte, page cache Clock và kiểm tra database header.
- Record codec hỗ trợ NULL, integer, real, UTF-8 text và blob.
- Table B-tree hỗ trợ tìm kiếm, thêm/sửa/xóa row, tách trang và kiểm tra tính toàn vẹn.
- Index B-tree hỗ trợ khóa ghép có thứ tự và tra cứu các `rowid` tương ứng; `Database` đồng bộ index trong cùng transaction với thao tác trên bảng.
- Catalog `ferrite_master` lưu metadata bảng và index theo cấu trúc năm cột tương tự SQLite, hỗ trợ nhiều bảng trong định dạng file riêng của FerriteDB.
- WAL riêng với CRC32C, khôi phục transaction chưa commit và checkpoint.
- WAL snapshot cho phép nhiều reader giữ `max_frame` riêng trong khi một writer nối WAL; checkpoint bị giới hạn bởi `min_reader_frame`.
- Reader ở nhiều process được phối hợp bằng khóa hệ điều hành trên file database; checkpoint được hoãn khi process khác còn mở reader.
- `SimVfs` hỗ trợ fault injection để kiểm tra ghi lỗi, ghi xé và phục hồi.
- API Rust `Database` và `DatabaseTransaction`; bỏ transaction chưa commit sẽ rollback các trang đang staging.
- Ví dụ `playback_history` minh họa chuẩn hóa path, upsert trong một transaction và migration schema version 0→1; đây không phải API tích hợp sẵn trong core hoặc facade.
- WAL metrics (`WalStats`) và `checkpoint_if_needed` với ngưỡng byte tường minh; checkpoint báo riêng trạng thái bị hoãn do reader cũ.
- Backup/restore tối thiểu sao chép nhất quán cặp database/WAL; restore kiểm tra database trước khi trả handle.
- Crate `ferrite-vdbe` phân tích, biên dịch và chạy `CREATE TABLE`, `CREATE INDEX`, `INSERT`, `SELECT`, `UPDATE`, `DELETE`; bộ lập kế hoạch dùng B-tree index cho điều kiện equality phù hợp.
- Crate `ferrite-cache` cung cấp Cache-Aside cho Valkey/Redis, dirty key và Lua script nguyên tử; `CommitHook` nối cập nhật cache với WAL transaction.

## Yêu cầu

- Rust toolchain hỗ trợ edition 2024.
- Trên Windows, cần MSVC Build Tools và Windows SDK để build target MSVC. Trong `cmd.exe`, nạp môi trường MSVC trước khi gọi Cargo:

  ```bat
  call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
  ```

- Để dùng dependency audit đã cài trong workspace, đặt `CARGO_HOME=.\.cargo-local` trước khi chạy `cargo audit`.

## Dùng từ Rust

Workspace có crate facade `ferritedb` để ứng dụng chỉ cần một điểm import cho API phổ biến. Bản crate hiện nằm trong workspace; sau khi phát hành lên registry có thể khai báo trong dự án khác như sau:

```toml
[dependencies]
ferritedb = { version = "0.2", features = ["sql"] }
```

```rust
use ferritedb::{Database, Value};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut database = Database::create("example.fdb")?;

    database.insert(1, &[Value::Text("Ada"), Value::Integer(37)])?;
    let row = database.get(1)?.expect("inserted row exists");
    let record = ferritedb::RecordRef::new(&row)?;
    assert_eq!(record.get(0)?, Some(ferritedb::ValueRef::Text("Ada")));

    database.checkpoint()?;
    Ok(())
}
```

`Database::open(path)` mở lại database; `Database::builder(path)` cho phép đặt page-cache capacity (mặc định 64). `Database` dùng `OsVfs` mặc định, giải phóng ứng dụng khỏi việc cấu hình VFS trong trường hợp thông thường. `Database::into_core` và `Database::from_core` vẫn cho phép dùng API low-level hoặc VFS tùy chỉnh. Database dùng thêm sidecar `<đường-dẫn>-wal`. Custom VFS cần triển khai shared/exclusive file lock tương thích nếu dùng đồng thời từ nhiều process.

Feature mặc định là `sql`. `ferritedb::sql::execute_sql` chạy một statement SQL; feature `cache` bổ sung adapter Valkey/Redis và tự bật `sql`, còn `full` bật cả SQL lẫn cache. Dùng `database.into_cached(client, namespace)` để chuyển facade database sang `CachedDatabase`. Các crate `ferrite-pager`, `ferrite-btree`, `ferrite-vfs`, `ferrite-vdbe` và `ferrite-cache` vẫn có thể được dùng riêng cho ứng dụng cần kiểm soát sâu hơn.

### Tích hợp vào backend bất đồng bộ

API database hiện tại là đồng bộ. Backend Tokio nên gọi nó ngoài executor bất đồng bộ qua một worker blocking có queue giới hạn và một chủ sở hữu `Database`; không giữ mutex guard qua `.await` và không tạo một kết nối database mới cho mỗi request. Queue giới hạn giúp tạo backpressure thay vì để request dồn thành một lượng công việc không giới hạn. Chỉ mở rộng số worker đọc sau khi đã đo tải và xác nhận giới hạn snapshot/lock phù hợp; các ghi vẫn được tuần tự hóa.

Đặt file database và WAL trong thư mục dữ liệu riêng, không nằm dưới thư mục được phục vụ tĩnh. Worker nên xử lý `LockBusy` bằng retry có giới hạn và backoff; không lặp vô hạn khi có xung đột file hoặc process khác. `checkpoint_if_needed` cần được gọi theo chính sách của ứng dụng, không chạy đồng bộ trên luồng xử lý request nếu độ trễ checkpoint chưa được đo. Nếu process khác đang giữ database mở, checkpoint sẽ được hoãn; hãy retry có giới hạn sau khi reader bên kia đóng.

Ví dụ SQL ngắn:

```rust
use ferritedb::Database;
use ferritedb::sql::execute_sql;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut database = Database::create("products.fdb")?;
    execute_sql(&mut database, "CREATE TABLE products (name TEXT, qty INTEGER)")?;
    execute_sql(&mut database, "INSERT INTO products VALUES ('pear', 4)")?;
    let result = execute_sql(&mut database, "SELECT name, qty FROM products")?;
    println!("{result:?}");
    Ok(())
}
```

### Transaction và index

Gọi `begin_transaction`, thực hiện nhiều thao tác `insert`, `update`, `delete` hoặc `create_index`, rồi gọi `commit` để ghi một WAL transaction. Nếu transaction bị drop trước `commit`, hoặc bất kỳ thao tác nào lỗi, các thay đổi staging không được lưu. Rowid trùng sẽ trả lỗi thay vì ghi đè. Facade cung cấp các thao tác công khai tường minh để giữ bề mặt API ổn định; dùng `as_core`, `as_core_mut` hoặc `into_core` khi cần API low-level. API low-level tương ứng nằm trong crate `ferritedb_core`.

`Database` tạo sẵn cây `records` để tương thích API Rust cũ; có thể tạo thêm bảng bằng `create_table`. Crate `ferrite-vdbe` hỗ trợ cú pháp SQL cơ bản với predicate equality đơn giản, không có JOIN hay biểu thức tổng quát. Dùng `compile(sql)` để tạo bytecode `OpCode`, rồi `execute(&mut database, &program)` để chạy. Planner chọn index đơn cột khi predicate khớp cột được đánh chỉ mục.

### Playback history PoC

Logic playback history không nằm trong public core API; ví dụ
[`playback_history`](examples/playback_history.rs) dùng API bảng và transaction
tổng quát để minh họa schema version 1, migration, chuẩn hóa path và upsert
nguyên tử. Chạy bằng `cargo run --example playback_history -- <db-path> <media-path>`.
Upsert quét bảng nhỏ trong cùng transaction; đây là uniqueness
ở tầng ứng dụng, không phải unique index tổng quát. Path chuẩn hóa dấu `/` và
`\`, bỏ `.` và rút gọn `..`, giữ nguyên chữ hoa/thường, không phân giải symlink
hay kiểm tra file tồn tại. Nếu writer khác commit trước trên snapshot cũ, ứng
dụng cần mở lại handle rồi retry transaction. Migration giữ nguyên bảng và
record hiện có; không đổi page size hay file format, bao gồm cả bảng history
cũ được tạo bởi PoC trước đó.

`wal_stats()` trả số byte vật lý và committed frame. `checkpoint_if_needed(threshold_bytes)` chỉ thử checkpoint khi có frame committed và WAL đạt ngưỡng đã chọn; kết quả `CheckpointOutcome` phân biệt `NotNeeded`, `Checkpointed` và `Deferred`. Đây là chính sách tường minh, không có background timer.

`backup_to(path)` tạo cặp `path`/`path-wal` nhất quán dưới WAL writer lock; cả hai đích phải chưa tồn tại. `Database::restore_from(backup, destination)` chỉ ghi vào cặp đích mới, rồi mở/kiểm tra database trước khi trả về. Backup không thay thế lưu trữ an toàn ngoài thiết bị; lỗi giữa lúc sao chép có thể để lại file dở dang cần xóa thủ công.

### Giới hạn SQL và lưu trữ

- `UPDATE` và `DELETE` bắt buộc có `WHERE`; hiện chỉ hỗ trợ một điều kiện dạng `cột = giá_trị`. Chưa hỗ trợ `JOIN`, `ORDER BY` hoặc `LIMIT`.
- Trang dữ liệu có kích thước 4096 byte. Mỗi record cùng cell B-tree phải nằm gọn trong một trang; chưa có overflow page.
- Chưa có freelist để tái sử dụng trang sau khi xóa, nên xóa record không làm file database nhỏ lại.
- `Database::open` kiểm tra catalog và toàn bộ B-tree khi mở file; thời gian mở tăng theo số trang dữ liệu.
- Playback-history upsert chỉ là helper trong example, quét toàn bảng và phù hợp PoC-size; core không cung cấp unique constraint hay API playback-specific.
- `checkpoint_if_needed` là chính sách gọi tường minh, không tự chạy theo timer. `WalStats::bytes` tính cả WAL header; `CheckpointOutcome::Deferred` nghĩa là vẫn còn frame committed chưa checkpoint hết (chẳng hạn do snapshot reader cũ, reader ở process khác hoặc ghi đồng thời).
- `Wal::open` lấy khóa writer trong khi đọc và phục hồi WAL; nếu writer đang append, lời gọi có thể trả `LockBusy` và ứng dụng cần retry có giới hạn.
- `backup_to` ghi database và WAL thành hai file cùng tên gốc và hậu tố `-wal`; đích phải chưa tồn tại. `restore_from` chỉ ghi vào cặp đường dẫn đích mới. Nếu I/O lỗi giữa chừng, file backup/restore dở dang không tự được xóa; hãy loại bỏ chúng trước khi thử lại. Không thay thế việc đồng bộ thư mục hay kiểm thử durability trên thiết bị thật.

Để bật cache Valkey/Redis, tạo `RedisValkey::connect(url)` rồi truyền vào `CachedDatabase`. Transaction đánh dấu dirty trước khi ghi WAL; sau khi commit bền vững, Lua script cập nhật/xóa cache và xóa dirty marker nguyên tử. Cache-Aside bỏ qua dữ liệu cache còn dirty và nạp lại từ FerriteDB.

### SQL và Valkey demo

Chạy ví dụ SQL end-to-end với mock Valkey tích hợp, không cần dịch vụ ngoài:

```sh
cargo run --example sql_valkey_demo
```

Ví dụ chạy `CREATE TABLE`, `CREATE INDEX`, `INSERT`, `SELECT`, `UPDATE` và `DELETE` qua VDBE trên `CachedDatabase`; log cache miss/hit, dirty invalidation và đồng bộ sau commit. Cuối luồng, ví dụ so sánh độ trễ trung bình của 100 lần đọc qua handle FerriteDB mới với `CachedDatabase`/Valkey. Mặc định dùng `InMemoryValkey`; để dùng Redis/Valkey thật, đặt biến môi trường trước khi chạy:

```powershell
$env:FERRITEDB_VALKEY_URL = "redis://127.0.0.1/"
cargo run --example sql_valkey_demo
```

Benchmark minh họa hai đường truy cập API, không đảm bảo đọc vật lý từ đĩa: OS và filesystem vẫn có thể cache dữ liệu.

## Kiểm thử và lint

Chạy từ thư mục gốc:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo audit
```

Trước lệnh `cargo audit`, đặt `CARGO_HOME=.\.cargo-local` (PowerShell: `$env:CARGO_HOME = ".\.cargo-local"`). Các test bao gồm property tests cho varint, record codec, slotted page và tách B-tree, cùng transaction commit/rollback, reopen, checkpoint giới hạn theo reader, snapshot isolation đa luồng và fault injection khi commit/checkpoint WAL.

Các crate trong workspace:

| Crate | Version | Vai trò |
|---|---:|---|
| `ferritedb-core` | 0.4.1 | Core database API |
| `ferritedb` | 0.2.1 | Facade ứng dụng, SQL/cache tùy chọn |
| `ferrite-btree` | 0.4.0 | Record codec và table/index B-tree |
| `ferrite-pager` | 0.5.2 | Page cache, WAL và checkpoint |
| `ferrite-vfs` | 0.2.2 | Virtual filesystem và `OsVfs`/`SimVfs` |
| `ferrite-vdbe` | 0.1.0 | SQL parser/compiler và VDBE |
| `ferrite-cache` | 0.1.0 | Cache-Aside Valkey/Redis |

## Benchmark

Chạy benchmark truy vấn và ghi theo batch:

```sh
cargo run --release --example database_bench -- 10000
```

Tham số là số row; mặc định là `10000`. Benchmark dùng `OsVfs`, ghi tất cả row trong một transaction, commit một lần, đo lookup theo `rowid`, kiểm tra B-tree, checkpoint và xóa các file tạm. Đây là benchmark point lookup của API hiện tại, **không phải benchmark SQL**.

Đo baseline thời gian `Database::open` trên database đã checkpoint và database còn WAL:

```sh
cargo run --release --example open_bench -- 5000 20
```

Tham số là số record và số lần lấy mẫu. Benchmark báo số trang, WAL bytes/frame cùng median/mean thời gian mở; kết quả phụ thuộc môi trường. Trên Samsung SM-J250F chạy Android 7.1.1, mẫu 5.000 record/20 lần mở đo được median 8,997 µs cho DB đã checkpoint và 13,135 µs cho DB có WAL qua ba lượt; đây không phải ngưỡng đảm bảo. Xem [báo cáo đo trên thiết bị](docs/reviews/2026-11-17-android-device-evaluate.md).

Một lần chạy tham chiếu ngày 2026-11-14 trên môi trường phát triển hiện tại:

```text
rows: 10000; tree pages: 103
batch insert: 30694 rows/s (0.326s)
point lookup: 50637 queries/s (0.197s)
```

Đây chỉ là kết quả của một lần chạy; hiệu năng phụ thuộc CPU, ổ đĩa, filesystem, hệ điều hành và trạng thái cache. Không xem các con số này là ngưỡng đảm bảo hoặc so sánh trực tiếp giữa các máy.

Benchmark tầng thấp bổ sung:

```sh
cargo run --release -p ferrite-pager --example page_cache_bench
cargo run --release -p ferrite-vfs --example page_io_bench
```

`page_cache_bench` đo page-cache hit và lần đọc lặp qua OS VFS; `page_io_bench` đo I/O tuần tự theo trang 4 KiB (thời gian ghi bao gồm `sync_all`). Các benchmark này đo tầng lưu trữ, không đại diện cho thời gian thực thi câu SQL.

## Giới hạn và lưu ý

- WAL hỗ trợ nhiều snapshot reader đồng thời với một writer. Writer được tuần tự hóa bằng khóa WAL; transaction từ snapshot writer cũ bị từ chối để tránh ghi đè commit mới. Mỗi tiến trình giữ shared lock trên file database trong khi còn reader mở; checkpoint khóa độc quyền ngắn trên file chính và bị hoãn nếu reader từ process khác còn hoạt động. Trong cùng tiến trình, registry snapshot và read/write lock cho phép checkpoint theo `min_reader_frame` mà không ghi đè snapshot cũ. Reader ở process khác không được đăng ký snapshot chi tiết, nên checkpoint chờ toàn bộ process đó đóng database.
- Các test fault injection kiểm tra những điểm crash được mô phỏng; chúng không chứng minh mọi đảm bảo ACID trên mọi filesystem/phần cứng.
- Kiểm thử Android hiện chạy binary dưới tài khoản `shell` trong `/data/local/tmp`, chưa xác minh quyền truy cập trong sandbox riêng của ứng dụng WBF.
- CRC32C phát hiện hỏng dữ liệu ngẫu nhiên, không phải cơ chế chống sửa đổi có chủ đích.
- Có thể kiểm tra advisory của dependency bằng `cargo audit`. Để giữ cache/tool Cargo trong workspace, đặt `CARGO_HOME=./.cargo-local` trước khi chạy; trên PowerShell dùng `$env:CARGO_HOME = ".\\.cargo-local"`. Dependency audit không thay thế đánh giá bảo mật tổng thể.

## Giấy phép

FerriteDB được cấp phép kép theo lựa chọn của người dùng: [MIT](https://opensource.org/license/mit) hoặc [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0). SPDX identifier: `MIT OR Apache-2.0`.
