use ferrite_btree::{
    BtreeError, PageTransaction, RecordRef, TableBtree, TreeStats, Value, ValueRef,
    index::IndexBtree,
};
pub use ferrite_pager::{CheckpointOutcome, WalStats};
use ferrite_pager::{Page, PageNumber, Pager, PagerError, PagerTransaction};
use ferrite_vfs::Vfs;
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("pager error: {0}")]
    Pager(#[from] PagerError),
    #[error("B-tree error: {0}")]
    Btree(#[from] BtreeError),
    #[error("database catalog is corrupt")]
    CorruptCatalog,
    #[error("database table metadata is missing")]
    MissingTableMetadata,
    #[error("table does not exist: {0}")]
    TableNotFound(String),
    #[error("table already exists: {0}")]
    TableAlreadyExists(String),
    #[error("table or column name is invalid: {0}")]
    InvalidSchemaName(String),
    #[error("table must have at least one column")]
    EmptyTableSchema,
    #[error("index name is invalid: {0}")]
    InvalidIndexName(String),
    #[error("index name already exists: {0}")]
    IndexAlreadyExists(String),
    #[error("index does not exist: {0}")]
    IndexNotFound(String),
    #[error("index columns must be nonempty and unique")]
    InvalidIndexColumns,
    #[error("index expects {expected} key fields, got {actual}")]
    InvalidIndexKey { expected: usize, actual: usize },
    #[error("record {rowid} has no indexed column {column}")]
    MissingIndexColumn { rowid: i64, column: usize },
    #[error("record with rowid {0} does not exist")]
    RowNotFound(i64),
    #[error("transaction was aborted after an earlier operation failed")]
    TransactionAborted,
    #[error("index entry is missing for rowid {0}")]
    MissingIndexEntry(i64),
    #[error("commit hook failed before the WAL commit: {0}")]
    CommitHookBefore(String),
    #[error("WAL commit is durable, but its commit hook failed: {0}")]
    CommitHookAfter(String),
    #[error("commit hook failed during rollback: {0}")]
    CommitHookRollback(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommitChange {
    pub table_name: String,
    pub rowid: i64,
    pub record: Option<Vec<u8>>,
}

pub trait CommitHook {
    fn before_commit(&mut self, _changes: &[CommitChange]) -> Result<(), String> {
        Ok(())
    }

    fn after_commit(&mut self, changes: &[CommitChange]) -> Result<(), String>;

    fn after_rollback(&mut self, _changes: &[CommitChange]) -> Result<(), String> {
        Ok(())
    }
}

pub struct Database {
    pager: Pager,
}

impl Database {
    pub fn create(
        vfs: &dyn Vfs,
        path: &Path,
        cache_capacity: usize,
    ) -> Result<Self, DatabaseError> {
        let mut pager = Pager::create(vfs, path, cache_capacity)?;
        let mut transaction = DatabasePageTransaction::new(pager.begin_write());
        let table_root = transaction.allocate_page()?;
        TableBtree::initialize_at(&mut transaction, table_root)?;
        let master_root = PageNumber::new(1)?;
        TableBtree::initialize_at(&mut transaction, master_root)?;
        insert_table_metadata(&mut transaction, table_root)?;
        transaction.commit()?;
        Ok(Self { pager })
    }

    pub fn open(vfs: &dyn Vfs, path: &Path, cache_capacity: usize) -> Result<Self, DatabaseError> {
        let mut pager = Pager::open(vfs, path, cache_capacity)?;
        let mut transaction = DatabasePageTransaction::new(pager.begin_write());
        match read_catalog(&mut transaction)? {
            Some(catalog) => {
                validate_catalog_roots(&mut transaction, &catalog)?;
            }
            None => {
                migrate_legacy_database(&mut transaction)?;
                transaction.commit()?;
                return Ok(Self { pager });
            }
        }
        transaction.commit()?;
        Ok(Self { pager })
    }

    pub fn insert(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.insert(rowid, values)?;
        transaction.commit()
    }

    pub fn create_table(&mut self, name: &str, columns: &[&str]) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.create_table(name, columns)?;
        transaction.commit()
    }

    pub fn table_columns(&mut self, name: &str) -> Result<Vec<String>, DatabaseError> {
        self.begin_transaction().table_columns(name)
    }

    pub fn scan_table(&mut self, name: &str) -> Result<Vec<(i64, Vec<u8>)>, DatabaseError> {
        self.begin_transaction().scan_table(name)
    }

    pub fn insert_into(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.insert_into(table_name, rowid, values)?;
        transaction.commit()
    }

    pub fn update_in(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.update_in(table_name, rowid, values)?;
        transaction.commit()
    }

    pub fn delete_from(&mut self, table_name: &str, rowid: i64) -> Result<bool, DatabaseError> {
        let mut transaction = self.begin_transaction();
        let removed = transaction.delete_from(table_name, rowid)?;
        transaction.commit()?;
        Ok(removed)
    }

    pub fn get_from(
        &mut self,
        table_name: &str,
        rowid: i64,
    ) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.begin_transaction().get_from(table_name, rowid)
    }

    pub fn next_rowid(&mut self, table_name: &str) -> Result<i64, DatabaseError> {
        self.begin_transaction().next_rowid(table_name)
    }

    pub fn update(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.update(rowid, values)?;
        transaction.commit()
    }

    pub fn delete(&mut self, rowid: i64) -> Result<bool, DatabaseError> {
        let mut transaction = self.begin_transaction();
        let removed = transaction.delete(rowid)?;
        transaction.commit()?;
        Ok(removed)
    }

    pub fn create_index(&mut self, name: &str, columns: &[usize]) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.create_index(name, columns)?;
        transaction.commit()
    }

    pub fn create_index_on(
        &mut self,
        name: &str,
        table_name: &str,
        columns: &[usize],
    ) -> Result<(), DatabaseError> {
        let mut transaction = self.begin_transaction();
        transaction.create_index_on(name, table_name, columns)?;
        transaction.commit()
    }

    pub fn find_index(
        &mut self,
        name: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.begin_transaction().find_index(name, values)
    }

    pub fn find_index_on(
        &mut self,
        name: &str,
        table_name: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.begin_transaction()
            .find_index_on(name, table_name, values)
    }

    pub fn indexes(&mut self) -> Result<Vec<IndexInfo>, DatabaseError> {
        self.begin_transaction().indexes()
    }

    pub fn get(&mut self, rowid: i64) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.begin_transaction().get(rowid)
    }

    pub fn validate(&mut self) -> Result<TreeStats, DatabaseError> {
        self.begin_transaction().validate()
    }

    pub fn begin_transaction(&mut self) -> DatabaseTransaction<'_> {
        DatabaseTransaction {
            transaction: DatabasePageTransaction::new(self.pager.begin_write()),
            aborted: false,
            changes: BTreeMap::new(),
            commit_hook: None,
        }
    }

    pub fn checkpoint(&mut self, vfs: &dyn Vfs) -> Result<(), DatabaseError> {
        self.pager.checkpoint(vfs)?;
        Ok(())
    }

    pub fn wal_stats(&mut self) -> Result<WalStats, DatabaseError> {
        Ok(self.pager.wal_stats()?)
    }

    pub fn checkpoint_if_needed(
        &mut self,
        vfs: &dyn Vfs,
        threshold_bytes: u64,
    ) -> Result<CheckpointOutcome, DatabaseError> {
        Ok(self.pager.checkpoint_if_needed(vfs, threshold_bytes)?)
    }

    pub fn backup_to(&mut self, vfs: &dyn Vfs, backup_path: &Path) -> Result<(), DatabaseError> {
        self.pager.backup_to(vfs, backup_path)?;
        Ok(())
    }

    pub fn restore_from(
        vfs: &dyn Vfs,
        backup_path: &Path,
        database_path: &Path,
        cache_capacity: usize,
    ) -> Result<Self, DatabaseError> {
        Pager::restore_from(vfs, backup_path, database_path, cache_capacity)?;
        Self::open(vfs, database_path, cache_capacity)
    }

    pub fn page_count(&self) -> u32 {
        self.pager.page_count()
    }
}

