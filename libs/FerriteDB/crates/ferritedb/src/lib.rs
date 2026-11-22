//! Application-facing API for FerriteDB.
//!
//! The facade provides an OS-backed [`Database`] and an optional SQL module.
//! Use [`DatabaseBuilder`] to set the page-cache capacity, or use
//! [`CoreDatabase`] with a custom VFS for lower-level control.
//!
//! ```no_run
//! use ferritedb::{Database, Value};
//! use std::error::Error;
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut database = Database::create("example.fdb")?;
//!     database.insert(1, &[Value::Text("pear")])?;
//!     database.checkpoint()?;
//!     Ok(())
//! }
//! ```

use std::path::{Path, PathBuf};

use ferrite_vfs::{OsVfs, Vfs};

static OS_VFS: OsVfs = OsVfs;

pub use ferrite_btree::{RecordRef, TreeStats, Value, ValueRef, encode_record};
pub use ferritedb_core::{
    CheckpointOutcome, CommitChange, CommitHook, Database as CoreDatabase, DatabaseError,
    DatabaseTransaction, IndexInfo, WalStats,
};

#[derive(Clone, Debug)]
pub struct DatabaseBuilder {
    path: PathBuf,
    cache_capacity: usize,
}

impl DatabaseBuilder {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            cache_capacity: 64,
        }
    }

    pub fn cache_capacity(mut self, pages: usize) -> Self {
        self.cache_capacity = pages;
        self
    }

    pub fn create(&self) -> Result<Database, DatabaseError> {
        Ok(Database::from_core(CoreDatabase::create(
            &OS_VFS,
            &self.path,
            self.cache_capacity,
        )?))
    }

    pub fn open(&self) -> Result<Database, DatabaseError> {
        Ok(Database::from_core(CoreDatabase::open(
            &OS_VFS,
            &self.path,
            self.cache_capacity,
        )?))
    }
}

pub struct Database {
    inner: CoreDatabase,
}

impl Database {
    pub fn builder(path: impl Into<PathBuf>) -> DatabaseBuilder {
        DatabaseBuilder::new(path)
    }

    pub fn create(path: impl Into<PathBuf>) -> Result<Self, DatabaseError> {
        DatabaseBuilder::new(path).create()
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self, DatabaseError> {
        DatabaseBuilder::new(path).open()
    }

    pub fn from_core(inner: CoreDatabase) -> Self {
        Self { inner }
    }

    pub fn as_core(&self) -> &CoreDatabase {
        &self.inner
    }

    pub fn as_core_mut(&mut self) -> &mut CoreDatabase {
        &mut self.inner
    }

    pub fn into_core(self) -> CoreDatabase {
        self.inner
    }

    pub fn checkpoint(&mut self) -> Result<(), DatabaseError> {
        self.inner.checkpoint(&OS_VFS)
    }

    pub fn checkpoint_with_vfs(&mut self, vfs: &dyn Vfs) -> Result<(), DatabaseError> {
        self.inner.checkpoint(vfs)
    }

    pub fn wal_stats(&mut self) -> Result<WalStats, DatabaseError> {
        self.inner.wal_stats()
    }

    pub fn checkpoint_if_needed(
        &mut self,
        threshold_bytes: u64,
    ) -> Result<CheckpointOutcome, DatabaseError> {
        self.inner.checkpoint_if_needed(&OS_VFS, threshold_bytes)
    }

    pub fn checkpoint_if_needed_with_vfs(
        &mut self,
        vfs: &dyn Vfs,
        threshold_bytes: u64,
    ) -> Result<CheckpointOutcome, DatabaseError> {
        self.inner.checkpoint_if_needed(vfs, threshold_bytes)
    }

    pub fn backup_to(&mut self, backup_path: impl AsRef<Path>) -> Result<(), DatabaseError> {
        self.inner.backup_to(&OS_VFS, backup_path.as_ref())
    }

    pub fn backup_to_with_vfs(
        &mut self,
        vfs: &dyn Vfs,
        backup_path: impl AsRef<Path>,
    ) -> Result<(), DatabaseError> {
        self.inner.backup_to(vfs, backup_path.as_ref())
    }

    pub fn restore_from(
        backup_path: impl AsRef<Path>,
        database_path: impl AsRef<Path>,
    ) -> Result<Self, DatabaseError> {
        Ok(Self::from_core(CoreDatabase::restore_from(
            &OS_VFS,
            backup_path.as_ref(),
            database_path.as_ref(),
            64,
        )?))
    }

    pub fn restore_from_with_vfs(
        vfs: &dyn Vfs,
        backup_path: impl AsRef<Path>,
        database_path: impl AsRef<Path>,
        cache_capacity: usize,
    ) -> Result<Self, DatabaseError> {
        Ok(Self::from_core(CoreDatabase::restore_from(
            vfs,
            backup_path.as_ref(),
            database_path.as_ref(),
            cache_capacity,
        )?))
    }

    pub fn begin_transaction(&mut self) -> DatabaseTransaction<'_> {
        self.inner.begin_transaction()
    }

