use ferrite_btree::{RecordRef, Value, ValueRef};
use ferrite_cache::{CacheError, CachedDatabase, InMemoryValkey};
use ferrite_vdbe::{AccessPath, ExecutionResult, SqlValue, compile, execute, execute_plan, plan};
use ferrite_vfs::SimVfs;
use ferritedb_core::{Database, DatabaseError};
use std::path::Path;
use std::sync::{Arc, Mutex};

fn create_database(path: &str) -> (SimVfs, Database) {
    let vfs = SimVfs::new();
    let database = Database::create(&vfs, Path::new(path), 4).expect("create database");
    (vfs, database)
}

#[test]
fn committed_updates_rollback_and_deletes_keep_cache_consistent() {
    let (vfs, database) = create_database("cache-consistency.fdb");
    let mut database = CachedDatabase::new(database, InMemoryValkey::default(), "test");

    let mut transaction = database.begin_transaction();
    transaction
        .insert("records", 1, &[Value::Text("committed"), Value::Integer(1)])
        .expect("stage insert");
    transaction.commit().expect("commit insert");
    let record = database
        .get("records", 1)
        .expect("cache aside read")
        .expect("inserted record exists");
    assert_eq!(
        RecordRef::new(&record)
            .expect("decode committed record")
            .get(0)
            .expect("read text"),
        Some(ValueRef::Text("committed"))
    );

    let mut transaction = database.begin_transaction();
    transaction
        .update(
            "records",
            1,
            &[Value::Text("rolled back"), Value::Integer(2)],
        )
        .expect("stage update");
    transaction.rollback().expect("rollback update");
    let record = database
        .get("records", 1)
        .expect("read after rollback")
        .expect("original record remains");
    assert_eq!(
        RecordRef::new(&record)
            .expect("decode original record")
            .get(0)
            .expect("read original text"),
        Some(ValueRef::Text("committed"))
    );

    let mut transaction = database.begin_transaction();
    assert!(transaction.delete("records", 1).expect("stage deletion"));
    transaction.commit().expect("commit deletion");
    assert!(
        database
            .cache_state("records", 1)
            .expect("inspect deleted cache row")
            .record
            .is_none()
    );
    assert!(
        database
            .get("records", 1)
            .expect("query deleted row")
            .is_none()
    );
    drop(database);

    let mut reopened =
        Database::open(&vfs, Path::new("cache-consistency.fdb"), 4).expect("reopen database");
    assert!(reopened.get(1).expect("verify durable deletion").is_none());
}

#[test]
fn crash_after_durable_wal_commit_is_repaired_from_dirty_marker() {
    let (vfs, database) = create_database("cache-crash-window.fdb");
    let mut database = CachedDatabase::new(database, InMemoryValkey::default(), "test");
    let client = database.client_handle();
    client.lock().expect("lock mock Valkey").fail_next_apply();

    let mut transaction = database.begin_transaction();
    transaction
        .insert("records", 7, &[Value::Text("durable")])
        .expect("stage insert");
    assert!(matches!(
        transaction.commit(),
        Err(CacheError::Database(DatabaseError::CommitHookAfter(_)))
    ));
    assert!(
        database
            .cache_state("records", 7)
            .expect("inspect dirty marker")
            .dirty_token
            .is_some()
    );
    drop(database);

    let reopened =
        Database::open(&vfs, Path::new("cache-crash-window.fdb"), 4).expect("reopen database");
    let mut database = CachedDatabase::with_client_handle(reopened, client, "test");
    let record = database
        .get("records", 7)
        .expect("refresh cache from durable storage")
        .expect("committed record exists");
    assert_eq!(
        RecordRef::new(&record)
            .expect("decode durable record")
            .get(0)
            .expect("read value"),
        Some(ValueRef::Text("durable"))
    );
    assert!(
        database
            .cache_state("records", 7)
            .expect("inspect repaired marker")
            .dirty_token
            .is_none()
    );
}