pub struct DatabaseTransaction<'a> {
    transaction: DatabasePageTransaction<'a>,
    aborted: bool,
    changes: BTreeMap<(String, i64), Option<Vec<u8>>>,
    commit_hook: Option<Box<dyn CommitHook>>,
}

impl DatabaseTransaction<'_> {
    pub fn insert(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        self.insert_into(TABLE_NAME, rowid, values)
    }

    pub fn create_table(&mut self, name: &str, columns: &[&str]) -> Result<(), DatabaseError> {
        self.apply(|transaction| create_table(transaction, name, columns))
    }

    pub fn table_columns(&mut self, name: &str) -> Result<Vec<String>, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            Ok(find_table(&catalog, name)?.columns.clone())
        })
    }

    pub fn scan_table(&mut self, name: &str) -> Result<Vec<(i64, Vec<u8>)>, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            let table = find_table(&catalog, name)?;
            Ok(TableBtree::open_at(transaction, table.root)?.scan(transaction)?)
        })
    }

    pub fn insert_into(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        let record = self.apply(|_| Ok(ferrite_btree::encode_record(values)?))?;
        self.apply(|transaction| insert_record(transaction, table_name, rowid, values))?;
        self.changes
            .insert((table_name.to_ascii_lowercase(), rowid), Some(record));
        Ok(())
    }

    pub fn update(&mut self, rowid: i64, values: &[Value<'_>]) -> Result<(), DatabaseError> {
        self.update_in(TABLE_NAME, rowid, values)
    }

    pub fn update_in(
        &mut self,
        table_name: &str,
        rowid: i64,
        values: &[Value<'_>],
    ) -> Result<(), DatabaseError> {
        let record = self.apply(|_| Ok(ferrite_btree::encode_record(values)?))?;
        self.apply(|transaction| update_record(transaction, table_name, rowid, values))?;
        self.changes
            .insert((table_name.to_ascii_lowercase(), rowid), Some(record));
        Ok(())
    }

    pub fn delete(&mut self, rowid: i64) -> Result<bool, DatabaseError> {
        self.delete_from(TABLE_NAME, rowid)
    }

    pub fn delete_from(&mut self, table_name: &str, rowid: i64) -> Result<bool, DatabaseError> {
        let removed = self.apply(|transaction| delete_record(transaction, table_name, rowid))?;
        if removed {
            self.changes
                .insert((table_name.to_ascii_lowercase(), rowid), None);
        }
        Ok(removed)
    }

    pub fn create_index(&mut self, name: &str, columns: &[usize]) -> Result<(), DatabaseError> {
        self.create_index_on(name, TABLE_NAME, columns)
    }

    pub fn create_index_on(
        &mut self,
        name: &str,
        table_name: &str,
        columns: &[usize],
    ) -> Result<(), DatabaseError> {
        self.apply(|transaction| create_index(transaction, name, table_name, columns))
    }

    pub fn find_index(
        &mut self,
        name: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.find_index_on(name, TABLE_NAME, values)
    }

    pub fn find_index_on(
        &mut self,
        name: &str,
        table_name: &str,
        values: &[Value<'_>],
    ) -> Result<Vec<i64>, DatabaseError> {
        self.apply(|transaction| find_index(transaction, name, table_name, values))
    }

    pub fn indexes(&mut self) -> Result<Vec<IndexInfo>, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            Ok(catalog
                .indexes
                .iter()
                .map(|index| IndexInfo {
                    name: index.name.clone(),
                    table_name: index.table_name.clone(),
                    columns: index.columns.clone(),
                })
                .collect())
        })
    }

    pub fn next_rowid(&mut self, table_name: &str) -> Result<i64, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            let table = find_table(&catalog, table_name)?;
            TableBtree::open_at(transaction, table.root)?
                .scan(transaction)?
                .iter()
                .map(|(rowid, _)| *rowid)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(DatabaseError::CorruptCatalog)
        })
    }

    pub fn get(&mut self, rowid: i64) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.get_from(TABLE_NAME, rowid)
    }

    pub fn get_from(
        &mut self,
        table_name: &str,
        rowid: i64,
    ) -> Result<Option<Vec<u8>>, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            let table = find_table(&catalog, table_name)?;
            let tree = TableBtree::open_at(transaction, table.root)?;
            Ok(tree.get(transaction, rowid)?)
        })
    }

    pub fn validate(&mut self) -> Result<TreeStats, DatabaseError> {
        self.apply(|transaction| {
            let catalog = require_catalog(transaction)?;
            let table = find_table(&catalog, TABLE_NAME)?;
            let tree = TableBtree::open_at(transaction, table.root)?;
            Ok(tree.validate(transaction)?)
        })
    }

    pub fn set_commit_hook(&mut self, hook: Box<dyn CommitHook>) {
        self.commit_hook = Some(hook);
    }

    pub fn rollback(mut self) -> Result<(), DatabaseError> {
        let changes = self.commit_changes();
        if let Some(hook) = &mut self.commit_hook {
            hook.after_rollback(&changes)
                .map_err(DatabaseError::CommitHookRollback)?;
        }
        Ok(())
    }

    pub fn commit(mut self) -> Result<(), DatabaseError> {
        if self.aborted {
            return Err(DatabaseError::TransactionAborted);
        }
        let changes = self.commit_changes();
        if let Some(hook) = &mut self.commit_hook
            && let Err(error) = hook.before_commit(&changes)
        {
            let rollback_error = hook.after_rollback(&changes).err();
            return match rollback_error {
                Some(rollback_error) => Err(DatabaseError::CommitHookBefore(format!(
                    "{error}; rollback hook also failed: {rollback_error}"
                ))),
                None => Err(DatabaseError::CommitHookBefore(error)),
            };
        }
        if let Err(error) = self.transaction.commit() {
            if !matches!(
                &error,
                DatabaseError::Pager(PagerError::CommitDurable(_) | PagerError::CommitUncertain(_))
            ) && let Some(hook) = &mut self.commit_hook
                && let Err(rollback_error) = hook.after_rollback(&changes)
            {
                return Err(DatabaseError::CommitHookRollback(format!(
                    "{error}; rollback hook also failed: {rollback_error}"
                )));
            }
            return Err(error);
        }
        if let Some(hook) = &mut self.commit_hook {
            hook.after_commit(&changes)
                .map_err(DatabaseError::CommitHookAfter)?;
        }
        Ok(())
    }

    fn commit_changes(&self) -> Vec<CommitChange> {
        self.changes
            .iter()
            .map(|((table_name, rowid), record)| CommitChange {
                table_name: table_name.clone(),
                rowid: *rowid,
                record: record.clone(),
            })
            .collect()
    }

    fn apply<T>(
        &mut self,
        operation: impl FnOnce(&mut DatabasePageTransaction) -> Result<T, DatabaseError>,
    ) -> Result<T, DatabaseError> {
        if self.aborted {
            return Err(DatabaseError::TransactionAborted);
        }
        match operation(&mut self.transaction) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.aborted = true;
                Err(error)
            }
        }
    }
}