    pub fn insert(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        self.inner.insert(rowid, values)
    }

    pub fn get(&mut self, rowid: i64) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.inner.get(rowid)
    }

    pub fn update(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        self.inner.update(rowid, values)
    }

    pub fn delete(&mut self, rowid: i64) -> Result<bool, DatabaseError> {
        self.inner.delete(rowid)
    }

    pub fn create_table(&mut self, name: &str, columns: &[&str]) -> Result<(), DatabaseError> {
        self.inner.create_table(name, columns)
    }

    pub fn table_columns(&mut self, name: &str) -> Result<Vec<String>, DatabaseError> {
        self.inner.table_columns(name)
    }

    pub fn scan_table(&mut self, name: &str) -> Result<Vec<(i64, Vec<u8>)>, DatabaseError> {
        self.inner.scan_table(name)
    }

    pub fn insert_into(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        self.inner.insert_into(table, rowid, values)
    }

    pub fn update_in(
        &mut self,
        table: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        self.inner.update_in(table, rowid, values)
    }

    pub fn delete_from(&mut self, table: &str, rowid: i64) -> Result<bool, DatabaseError> {
        self.inner.delete_from(table, rowid)
    }

    pub fn get_from(&mut self, table: &str, rowid: i64) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.inner.get_from(table, rowid)
    }

    pub fn next_rowid(&mut self, table: &str) -> Result<i64, DatabaseError> {
        self.inner.next_rowid(table)
    }

    pub fn create_index(&mut self, name: &str, columns: &[usize]) -> Result<(), DatabaseError> {
        self.inner.create_index(name, columns)
    }

    pub fn create_index_on(
        &mut self,
        name: &str,
        table: &str,
        columns: &[usize],
    ) -> Result<(), DatabaseError> {
        self.inner.create_index_on(name, table, columns)
    }

    pub fn find_index(
        &mut self,
        name: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.inner.find_index(name, values)
    }

    pub fn find_index_on(
        &mut self,
        name: &str,
        table: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.inner.find_index_on(name, table, values)
    }

    pub fn indexes(&mut self) -> Result<Vec<IndexInfo>, DatabaseError> {
        self.inner.indexes()
    }

    pub fn validate(&mut self) -> Result<TreeStats, DatabaseError> {
        self.inner.validate()
    }

    pub fn page_count(&self) -> u32 {
        self.inner.page_count()
    }

    #[cfg(feature = "cache")]
    pub fn into_cached<C: ferrite_cache::CacheClient + 'static>(
        self,
        client: C,
        namespace: impl Into<String>,
    ) -> ferrite_cache::CachedDatabase<C> {
        ferrite_cache::CachedDatabase::new(self.inner, client, namespace)
    }
}

#[cfg(feature = "sql")]
pub mod sql {
    pub use ferrite_vdbe::{
        AccessPath, ExecutionResult, OpCode, Plan, Predicate, SqlValue, VdbeError, compile,
        execute, execute_plan, plan,
    };

    pub fn execute_sql<D: ferrite_vdbe::SqlBackend>(
        database: &mut D,
        statement: &str,
    ) -> Result<ExecutionResult, VdbeError> {
        let program = compile(statement)?;
        execute(database, &program)
    }
}

