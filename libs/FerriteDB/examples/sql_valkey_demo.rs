use ferrite_btree::Value;
use ferrite_cache::{CacheClient, CacheState, CachedDatabase, InMemoryValkey, RedisValkey};
use ferrite_vdbe::{ExecutionResult, execute_plan, plan};
use ferrite_vfs::OsVfs;
use ferritedb_core::Database;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct LoggingClient<C> {
    inner: C,
    log_reads: bool,
}

impl<C: CacheClient> CacheClient for LoggingClient<C> {
    fn read_state(&mut self, data_key: &str, dirty_key: &str) -> Result<CacheState, String> {
        let state = self.inner.read_state(data_key, dirty_key)?;
        if self.log_reads {
            if state.dirty_token.is_some() {
                println!("  Cache dirty: bypassing Valkey and reading FerriteDB");
            } else if state.record.is_some() {
                println!("  Cache Hit: read record directly from Valkey");
            } else {
                println!("  Cache Miss: loading record from FerriteDB");
            }
        }
        Ok(state)
    }

    fn mark_dirty(&mut self, dirty_key: &str, token: &str) -> Result<Option<String>, String> {
        let previous = self.inner.mark_dirty(dirty_key, token)?;
        println!("  Dirty Invalidation: marked cache key before WAL commit");
        Ok(previous)
    }

    fn apply_change(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        token: &str,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        let applied = self
            .inner
            .apply_change(data_key, dirty_key, token, record)?;
        if applied {
            if record.is_some() {
                println!("  WAL durable: synchronized updated record to Valkey");
            } else {
                println!("  WAL durable: removed deleted record from Valkey");
            }
        }
        Ok(applied)
    }

    fn restore_dirty(
        &mut self,
        dirty_key: &str,
        token: &str,
        previous: Option<&str>,
    ) -> Result<(), String> {
        self.inner.restore_dirty(dirty_key, token, previous)
    }

    fn refresh_if_unchanged(
        &mut self,
        data_key: &str,
        dirty_key: &str,
        expected_token: Option<&str>,
        record: Option<&[u8]>,
    ) -> Result<bool, String> {
        let refreshed =
            self.inner
                .refresh_if_unchanged(data_key, dirty_key, expected_token, record)?;
        if refreshed && self.log_reads && record.is_some() {
            println!("  Cache warmed from FerriteDB after miss");
        }
        Ok(refreshed)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = demo_path()?;
    let vfs = OsVfs;
    let result: Result<(), Box<dyn Error>> = match std::env::var("FERRITEDB_VALKEY_URL") {
        Ok(url) => {
            println!("Using Redis/Valkey at {url}");
            match RedisValkey::connect(&url) {
                Ok(client) => run(&vfs, &path, client),
                Err(error) => Err(Box::new(error)),
            }
        }
        Err(_) => {
            println!("Using the in-memory Valkey mock (set FERRITEDB_VALKEY_URL for Redis)");
            run(&vfs, &path, InMemoryValkey::default())
        }
    };

    let cleanup = remove_database_files(&path);
    result?;
    cleanup?;
    Ok(())
}

fn run<C: CacheClient + 'static>(vfs: &OsVfs, path: &Path, cache: C) -> Result<(), Box<dyn Error>> {
    let client = Arc::new(Mutex::new(LoggingClient {
        inner: cache,
        log_reads: true,
    }));
    let database = Database::create(vfs, path, 32)?;
    let mut cached =
        CachedDatabase::with_client_handle(database, Arc::clone(&client), "sql-valkey-demo");
    run_sql(
        &mut cached,
        "CREATE TABLE products (name TEXT, qty INTEGER)",
    )?;
    drop(cached);

    let mut database = Database::open(vfs, path, 32)?;
    database.insert_into("products", 1, &[Value::Text("seed"), Value::Integer(3)])?;
    drop(database);

    let database = Database::open(vfs, path, 32)?;
    let mut cached =
        CachedDatabase::with_client_handle(database, Arc::clone(&client), "sql-valkey-demo");
    run_sql(&mut cached, "CREATE INDEX products_name ON products (name)")?;
    run_sql(&mut cached, "INSERT INTO products VALUES ('pear', 4)")?;

    println!("\n-- First indexed read --");
    run_sql(
        &mut cached,
        "SELECT name, qty FROM products WHERE name = 'seed'",
    )?;
    println!("\n-- Repeat indexed read --");
    run_sql(
        &mut cached,
        "SELECT name, qty FROM products WHERE name = 'seed'",
    )?;

    println!("\n-- Update through SQL --");
    run_sql(
        &mut cached,
        "UPDATE products SET qty = 5 WHERE name = 'seed'",
    )?;
    println!("\n-- Read synchronized value --");
    run_sql(
        &mut cached,
        "SELECT name, qty FROM products WHERE name = 'seed'",
    )?;

    measure_reads(vfs, path, &mut cached, &client)?;

    println!("\n-- Delete through SQL --");
    run_sql(&mut cached, "DELETE FROM products WHERE name = 'pear'")?;
    run_sql(
        &mut cached,
        "SELECT name, qty FROM products WHERE name = 'pear'",
    )?;
    Ok(())
}

fn run_sql<C: CacheClient + 'static>(
    database: &mut CachedDatabase<LoggingClient<C>>,
    sql: &str,
) -> Result<ExecutionResult, Box<dyn Error>> {
    println!("SQL> {sql}");
    let plan = plan(database, sql)?;
    println!("  planner: {:?}", plan.access_path);
    let result = execute_plan(database, &plan)?;
    println!("  result: {result:?}");
    Ok(result)
}

fn measure_reads<C: CacheClient + 'static>(
    vfs: &OsVfs,
    path: &Path,
    cached: &mut CachedDatabase<LoggingClient<C>>,
    client: &Arc<Mutex<LoggingClient<C>>>,
) -> Result<(), Box<dyn Error>> {
    const SAMPLES: usize = 100;
    {
        let mut client = client
            .lock()
            .map_err(|_| io::Error::other("cache client lock was poisoned"))?;
        client.log_reads = false;
    }

    let start = Instant::now();
    for _ in 0..SAMPLES {
        let mut database = Database::open(vfs, path, 32)?;
        std::hint::black_box(database.get_from("products", 1)?);
    }
    let direct_ns = start.elapsed().as_nanos() / SAMPLES as u128;

    let start = Instant::now();
    for _ in 0..SAMPLES {
        std::hint::black_box(cached.get("products", 1)?);
    }
    let cache_ns = start.elapsed().as_nanos() / SAMPLES as u128;

    println!(
        "\nAverage read latency ({SAMPLES} samples):\n  fresh FerriteDB handle: {} us\n  CachedDatabase/Valkey:  {} us",
        direct_ns as f64 / 1_000.0,
        cache_ns as f64 / 1_000.0
    );
    Ok(())
}

fn demo_path() -> Result<PathBuf, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(std::env::temp_dir().join(format!(
        "ferritedb-sql-valkey-demo-{}-{timestamp}.fdb",
        std::process::id()
    )))
}

fn remove_database_files(path: &Path) -> io::Result<()> {
    remove_if_present(path)?;
    let mut wal_path = OsString::from(path.as_os_str());
    wal_path.push("-wal");
    remove_if_present(Path::new(&wal_path))
}

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