const MASTER_ROOT_PAGE: u32 = 1;
const TABLE_NAME: &str = "records";
const TABLE_SQL: &str =
    "CREATE TABLE \"records\" (\"rowid\" INTEGER PRIMARY KEY, \"payload\" BLOB)";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexInfo {
    pub name: String,
    pub table_name: String,
    pub columns: Vec<usize>,
}

#[derive(Clone)]
struct TableDefinition {
    name: String,
    root: PageNumber,
    columns: Vec<String>,
}

#[derive(Clone)]
struct IndexDefinition {
    name: String,
    table_name: String,
    root: PageNumber,
    columns: Vec<usize>,
}

struct Catalog {
    tables: Vec<TableDefinition>,
    indexes: Vec<IndexDefinition>,
    next_rowid: i64,
}

fn master_root() -> Result<PageNumber, DatabaseError> {
    Ok(PageNumber::new(MASTER_ROOT_PAGE)?)
}

fn insert_table_metadata(
    transaction: &mut DatabasePageTransaction<'_>,
    table_root: PageNumber,
) -> Result<(), DatabaseError> {
    insert_catalog_metadata(
        transaction,
        1,
        "table",
        TABLE_NAME,
        TABLE_NAME,
        table_root,
        TABLE_SQL,
    )
}

fn insert_catalog_metadata(
    transaction: &mut DatabasePageTransaction<'_>,
    rowid: i64,
    record_type: &str,
    name: &str,
    table_name: &str,
    root: PageNumber,
    sql: &str,
) -> Result<(), DatabaseError> {
    let master = TableBtree::open_at(transaction, master_root()?)?;
    let record = ferrite_btree::encode_record(&[
        Value::Text(record_type),
        Value::Text(name),
        Value::Text(table_name),
        Value::Integer(i64::from(root.get())),
        Value::Text(sql),
    ])?;
    master.insert(transaction, rowid, &record)?;
    Ok(())
}

fn read_catalog(
    transaction: &mut DatabasePageTransaction<'_>,
) -> Result<Option<Catalog>, DatabaseError> {
    let master = TableBtree::open_at(transaction, master_root()?)?;
    let rows = master.scan(transaction)?;
    let mut tables = Vec::new();
    let mut indexes = Vec::new();
    let mut raw_indexes = Vec::new();
    let mut maximum_rowid = 0_i64;

    for (rowid, bytes) in rows {
        maximum_rowid = maximum_rowid.max(rowid);
        let record = RecordRef::new(&bytes)?;
        let record_type = optional_text(&record, 0)?;
        let name = optional_text(&record, 1)?;
        let table_name = optional_text(&record, 2)?;
        if record_type == Some("table") {
            let name = name.ok_or(DatabaseError::CorruptCatalog)?;
            if table_name != Some(name) || !valid_schema_name(name) {
                return Err(DatabaseError::CorruptCatalog);
            }
            if tables
                .iter()
                .any(|table: &TableDefinition| table.name.eq_ignore_ascii_case(name))
            {
                return Err(DatabaseError::CorruptCatalog);
            }
            let root_page = required_integer(&record, 3)?;
            let sql = required_text(&record, 4)?;
            if root_page <= 0 {
                return Err(DatabaseError::CorruptCatalog);
            }
            let columns = parse_table_columns(name, sql)?;
            tables.push(TableDefinition {
                name: name.to_owned(),
                root: catalog_page_number(root_page, transaction.page_count())?,
                columns,
            });
        } else if record_type == Some("index") {
            let name = name.ok_or(DatabaseError::CorruptCatalog)?;
            let table_name = table_name.ok_or(DatabaseError::CorruptCatalog)?;
            if !valid_schema_name(name) || !valid_schema_name(table_name) {
                return Err(DatabaseError::CorruptCatalog);
            }
            let root_page = required_integer(&record, 3)?;
            let root = catalog_page_number(root_page, transaction.page_count())?;
            let sql = required_text(&record, 4)?;
            raw_indexes.push((name.to_owned(), table_name.to_owned(), root, sql.to_owned()));
        }
    }

    if tables.is_empty() {
        return Ok(None);
    }

    for (name, table_name, root, sql) in raw_indexes {
        let table =
            find_table_in(&tables, &table_name).map_err(|_| DatabaseError::CorruptCatalog)?;
        let columns = parse_index_columns(&name, &table_name, &sql, &table.columns)?;
        if indexes
            .iter()
            .any(|index: &IndexDefinition| index.name.eq_ignore_ascii_case(&name))
        {
            return Err(DatabaseError::CorruptCatalog);
        }
        indexes.push(IndexDefinition {
            name,
            table_name,
            root,
            columns,
        });
    }

    let next_rowid = maximum_rowid
        .checked_add(1)
        .ok_or(DatabaseError::CorruptCatalog)?;
    Ok(Some(Catalog {
        tables,
        indexes,
        next_rowid,
    }))
}

fn optional_text<'a>(
    record: &RecordRef<'a>,
    column: usize,
) -> Result<Option<&'a str>, DatabaseError> {
    match record.get(column)? {
        None | Some(ValueRef::Null) => Ok(None),
        Some(ValueRef::Text(value)) => Ok(Some(value)),
        _ => Ok(None),
    }
}

fn required_text<'a>(record: &RecordRef<'a>, column: usize) -> Result<&'a str, DatabaseError> {
    optional_text(record, column)?.ok_or(DatabaseError::CorruptCatalog)
}

fn required_integer(record: &RecordRef<'_>, column: usize) -> Result<i64, DatabaseError> {
    match record.get(column)? {
        Some(ValueRef::Integer(value)) => Ok(value),
        _ => Err(DatabaseError::CorruptCatalog),
    }
}