#[test]
fn uncertain_wal_commit_keeps_cache_key_dirty() {
    let (vfs, database) = create_database("cache-uncertain-commit.fdb");
    let mut database = CachedDatabase::new(database, InMemoryValkey::default(), "test");

    let mut transaction = database.begin_transaction();
    transaction
        .insert("records", 7, &[Value::Text("uncertain")])
        .expect("stage insert");
    vfs.inject_torn_write(0, 7)
        .expect("configure torn WAL write");

    let error = transaction
        .commit()
        .expect_err("commit should be uncertain");
    assert!(
        error.to_string().contains("uncertain"),
        "unexpected commit error: {error}"
    );
    assert!(
        database
            .cache_state("records", 7)
            .expect("inspect dirty marker")
            .dirty_token
            .is_some()
    );
}

#[test]
fn vdbe_sql_runs_on_cached_database_and_commits_cache_changes() {
    let (vfs, database) = create_database("cache-vdbe.fdb");
    let mut database = CachedDatabase::new(database, InMemoryValkey::default(), "sql-test");

    for sql in [
        "CREATE TABLE products (name TEXT, qty INTEGER)",
        "CREATE INDEX products_name ON products (name)",
        "INSERT INTO products VALUES ('pear', 4)",
    ] {
        assert!(matches!(
            execute(&mut database, &compile(sql).expect("compile SQL")).expect("execute SQL"),
            ExecutionResult::CommandComplete { .. }
        ));
    }

    let select = plan(
        &mut database,
        "SELECT name, qty FROM products WHERE name = 'pear'",
    )
    .expect("plan indexed query");
    assert!(matches!(select.access_path, AccessPath::IndexLookup { .. }));
    assert_eq!(
        execute_plan(&mut database, &select).expect("select from cached database"),
        ExecutionResult::Rows {
            columns: vec!["name".into(), "qty".into()],
            rows: vec![vec![SqlValue::Text("pear".into()), SqlValue::Integer(4)]],
        }
    );
    assert!(
        database
            .cache_state("products", 1)
            .expect("inspect cached SQL row")
            .record
            .is_some()
    );

    execute(
        &mut database,
        &compile("UPDATE products SET qty = 5 WHERE name = 'pear'").expect("compile update"),
    )
    .expect("execute update");
    let updated = database
        .get("products", 1)
        .expect("read committed cache row")
        .expect("updated row exists");
    assert_eq!(
        RecordRef::new(&updated)
            .expect("decode updated row")
            .get(1)
            .expect("read updated quantity"),
        Some(ValueRef::Integer(5))
    );

    execute(
        &mut database,
        &compile("DELETE FROM products WHERE name = 'pear'").expect("compile delete"),
    )
    .expect("execute delete");
    assert!(
        database
            .cache_state("products", 1)
            .expect("inspect deleted cache row")
            .record
            .is_none()
    );
    drop(database);

    let mut reopened =
        Database::open(&vfs, Path::new("cache-vdbe.fdb"), 4).expect("reopen SQL database");
    assert!(
        reopened
            .get_from("products", 1)
            .expect("verify SQL deletion")
            .is_none()
    );
}

#[test]
fn stale_writer_failure_restores_dirty_marker() {
    let (vfs, database) = create_database("cache-stale-writer.fdb");
    let client = Arc::new(Mutex::new(InMemoryValkey::default()));
    let mut current =
        CachedDatabase::with_client_handle(database, Arc::clone(&client), "stale-test");
    let stale_database =
        Database::open(&vfs, Path::new("cache-stale-writer.fdb"), 4).expect("open stale handle");
    let mut stale =
        CachedDatabase::with_client_handle(stale_database, Arc::clone(&client), "stale-test");

    let mut stale_transaction = stale.begin_transaction();
    stale_transaction
        .insert("records", 2, &[Value::Text("stale")])
        .expect("stage stale writer row");

    let mut current_transaction = current.begin_transaction();
    current_transaction
        .insert("records", 1, &[Value::Text("current")])
        .expect("stage current writer row");
    current_transaction.commit().expect("commit current writer");

    let error = stale_transaction
        .commit()
        .expect_err("stale writer must not overwrite the current commit");
    assert!(error.to_string().contains("writer snapshot is stale"));
    assert!(
        current
            .cache_state("records", 2)
            .expect("inspect failed writer cache key")
            .dirty_token
            .is_none()
    );
}
