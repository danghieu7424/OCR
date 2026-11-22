use ferrite_btree::Value;
use ferrite_pager::Wal;
use ferrite_vfs::OsVfs;
use ferritedb_core::Database;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn main() -> Result<(), Box<dyn Error>> {
    let rows = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(5_000);
    let samples = std::env::args()
        .nth(2)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(20);
    if rows == 0 || samples == 0 {
        return Err("row count and sample count must be greater than zero".into());
    }

    let run_id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let checkpointed_path = temporary_path("checkpointed", run_id);
    let wal_path = temporary_path("wal", run_id);
    let vfs = OsVfs;

    let (checkpointed_stats, checkpointed_pages) =
        create_fixture(&vfs, &checkpointed_path, rows, true)?;
    let (wal_stats, wal_pages) = create_fixture(&vfs, &wal_path, rows, false)?;
    let checkpointed_samples = measure_open(&vfs, &checkpointed_path, samples)?;
    let wal_samples = measure_open(&vfs, &wal_path, samples)?;

    println!("rows: {rows}; open samples: {samples}");
    println!(
        "checkpointed: {} pages; WAL {} bytes / {} frames; {}",
        checkpointed_pages,
        checkpointed_stats.bytes,
        checkpointed_stats.committed_frames,
        summarize(&checkpointed_samples)
    );
    println!(
        "WAL-backed: {} pages; WAL {} bytes / {} frames; {}",
        wal_pages,
        wal_stats.bytes,
        wal_stats.committed_frames,
        summarize(&wal_samples)
    );

    remove_database_files(&checkpointed_path)?;
    remove_database_files(&wal_path)?;
    Ok(())
}

fn temporary_path(label: &str, run_id: u128) -> PathBuf {
    std::env::temp_dir().join(format!(
        "ferritedb-open-{label}-{}-{run_id}.fdb",
        std::process::id()
    ))
}

fn create_fixture(
    vfs: &OsVfs,
    path: &Path,
    rows: usize,
    checkpoint: bool,
) -> Result<(ferrite_pager::WalStats, u32), Box<dyn Error>> {
    let mut database = Database::create(vfs, path, 64)?;
    let mut transaction = database.begin_transaction();
    for rowid in 1..=rows {
        let rowid = i64::try_from(rowid)?;
        let media_path = format!("/media/item-{rowid}.mp4");
        transaction.insert(rowid, &[Value::Text(&media_path), Value::Integer(rowid)])?;
    }
    transaction.commit()?;
    if checkpoint {
        database.checkpoint(vfs)?;
    }
    let stats = database.wal_stats()?;
    let page_count = database.page_count();
    drop(database);
    Ok((stats, page_count))
}

fn measure_open(vfs: &OsVfs, path: &Path, samples: usize) -> Result<Vec<u128>, Box<dyn Error>> {
    let mut durations = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        let database = Database::open(vfs, path, 64)?;
        durations.push(start.elapsed().as_micros());
        drop(database);
    }
    durations.sort_unstable();
    Ok(durations)
}

fn summarize(samples: &[u128]) -> String {
    let middle = samples.len() / 2;
    let median = if samples.len().is_multiple_of(2) {
        (samples[middle - 1] + samples[middle]) / 2
    } else {
        samples[middle]
    };
    let average = samples.iter().sum::<u128>() / samples.len() as u128;
    format!("open median {median} us; mean {average} us")
}

fn remove_database_files(path: &Path) -> Result<(), Box<dyn Error>> {
    fs::remove_file(path)?;
    fs::remove_file(Wal::path_for_database(path))?;
    Ok(())
}