fn catalog_page_number(value: i64, page_count: u32) -> Result<PageNumber, DatabaseError> {
    let page_number = u32::try_from(value).map_err(|_| DatabaseError::CorruptCatalog)?;
    if page_number > page_count {
        return Err(DatabaseError::CorruptCatalog);
    }
    Ok(PageNumber::new(page_number)?)
}

fn validate_catalog_roots(
    transaction: &mut DatabasePageTransaction<'_>,
    catalog: &Catalog,
) -> Result<(), DatabaseError> {
    for table in &catalog.tables {
        let tree = TableBtree::open_at(transaction, table.root)?;
        tree.validate(transaction)?;
    }
    for index in &catalog.indexes {
        let tree = IndexBtree::open(transaction, index.root)?;
        tree.validate(transaction)?;
    }
    Ok(())
}

fn require_catalog(
    transaction: &mut DatabasePageTransaction<'_>,
) -> Result<Catalog, DatabaseError> {
    read_catalog(transaction)?.ok_or(DatabaseError::MissingTableMetadata)
}

fn migrate_legacy_database(
    transaction: &mut DatabasePageTransaction<'_>,
) -> Result<(), DatabaseError> {
    let old_root = master_root()?;
    let old_table = TableBtree::open_at(transaction, old_root)?;
    let rows = old_table.scan(transaction)?;
    let table_root = transaction.allocate_page()?;
    let table = TableBtree::initialize_at(transaction, table_root)?;
    for (rowid, record) in rows {
        table.insert(transaction, rowid, &record)?;
    }
    TableBtree::initialize_at(transaction, old_root)?;
    insert_table_metadata(transaction, table_root)?;
    Ok(())
}

fn valid_schema_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn find_table_in<'a>(
    tables: &'a [TableDefinition],
    name: &str,
) -> Result<&'a TableDefinition, DatabaseError> {
    tables
        .iter()
        .find(|table| table.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| DatabaseError::TableNotFound(name.to_owned()))
}

fn find_table<'a>(catalog: &'a Catalog, name: &str) -> Result<&'a TableDefinition, DatabaseError> {
    find_table_in(&catalog.tables, name)
}

fn create_table_sql(name: &str, columns: &[&str]) -> String {
    let columns = columns
        .iter()
        .map(|column| format!("\"{column}\" BLOB"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("CREATE TABLE \"{name}\" ({columns})")
}

fn parse_table_columns(name: &str, sql: &str) -> Result<Vec<String>, DatabaseError> {
    if name.eq_ignore_ascii_case(TABLE_NAME) && sql == TABLE_SQL {
        return Ok(Vec::new());
    }
    let prefix = format!("CREATE TABLE \"{name}\" (");
    let body = sql
        .strip_prefix(&prefix)
        .and_then(|body| body.strip_suffix(')'))
        .ok_or(DatabaseError::CorruptCatalog)?;
    let mut columns = Vec::new();
    for definition in body.split(", ") {
        let column = definition
            .strip_prefix('"')
            .and_then(|definition| definition.strip_suffix("\" BLOB"))
            .ok_or(DatabaseError::CorruptCatalog)?;
        if !valid_schema_name(column)
            || columns
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(column))
        {
            return Err(DatabaseError::CorruptCatalog);
        }
        columns.push(column.to_owned());
    }
    if columns.is_empty() {
        return Err(DatabaseError::CorruptCatalog);
    }
    Ok(columns)
}

fn validate_index_columns(columns: &[usize]) -> Result<(), DatabaseError> {
    if columns.is_empty() {
        return Err(DatabaseError::InvalidIndexColumns);
    }
    let mut seen = std::collections::HashSet::new();
    seen.try_reserve(columns.len())
        .map_err(|_| DatabaseError::InvalidIndexColumns)?;
    if columns.iter().any(|column| !seen.insert(*column)) {
        return Err(DatabaseError::InvalidIndexColumns);
    }
    Ok(())
}

fn make_index_sql(name: &str, table: &TableDefinition, columns: &[usize]) -> String {
    let columns = columns
        .iter()
        .map(|column| {
            table
                .columns
                .get(*column)
                .map(|name| format!("\"{name}\""))
                .unwrap_or_else(|| format!("\"column_{column}\""))
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("CREATE INDEX \"{name}\" ON \"{}\" ({columns})", table.name)
}

fn parse_index_columns(
    name: &str,
    table_name: &str,
    sql: &str,
    table_columns: &[String],
) -> Result<Vec<usize>, DatabaseError> {
    let prefix = format!("CREATE INDEX \"{name}\" ON \"{table_name}\" (");
    let body = sql
        .strip_prefix(&prefix)
        .and_then(|body| body.strip_suffix(')'))
        .ok_or(DatabaseError::CorruptCatalog)?;
    let mut columns = Vec::new();
    for column in body.split(", ") {
        let column = column
            .strip_prefix('"')
            .and_then(|column| column.strip_suffix('"'))
            .ok_or(DatabaseError::CorruptCatalog)?;
        let position = if table_columns.is_empty() {
            column
                .strip_prefix("column_")
                .and_then(|column| column.parse::<usize>().ok())
                .ok_or(DatabaseError::CorruptCatalog)?
        } else {
            table_columns
                .iter()
                .position(|candidate| candidate.eq_ignore_ascii_case(column))
                .ok_or(DatabaseError::CorruptCatalog)?
        };
        columns.push(position);
    }
    validate_index_columns(&columns).map_err(|_| DatabaseError::CorruptCatalog)?;
    Ok(columns)
}

fn find_index_definition<'a>(
    catalog: &'a Catalog,
    name: &str,
    table_name: &str,
) -> Result<&'a IndexDefinition, DatabaseError> {
    catalog
        .indexes
        .iter()
        .find(|index| {
            index.name.eq_ignore_ascii_case(name)
                && index.table_name.eq_ignore_ascii_case(table_name)
        })
        .ok_or_else(|| DatabaseError::IndexNotFound(name.to_owned()))
}

fn index_values<'a>(
    record_bytes: &'a [u8],
    rowid: i64,
    columns: &[usize],
) -> Result<Vec<Value<'a>>, DatabaseError> {
    let record = RecordRef::new(record_bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(columns.len())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for column in columns {
        let value = record
            .get(*column)?
            .ok_or(DatabaseError::MissingIndexColumn {
                rowid,
                column: *column,
            })?;
        values.push(match value {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => Value::Integer(value),
            ValueRef::Real(value) => Value::Real(value),
            ValueRef::Text(value) => Value::Text(value),
            ValueRef::Blob(value) => Value::Blob(value),
        });
    }
    Ok(values)
}

