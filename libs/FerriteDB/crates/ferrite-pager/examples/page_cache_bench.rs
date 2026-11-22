use std::error::Error;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ferrite_pager::{PAGE_SIZE, PageNumber, Pager};
use ferrite_vfs::{OpenOptions, OsVfs, Vfs};

const ITERATIONS: usize = 100_000;

fn main() -> Result<(), Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ferritedb-cache-{}-{timestamp}.fdb",
        std::process::id()
    ));
    let cleanup = TempFile(path.clone());
    let vfs = OsVfs;
    let mut pager = Pager::create(&vfs, &path, 64)?;
    let page_number = PageNumber::new(1)?;
    drop(pager.read_page(page_number)?);

    let started = Instant::now();
    let mut checksum = 0_u8;
    for _ in 0..ITERATIONS {
        let page = pager.read_page(page_number)?;
        checksum ^= black_box(page.with_bytes(|bytes| bytes[0])?);
    }
    let cache_elapsed = started.elapsed();
    let cache_stats = pager.cache_stats()?;
    drop(pager);

    let mut file = vfs.open(
        &path,
        OpenOptions {
            read: true,
            write: false,
            create: false,
            truncate: false,
        },
    )?;
    let mut buffer = [0; PAGE_SIZE];
    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let read = file.read_at(0, &mut buffer)?;
        if read != PAGE_SIZE {
            return Err(format!("short read: expected {PAGE_SIZE}, got {read}").into());
        }
        checksum ^= black_box(buffer[0]);
    }
    let vfs_elapsed = started.elapsed();

    println!(
        "Page-cache hit: {:.0} reads/s ({:.2} ns/read), stats={cache_stats:?}",
        ITERATIONS as f64 / cache_elapsed.as_secs_f64(),
        cache_elapsed.as_nanos() as f64 / ITERATIONS as f64
    );
    println!(
        "OS VFS cached page read: {:.0} reads/s ({:.2} us/read)",
        ITERATIONS as f64 / vfs_elapsed.as_secs_f64(),
        vfs_elapsed.as_micros() as f64 / ITERATIONS as f64
    );
    black_box(checksum);
    drop(file);
    drop(cleanup);
    Ok(())
}

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
