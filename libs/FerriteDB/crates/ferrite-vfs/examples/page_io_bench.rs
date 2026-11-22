use std::error::Error;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use ferrite_vfs::{OpenOptions, OsVfs, Vfs};

const PAGE_SIZE: usize = 4096;
const ITERATIONS: usize = 20_000;

fn main() -> Result<(), Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ferritedb-page-io-{}-{timestamp}.tmp",
        std::process::id()
    ));
    let cleanup = TempFile(path.clone());
    let vfs = OsVfs;
    let mut file = vfs.open(
        &path,
        OpenOptions {
            read: true,
            write: true,
            create: true,
            truncate: true,
        },
    )?;
    let page = [0x5a; PAGE_SIZE];
    let mut buffer = [0; PAGE_SIZE];

    let started = Instant::now();
    for index in 0..ITERATIONS {
        write_all_at(&mut *file, (index * PAGE_SIZE) as u64, &page)?;
    }
    file.sync_all()?;
    let write_elapsed = started.elapsed();

    let started = Instant::now();
    let mut checksum = 0_u8;
    for index in 0..ITERATIONS {
        let read = file.read_at((index * PAGE_SIZE) as u64, &mut buffer)?;
        if read != PAGE_SIZE {
            return Err(format!("short read: expected {PAGE_SIZE}, got {read}").into());
        }
        checksum ^= black_box(buffer[0]);
    }
    let read_elapsed = started.elapsed();

    let mib = (ITERATIONS * PAGE_SIZE) as f64 / (1024.0 * 1024.0);
    println!(
        "OS VFS 4 KiB sequential writes: {:.2} MiB/s ({:.0} pages/s, sync included)",
        mib / write_elapsed.as_secs_f64(),
        ITERATIONS as f64 / write_elapsed.as_secs_f64()
    );
    println!(
        "OS VFS 4 KiB sequential reads:  {:.2} MiB/s ({:.0} pages/s)",
        mib / read_elapsed.as_secs_f64(),
        ITERATIONS as f64 / read_elapsed.as_secs_f64()
    );
    black_box(checksum);
    drop(file);
    drop(cleanup);
    Ok(())
}

fn write_all_at(
    file: &mut dyn ferrite_vfs::FileHandle,
    mut offset: u64,
    mut bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    while !bytes.is_empty() {
        let written = file.write_at(offset, bytes)?;
        if written == 0 {
            return Err("write_at returned zero before the page was written".into());
        }
        offset += written as u64;
        bytes = &bytes[written..];
    }
    Ok(())
}

struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