fn create_table(
    transaction: &mut DatabasePageTransaction<'_>,
    name: &str,
    columns: &[&str],
) -> Result<(), DatabaseError> {
    if !valid_schema_name(name) {
        return Err(DatabaseError::InvalidSchemaName(name.to_owned()));
    }
    if columns.is_empty() {
        return Err(DatabaseError::EmptyTableSchema);
    }
    let mut seen = std::collections::HashSet::new();
    seen.try_reserve(columns.len())
        .map_err(|_| BtreeError::RecordAllocation)?;
    for column in columns {
        if !valid_schema_name(column) || !seen.insert(column.to_ascii_lowercase()) {
            return Err(DatabaseError::InvalidSchemaName((*column).to_owned()));
        }
    }
    let catalog = require_catalog(transaction)?;
    if let Some(existing) = catalog
        .tables
        .iter()
        .find(|table| table.name.eq_ignore_ascii_case(name))
    {
        if existing.columns.is_empty()
            && existing.name.eq_ignore_ascii_case(TABLE_NAME)
            && !catalog
                .indexes
                .iter()
                .any(|index| index.table_name.eq_ignore_ascii_case(TABLE_NAME))
            && TableBtree::open_at(transaction, existing.root)?
                .scan(transaction)?
                .is_empty()
        {
            let sql = create_table_sql(name, columns);
            let master = TableBtree::open_at(transaction, master_root()?)?;
            if master.remove(transaction, 1)?.is_none() {
                return Err(DatabaseError::CorruptCatalog);
            }
            return insert_catalog_metadata(
                transaction,
                1,
                "table",
                name,
                name,
                existing.root,
                &sql,
            );
        }
        return Err(DatabaseError::TableAlreadyExists(name.to_owned()));
    }
    let root = transaction.allocate_page()?;
    TableBtree::initialize_at(transaction, root)?;
    let sql = create_table_sql(name, columns);
    insert_catalog_metadata(
        transaction,
        catalog.next_rowid,
        "table",
        name,
        name,
        root,
        &sql,
    )
}

