use ferrite_btree::{RecordRef, Value};
use ferrite_pager::Wal;
use ferrite_vfs::OsVfs;
use ferritedb_core::Database;
use std::error::Error;
use std::fs;
use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let row_count = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(10_000);
    if row_count == 0 {
        return Err("row count must be greater than zero".into());
    }

    let path = std::env::temp_dir().join(format!("ferritedb-bench-{}.fdb", std::process::id()));
    let wal_path = Wal::path_for_database(&path);
    let vfs = OsVfs;
    let mut database = Database::create(&vfs, &path, 64)?;

    let insert_start = Instant::now();
    let mut transaction = database.begin_transaction();
    for rowid in 1..=row_count {
        let rowid = i64::try_from(rowid)?;
        let value = format!("record-{rowid}");
        transaction.insert(rowid, &[Value::Integer(rowid), Value::Text(&value)])?;
    }
    transaction.commit()?;
    let insert_elapsed = insert_start.elapsed();

    let lookup_start = Instant::now();
    for rowid in (1..=row_count).rev() {
        let record = database
            .get(i64::try_from(rowid)?)?
            .ok_or("benchmark row was not found")?;
        let decoded = RecordRef::new(&record)?;
        if decoded.field_count() != 2 {
            return Err("benchmark row has an unexpected field count".into());
        }
    }
    let lookup_elapsed = lookup_start.elapsed();

    let stats = database.validate()?;
    database.checkpoint(&vfs)?;
    drop(database);
    fs::remove_file(&path)?;
    fs::remove_file(&wal_path)?;

    println!("rows: {row_count}; tree pages: {}", stats.pages);
    println!(
        "batch insert: {:.0} rows/s ({:.3}s)",
        row_count as f64 / insert_elapsed.as_secs_f64(),
        insert_elapsed.as_secs_f64()
    );
    println!(
        "point lookup: {:.0} queries/s ({:.3}s)",
        row_count as f64 / lookup_elapsed.as_secs_f64(),
        lookup_elapsed.as_secs_f64()
    );
    Ok(())
}