#[cfg(feature = "cache")]
pub mod cache {
    pub use ferrite_cache::{
        CacheClient, CacheError, CacheState, CachedDatabase, CachedTransaction, InMemoryValkey,
        RedisValkey,
    };
}

#[cfg(feature = "sql")]
impl ferrite_vdbe::SqlBackend for Database {
    type Error = DatabaseError;
    type Transaction<'a> = DatabaseTransaction<'a>;

    fn table_columns(&mut self, table: &str) -> Result<Vec<String>, Self::Error> {
        self.inner.table_columns(table)
    }

    fn indexes(&mut self) -> Result<Vec<IndexInfo>, Self::Error> {
        self.inner.indexes()
    }

    fn next_rowid(&mut self, table: &str) -> Result<i64, Self::Error> {
        self.inner.next_rowid(table)
    }

    fn begin_transaction(&mut self) -> Self::Transaction<'_> {
        self.inner.begin_transaction()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEST_FILE: AtomicU64 = AtomicU64::new(1);

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "ferritedb-facade-{}-{}.fdb",
            std::process::id(),
            NEXT_TEST_FILE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn remove_database_files(path: &Path) {
        let _ = fs::remove_file(path);
        let mut wal_path = path.as_os_str().to_os_string();
        wal_path.push("-wal");
        let _ = fs::remove_file(PathBuf::from(wal_path));
    }

    #[test]
    fn builder_creates_reopens_and_checkpoints_a_database() {
        let path = test_path();
        let mut stage = "create";
        let result = (|| {
            let mut database = Database::builder(&path).cache_capacity(8).create()?;
            stage = "insert";
            database.insert(1, &[Value::Text("portable")])?;
            stage = "checkpoint";
            database.checkpoint()?;
            drop(database);

            stage = "reopen";
            let mut reopened = Database::open(&path)?;
            stage = "get";
            let bytes = reopened.get(1)?.expect("row persists after reopen");
            stage = "decode";
            let record = RecordRef::new(&bytes).expect("record decodes");
            assert_eq!(
                record.get(0).expect("read stored value"),
                Some(ValueRef::Text("portable"))
            );
            Ok::<(), DatabaseError>(())
        })();
        remove_database_files(&path);
        assert!(result.is_ok(), "facade failed during {stage}: {result:?}");
    }

    #[cfg(feature = "sql")]
    #[test]
    fn sql_feature_executes_statements_through_the_facade() {
        let path = test_path();
        let result = (|| {
            let mut database = Database::create(&path)?;
            sql::execute_sql(
                &mut database,
                "CREATE TABLE products (name TEXT, qty INTEGER)",
            )
            .expect("create table");
            sql::execute_sql(&mut database, "INSERT INTO products VALUES ('pear', 4)")
                .expect("insert row");
            let rows = sql::execute_sql(
                &mut database,
                "SELECT name, qty FROM products WHERE name = 'pear'",
            )
            .expect("select row");
            assert!(matches!(
                rows,
                sql::ExecutionResult::Rows { ref rows, .. }
                    if rows == &[vec![sql::SqlValue::Text("pear".to_owned()), sql::SqlValue::Integer(4)]]
            ));
            Ok::<(), DatabaseError>(())
        })();
        remove_database_files(&path);
        result.expect("SQL facade round trip succeeds");
    }

    #[cfg(feature = "cache")]
    #[test]
    fn cache_feature_runs_sql_on_the_facade_database() {
        let path = test_path();
        let result = (|| {
            let database = Database::create(&path)?;
            let mut cached = database.into_cached(cache::InMemoryValkey::default(), "facade-test");
            sql::execute_sql(&mut cached, "CREATE TABLE products (name TEXT)")?;
            sql::execute_sql(&mut cached, "INSERT INTO products VALUES ('pear')")?;
            let rows =
                sql::execute_sql(&mut cached, "SELECT name FROM products WHERE name = 'pear'")?;
            assert!(matches!(
                rows,
                sql::ExecutionResult::Rows { ref rows, .. }
                    if rows == &[vec![sql::SqlValue::Text("pear".to_owned())]]
            ));
            Ok::<(), Box<dyn std::error::Error>>(())
        })();
        remove_database_files(&path);
        result.expect("cached SQL facade round trip succeeds");
    }
}