fn insert_record(
    transaction: &mut DatabasePageTransaction<'_>,
    table_name: &str,
    rowid: i64,
    values: &[Value<'_>],
) -> Result<(), DatabaseError> {
    let catalog = require_catalog(transaction)?;
    let record = ferrite_btree::encode_record(values)?;
    let table_definition = find_table(&catalog, table_name)?;
    let table = TableBtree::open_at(transaction, table_definition.root)?;
    table.insert(transaction, rowid, &record)?;
    for definition in &catalog.indexes {
        if !definition.table_name.eq_ignore_ascii_case(table_name) {
            continue;
        }
        let key = index_values(&record, rowid, &definition.columns)?;
        let index = IndexBtree::open(transaction, definition.root)?;
        index.insert(transaction, &key, rowid)?;
    }
    Ok(())
}

fn update_record(
    transaction: &mut DatabasePageTransaction<'_>,
    table_name: &str,
    rowid: i64,
    values: &[Value<'_>],
) -> Result<(), DatabaseError> {
    let catalog = require_catalog(transaction)?;
    let table_definition = find_table(&catalog, table_name)?;
    let table = TableBtree::open_at(transaction, table_definition.root)?;
    let previous = table
        .get(transaction, rowid)?
        .ok_or(DatabaseError::RowNotFound(rowid))?;
    let updated = ferrite_btree::encode_record(values)?;
    for definition in &catalog.indexes {
        if !definition.table_name.eq_ignore_ascii_case(table_name) {
            continue;
        }
        let old_key = index_values(&previous, rowid, &definition.columns)?;
        let index = IndexBtree::open(transaction, definition.root)?;
        if !index.remove(transaction, &old_key, rowid)? {
            return Err(DatabaseError::MissingIndexEntry(rowid));
        }
    }
    if table.remove(transaction, rowid)?.is_none() {
        return Err(DatabaseError::RowNotFound(rowid));
    }
    table.insert(transaction, rowid, &updated)?;
    for definition in &catalog.indexes {
        if !definition.table_name.eq_ignore_ascii_case(table_name) {
            continue;
        }
        let new_key = index_values(&updated, rowid, &definition.columns)?;
        let index = IndexBtree::open(transaction, definition.root)?;
        index.insert(transaction, &new_key, rowid)?;
    }
    Ok(())
}

fn delete_record(
    transaction: &mut DatabasePageTransaction<'_>,
    table_name: &str,
    rowid: i64,
) -> Result<bool, DatabaseError> {
    let catalog = require_catalog(transaction)?;
    let table_definition = find_table(&catalog, table_name)?;
    let table = TableBtree::open_at(transaction, table_definition.root)?;
    let Some(previous) = table.get(transaction, rowid)? else {
        return Ok(false);
    };
    for definition in &catalog.indexes {
        if !definition.table_name.eq_ignore_ascii_case(table_name) {
            continue;
        }
        let old_key = index_values(&previous, rowid, &definition.columns)?;
        let index = IndexBtree::open(transaction, definition.root)?;
        if !index.remove(transaction, &old_key, rowid)? {
            return Err(DatabaseError::MissingIndexEntry(rowid));
        }
    }
    if table.remove(transaction, rowid)?.is_none() {
        return Err(DatabaseError::RowNotFound(rowid));
    }
    Ok(true)
}

fn create_index(
    transaction: &mut DatabasePageTransaction<'_>,
    name: &str,
    table_name: &str,
    columns: &[usize],
) -> Result<(), DatabaseError> {
    if !valid_schema_name(name) {
        return Err(DatabaseError::InvalidIndexName(name.to_owned()));
    }
    validate_index_columns(columns)?;
    let catalog = require_catalog(transaction)?;
    let table_definition = find_table(&catalog, table_name)?;
    if catalog
        .indexes
        .iter()
        .any(|index| index.name.eq_ignore_ascii_case(name))
    {
        return Err(DatabaseError::IndexAlreadyExists(name.to_owned()));
    }
    let table = TableBtree::open_at(transaction, table_definition.root)?;
    let rows = table.scan(transaction)?;
    let root = transaction.allocate_page()?;
    let index = IndexBtree::initialize(transaction, root)?;
    for (rowid, record) in rows {
        let key = index_values(&record, rowid, columns)?;
        index.insert(transaction, &key, rowid)?;
    }
    let sql = make_index_sql(name, table_definition, columns);
    insert_catalog_metadata(
        transaction,
        catalog.next_rowid,
        "index",
        name,
        &table_definition.name,
        root,
        &sql,
    )
}

fn find_index(
    transaction: &mut DatabasePageTransaction<'_>,
    name: &str,
    table_name: &str,
    values: &[Value<'_>],
) -> Result<Vec<i64>, DatabaseError> {
    let catalog = require_catalog(transaction)?;
    let definition = find_index_definition(&catalog, name, table_name)?;
    if values.len() != definition.columns.len() {
        return Err(DatabaseError::InvalidIndexKey {
            expected: definition.columns.len(),
            actual: values.len(),
        });
    }
    let index = IndexBtree::open(transaction, definition.root)?;
    Ok(index.find(transaction, values)?)
}

struct DatabasePageTransaction<'a> {
    transaction: PagerTransaction<'a>,
}

impl<'a> DatabasePageTransaction<'a> {
    fn new(transaction: PagerTransaction<'a>) -> Self {
        Self { transaction }
    }

    fn commit(self) -> Result<(), DatabaseError> {
        self.transaction.commit()?;
        Ok(())
    }
}

impl PageTransaction for DatabasePageTransaction<'_> {
    fn page_count(&self) -> u32 {
        self.transaction.database_size()
    }

    fn allocate_page(&mut self) -> Result<PageNumber, BtreeError> {
        self.transaction
            .allocate_page()
            .map_err(|error| BtreeError::Transaction(error.to_string()))
    }

    fn read_page(&mut self, page_number: PageNumber) -> Result<Page, BtreeError> {
        self.transaction
            .read_page(page_number)
            .map_err(|error| BtreeError::Transaction(error.to_string()))
    }

    fn write_page(&mut self, page_number: PageNumber, page: &Page) -> Result<(), BtreeError> {
        self.transaction
            .write_page(page_number, page)
            .map_err(|error| BtreeError::Transaction(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrite_btree::{RecordRef, ValueRef};
    use ferrite_vfs::SimVfs;
    use std::path::Path;

    #[test]
    fn database_inserts_reads_reopens_and_checkpoints_records() {
        let vfs = SimVfs::new();
        let path = Path::new("database-api.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");

        database
            .insert(
                17,
                &[
                    Value::Text("Ferrite"),
                    Value::Integer(2026),
                    Value::Blob(&[0x01, 0x02, 0x03]),
                ],
            )
            .expect("insert row");

        let record = database.get(17).expect("query row").expect("row exists");
        let decoded = RecordRef::new(&record).expect("decode stored record");
        assert_eq!(
            decoded.get(0).expect("read text"),
            Some(ValueRef::Text("Ferrite"))
        );
        assert_eq!(
            decoded.get(1).expect("read integer"),
            Some(ValueRef::Integer(2026))
        );
        assert_eq!(
            decoded.get(2).expect("read blob"),
            Some(ValueRef::Blob(&[0x01, 0x02, 0x03]))
        );
        assert_eq!(database.validate().expect("validate tree").rows, 1);
        drop(database);

        let mut database = Database::open(&vfs, path, 4).expect("reopen database");
        assert!(database.get(17).expect("query persisted row").is_some());
        database.checkpoint(&vfs).expect("checkpoint WAL");
        drop(database);

        let mut database = Database::open(&vfs, path, 4).expect("reopen checkpointed database");
        assert!(database.get(17).expect("query checkpointed row").is_some());
    }

    #[test]
    fn duplicate_rowid_does_not_replace_the_existing_record() {
        let vfs = SimVfs::new();
        let path = Path::new("database-duplicate.fdb");
        let mut database = Database::create(&vfs, path, 2).expect("create database");
        database
            .insert(1, &[Value::Text("original")])
            .expect("insert original row");

        assert!(matches!(
            database.insert(1, &[Value::Text("replacement")]),
            Err(DatabaseError::Btree(BtreeError::DuplicateRowId(1)))
        ));
        let record = database
            .get(1)
            .expect("query original row")
            .expect("row exists");
        let decoded = RecordRef::new(&record).expect("decode stored record");
        assert_eq!(
            decoded.get(0).expect("read original text"),
            Some(ValueRef::Text("original"))
        );
    }

    #[test]
    fn new_database_does_not_create_application_specific_tables() {
        let vfs = SimVfs::new();
        let mut database = Database::create(&vfs, Path::new("database-generic-schema.fdb"), 4)
            .expect("create database");

        assert!(matches!(
            database.table_columns("ferritedb_playback_history"),
            Err(DatabaseError::TableNotFound(_))
        ));
    }

    #[test]
    fn opening_a_legacy_catalog_with_app_metadata_preserves_rows() {
        let vfs = SimVfs::new();
        let path = Path::new("database-legacy-app-metadata.fdb");
        let mut pager = Pager::create(&vfs, path, 4).expect("create old-format database");
        let mut transaction = DatabasePageTransaction::new(pager.begin_write());
        let table_root = transaction.allocate_page().expect("allocate table root");
        let table =
            TableBtree::initialize_at(&mut transaction, table_root).expect("initialize records");
        TableBtree::initialize_at(&mut transaction, master_root().expect("master root"))
            .expect("initialize catalog");
        insert_table_metadata(&mut transaction, table_root).expect("write old catalog");
        let history_root = transaction
            .allocate_page()
            .expect("allocate old history table root");
        let history = TableBtree::initialize_at(&mut transaction, history_root)
            .expect("initialize old history table");
        insert_catalog_metadata(
            &mut transaction,
            2,
            "table",
            "ferritedb_playback_history",
            "ferritedb_playback_history",
            history_root,
            "CREATE TABLE \"ferritedb_playback_history\" (\"media_path\" BLOB, \"position_ms\" BLOB, \"duration_ms\" BLOB, \"updated_at_ms\" BLOB)",
        )
        .expect("write old history table metadata");
        insert_catalog_metadata(
            &mut transaction,
            3,
            "meta",
            "schema_version",
            "ferritedb",
            master_root().expect("master root"),
            "1",
        )
        .expect("write old schema version metadata");
        let record = ferrite_btree::encode_record(&[Value::Text("existing row")])
            .expect("encode existing row");
        table
            .insert(&mut transaction, 17, &record)
            .expect("insert old row");
        let old_history = ferrite_btree::encode_record(&[
            Value::Text("/media/legacy.mp4"),
            Value::Integer(100),
            Value::Integer(1_000),
            Value::Integer(50),
        ])
        .expect("encode old history row");
        history
            .insert(&mut transaction, 1, &old_history)
            .expect("insert old history row");
        transaction.commit().expect("commit old database");
        drop(pager);

        let mut reopened = Database::open(&vfs, path, 4).expect("open versionless catalog");
        let record = reopened
            .get(17)
            .expect("read old row")
            .expect("old row survives migration");
        assert_eq!(
            RecordRef::new(&record)
                .expect("decode old row")
                .get(0)
                .expect("read old value"),
            Some(ValueRef::Text("existing row"))
        );
        assert_eq!(
            reopened
                .scan_table("ferritedb_playback_history")
                .expect("read old history table")
                .len(),
            1
        );
    }

    #[test]
    fn backup_and_restore_preserve_committed_rows_and_wal_frames() {
        let vfs = SimVfs::new();
        let source_path = Path::new("database-backup-source.fdb");
        let backup_path = Path::new("database-backup.fdb");
        let restored_path = Path::new("database-backup-restored.fdb");
        let mut database = Database::create(&vfs, source_path, 4).expect("create source database");
        database
            .create_table("backup_items", &["value"])
            .expect("create backup table");
        database
            .insert_into("backup_items", 7, &[Value::Text("committed in WAL")])
            .expect("commit row to WAL");
        let source_stats = database.wal_stats().expect("read source WAL metrics");
        assert!(source_stats.committed_frames > 0);

        database
            .backup_to(&vfs, backup_path)
            .expect("create consistent backup");
        Database::restore_from(&vfs, backup_path, restored_path, 4).expect("restore backup");
        drop(database);

        let mut restored =
            Database::open(&vfs, restored_path, 4).expect("reopen restored database");
        assert!(
            restored
                .wal_stats()
                .expect("read restored WAL metrics")
                .committed_frames
                > 0
        );
        let record = restored
            .get_from("backup_items", 7)
            .expect("read restored row")
            .expect("restored row exists");
        assert_eq!(
            RecordRef::new(&record)
                .expect("decode restored row")
                .get(0)
                .expect("read restored value"),
            Some(ValueRef::Text("committed in WAL"))
        );
        assert!(matches!(
            restored.backup_to(&vfs, backup_path),
            Err(DatabaseError::Pager(PagerError::BackupTargetExists))
        ));
    }

    #[test]
    fn interrupted_insert_recovers_as_either_old_or_complete_new_state() {
        for (successful_writes, torn_bytes) in
            [(0, 0), (0, 17), (0, 24), (1, 0), (1, 17), (1, 4096)]
        {
            let vfs = SimVfs::new();
            let path_name =
                format!("database-interrupted-insert-{successful_writes}-{torn_bytes}.fdb");
            let path = Path::new(&path_name);
            let mut database = Database::create(&vfs, path, 2).expect("create database");
            vfs.inject_torn_write(successful_writes, torn_bytes)
                .expect("configure torn WAL write");

            assert!(matches!(
                database.insert(8, &[Value::Text("committed atomically")]),
                Err(DatabaseError::Pager(PagerError::CommitUncertain(_)))
            ));
            drop(database);
            vfs.clear_fault().expect("clear simulated power loss");

            let mut database = Database::open(&vfs, path, 2).expect("recover database");
            let record = database.get(8).expect("query after recovery");
            let stats = database.validate().expect("validate recovered tree");
            match record {
                None => assert_eq!(stats.rows, 0),
                Some(record) => {
                    let decoded = RecordRef::new(&record).expect("decode recovered row");
                    assert_eq!(
                        decoded.get(0).expect("read recovered value"),
                        Some(ValueRef::Text("committed atomically"))
                    );
                    assert_eq!(stats.rows, 1);
                }
            }
        }
    }

    #[test]
    fn interrupted_checkpoint_preserves_a_complete_committed_transaction() {
        use ferrite_vfs::{SimFaultKind, SimOperation};

        for (operation, successful_operations) in [
            (SimOperation::Write, 0),
            (SimOperation::Write, 1),
            (SimOperation::Write, 2),
            (SimOperation::Sync, 0),
            (SimOperation::Sync, 1),
        ] {
            let vfs = SimVfs::new();
            let path_name =
                format!("database-checkpoint-{operation:?}-{successful_operations}.fdb");
            let path = Path::new(&path_name);
            let mut database = Database::create(&vfs, path, 4).expect("create database");
            let mut transaction = database.begin_transaction();
            transaction
                .insert(1, &[Value::Text("first")])
                .expect("stage first row");
            transaction
                .insert(2, &[Value::Text("second")])
                .expect("stage second row");
            transaction.commit().expect("commit complete transaction");
            vfs.inject_failure(operation, successful_operations, SimFaultKind::PowerLoss)
                .expect("configure checkpoint fault");

            assert!(
                database.checkpoint(&vfs).is_err(),
                "injected {operation:?} fault after {successful_operations} successful operations was not reached"
            );
            drop(database);
            vfs.clear_fault().expect("clear simulated power loss");

            let mut reopened = Database::open(&vfs, path, 4).expect("recover database");
            assert_eq!(
                reopened.validate().expect("validate recovered tree").rows,
                2
            );
            for (rowid, expected) in [(1, "first"), (2, "second")] {
                let record = reopened
                    .get(rowid)
                    .expect("query recovered row")
                    .expect("committed row exists");
                let record = RecordRef::new(&record).expect("decode recovered row");
                assert_eq!(
                    record.get(0).expect("read recovered value"),
                    Some(ValueRef::Text(expected))
                );
            }
        }
    }

    #[test]
    fn database_transactions_batch_commits_and_drop_rolls_back() {
        let vfs = SimVfs::new();
        let path = Path::new("database-transactions.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        {
            let mut transaction = database.begin_transaction();
            transaction
                .insert(1, &[Value::Text("rolled back")])
                .expect("stage first row");
            transaction
                .insert(2, &[Value::Text("also rolled back")])
                .expect("stage second row");
        }
        assert!(database.get(1).expect("query after rollback").is_none());
        assert!(database.get(2).expect("query after rollback").is_none());

        let mut transaction = database.begin_transaction();
        transaction
            .insert(1, &[Value::Text("committed")])
            .expect("stage first row");
        transaction
            .insert(2, &[Value::Text("committed too")])
            .expect("stage second row");
        assert_eq!(
            transaction.validate().expect("validate staged rows").rows,
            2
        );
        transaction.commit().expect("commit batch");

        assert_eq!(
            database.validate().expect("validate committed rows").rows,
            2
        );
    }

    #[test]
    fn catalog_and_index_metadata_survive_reopen() {
        let vfs = SimVfs::new();
        let path = Path::new("database-index-catalog.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        database
            .insert(7, &[Value::Text("Ada"), Value::Integer(37)])
            .expect("insert first row");
        database
            .insert(9, &[Value::Text("Lin"), Value::Integer(37)])
            .expect("insert second row");
        database
            .create_index("idx_records_age", &[1])
            .expect("create index");
        assert_eq!(
            database
                .find_index("idx_records_age", &[Value::Integer(37)])
                .expect("query index"),
            vec![7, 9]
        );
        drop(database);

        let mut database = Database::open(&vfs, path, 4).expect("reopen indexed database");
        assert_eq!(
            database
                .find_index("idx_records_age", &[Value::Integer(37)])
                .expect("query reopened index"),
            vec![7, 9]
        );

        let mut transaction = database.begin_transaction();
        let master = TableBtree::open_at(
            &mut transaction.transaction,
            master_root().expect("master root"),
        )
        .expect("open ferrite_master");
        let rows = master
            .scan(&mut transaction.transaction)
            .expect("scan ferrite_master");
        assert_eq!(rows.len(), 2);
        let table_metadata = rows
            .iter()
            .map(|(_, bytes)| RecordRef::new(bytes).expect("decode catalog row"))
            .find(|record| {
                record.get(0) == Ok(Some(ValueRef::Text("table")))
                    && record.get(1) == Ok(Some(ValueRef::Text(TABLE_NAME)))
            })
            .expect("find table metadata");
        assert_eq!(
            table_metadata.get(2).expect("read table name"),
            Some(ValueRef::Text(TABLE_NAME))
        );
        assert_eq!(
            table_metadata.get(4).expect("read table SQL"),
            Some(ValueRef::Text(TABLE_SQL))
        );
        let table_root = required_integer(&table_metadata, 3).expect("read table root");
        assert!(table_root > 1);

        let index_metadata = rows
            .iter()
            .map(|(_, bytes)| RecordRef::new(bytes).expect("decode catalog row"))
            .find(|record| {
                record.get(0) == Ok(Some(ValueRef::Text("index")))
                    && record.get(1) == Ok(Some(ValueRef::Text("idx_records_age")))
            })
            .expect("find index metadata");
        assert_eq!(
            index_metadata.get(2).expect("read indexed table"),
            Some(ValueRef::Text(TABLE_NAME))
        );
        assert!(required_integer(&index_metadata, 3).expect("read index root") > table_root);
        assert_eq!(
            index_metadata.get(4).expect("read index SQL"),
            Some(ValueRef::Text(
                "CREATE INDEX \"idx_records_age\" ON \"records\" (\"column_1\")"
            ))
        );
    }

    #[test]
    fn opening_a_legacy_page_one_table_migrates_its_rows() {
        let vfs = SimVfs::new();
        let path = Path::new("database-legacy-migration.fdb");
        let mut pager = Pager::create(&vfs, path, 4).expect("create legacy database");
        let mut transaction = DatabasePageTransaction::new(pager.begin_write());
        let legacy_table =
            TableBtree::initialize(&mut transaction).expect("initialize legacy table");
        let legacy_record =
            ferrite_btree::encode_record(&[Value::Text("legacy row"), Value::Integer(42)])
                .expect("encode legacy record");
        legacy_table
            .insert(&mut transaction, 5, &legacy_record)
            .expect("insert legacy row");
        transaction.commit().expect("commit legacy row");
        drop(pager);

        let mut database = Database::open(&vfs, path, 4).expect("open and migrate database");
        let migrated = database
            .get(5)
            .expect("read migrated row")
            .expect("legacy row exists");
        let migrated = RecordRef::new(&migrated).expect("decode migrated row");
        assert_eq!(
            migrated.get(0).expect("read migrated text"),
            Some(ValueRef::Text("legacy row"))
        );
        assert_eq!(
            migrated.get(1).expect("read migrated integer"),
            Some(ValueRef::Integer(42))
        );
        assert_eq!(database.validate().expect("validate migrated tree").rows, 1);
    }

    #[test]
    fn insert_update_and_delete_keep_indexes_synchronized() {
        let vfs = SimVfs::new();
        let path = Path::new("database-index-mutations.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        database
            .insert(1, &[Value::Text("Ada"), Value::Integer(37)])
            .expect("insert first row");
        database
            .insert(2, &[Value::Text("Lin"), Value::Integer(28)])
            .expect("insert second row");
        database
            .create_index("idx_age", &[1])
            .expect("create index");

        database
            .insert(3, &[Value::Text("Kai"), Value::Integer(37)])
            .expect("insert indexed row");
        assert_eq!(
            database
                .find_index("idx_age", &[Value::Integer(37)])
                .expect("find rows after insert"),
            vec![1, 3]
        );

        database
            .update(1, &[Value::Text("Ada"), Value::Integer(28)])
            .expect("update indexed value");
        assert_eq!(
            database
                .find_index("idx_age", &[Value::Integer(37)])
                .expect("find rows after update"),
            vec![3]
        );
        assert_eq!(
            database
                .find_index("idx_age", &[Value::Integer(28)])
                .expect("find updated rows"),
            vec![1, 2]
        );

        assert!(database.delete(2).expect("delete indexed row"));
        assert_eq!(
            database
                .find_index("idx_age", &[Value::Integer(28)])
                .expect("find rows after delete"),
            vec![1]
        );
        assert!(database.get(2).expect("query deleted row").is_none());
        assert_eq!(database.validate().expect("validate table").rows, 2);
    }

    #[test]
    fn index_failure_aborts_all_staged_table_and_index_changes() {
        let vfs = SimVfs::new();
        let path = Path::new("database-index-abort.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        database
            .insert(1, &[Value::Text("existing"), Value::Integer(10)])
            .expect("insert existing row");
        database
            .create_index("idx_value", &[1])
            .expect("create index");

        let mut transaction = database.begin_transaction();
        transaction
            .insert(2, &[Value::Text("staged"), Value::Integer(20)])
            .expect("stage valid indexed row");
        assert!(matches!(
            transaction.insert(3, &[Value::Text("missing indexed field")]),
            Err(DatabaseError::MissingIndexColumn {
                rowid: 3,
                column: 1
            })
        ));
        assert!(matches!(
            transaction.commit(),
            Err(DatabaseError::TransactionAborted)
        ));

        assert!(database.get(2).expect("query rolled-back row").is_none());
        assert!(database.get(3).expect("query failed row").is_none());
        assert_eq!(
            database
                .validate()
                .expect("validate rolled-back table")
                .rows,
            1
        );
        assert_eq!(
            database
                .find_index("idx_value", &[Value::Integer(10)])
                .expect("query original index entry"),
            vec![1]
        );
        assert!(
            database
                .find_index("idx_value", &[Value::Integer(20)])
                .expect("query rolled-back index entry")
                .is_empty()
        );
    }

    #[test]
    fn named_tables_and_their_indexes_persist_independently() {
        let vfs = SimVfs::new();
        let path = Path::new("database-named-tables.fdb");
        let mut database = Database::create(&vfs, path, 4).expect("create database");
        database
            .create_table("users", &["name", "age"])
            .expect("create users table");
        database
            .create_table("products", &["title", "price"])
            .expect("create products table");
        database
            .insert_into("users", 1, &[Value::Text("Ada"), Value::Integer(37)])
            .expect("insert user");
        database
            .insert_into("products", 1, &[Value::Text("Ferrite"), Value::Integer(12)])
            .expect("insert product with overlapping rowid");
        database
            .create_index_on("idx_users_age", "users", &[1])
            .expect("index users age");
        assert_eq!(
            database
                .find_index_on("idx_users_age", "users", &[Value::Integer(37)])
                .expect("query users index"),
            vec![1]
        );
        drop(database);

        let mut database = Database::open(&vfs, path, 4).expect("reopen named tables");
        assert_eq!(
            database.table_columns("users").expect("read user columns"),
            vec!["name", "age"]
        );
        assert_eq!(
            database
                .table_columns("products")
                .expect("read product columns"),
            vec!["title", "price"]
        );
        assert_eq!(
            database
                .find_index_on("idx_users_age", "users", &[Value::Integer(37)])
                .expect("query persisted users index"),
            vec![1]
        );
        assert_eq!(database.scan_table("users").expect("scan users").len(), 1);
        assert_eq!(
            database
                .scan_table("products")
                .expect("scan products")
                .len(),
            1
        );
    }
}
