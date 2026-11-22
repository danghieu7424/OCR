# ferritedb

`ferritedb` is the application-facing FerriteDB crate. It supplies a default
OS-backed database API and keeps lower-level storage crates available for
applications that need custom VFS or pager control.

```toml
[dependencies]
ferritedb = { git = "https://github.com/danghieu7424/FerriteDB", package = "ferritedb", features = ["sql"] }
```

```rust
use ferritedb::Database;
use ferritedb::sql::execute_sql;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut database = Database::create("example.fdb")?;
    execute_sql(&mut database, "CREATE TABLE products (name TEXT, qty INTEGER)")?;
    execute_sql(&mut database, "INSERT INTO products VALUES ('pear', 4)")?;
    let rows = execute_sql(&mut database, "SELECT name, qty FROM products")?;
    println!("{rows:?}");
    database.checkpoint()?;
    Ok(())
}
```

The default feature enables the lightweight SQL compiler and executor. Disable
default features for the storage API only. Enable `cache` to add Cache-Aside
integration with the in-memory or Redis/Valkey client; `cache` also enables
`sql`. `full` enables both optional integrations.

For applications using custom storage, create a `CoreDatabase` with the
`FerriteDB` core API and wrap it with `Database::from_core`. Such applications
should checkpoint with `checkpoint_with_vfs` and the same VFS instance used to
open the database.

The core and facade APIs remain application-neutral. The
[`playback_history` example](../../examples/playback_history.rs) demonstrates
application-owned history schema, migration, path normalization, and atomic
upsert using generic table and transaction operations. `wal_stats` exposes
WAL bytes/committed frames, and `checkpoint_if_needed` applies an explicit byte
threshold. `backup_to` copies the database and WAL sidecar to new paths;
`Database::restore_from` restores them to a fresh destination and validates
the result.
