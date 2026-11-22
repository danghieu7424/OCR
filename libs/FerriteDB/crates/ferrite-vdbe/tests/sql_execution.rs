use ferrite_vdbe::{AccessPath, ExecutionResult, SqlValue, compile, execute, execute_plan, plan};
use ferrite_vfs::SimVfs;
use ferritedb_core::Database;
use std::path::Path;

fn database() -> (SimVfs, Database) {
    let vfs = SimVfs::new();
    let database = Database::create(&vfs, Path::new("vdbe.db"), 8).unwrap();
    (vfs, database)
}

fn complete(result: ExecutionResult) {
    assert!(matches!(result, ExecutionResult::CommandComplete { .. }));
}

#[test]
fn runs_all_statement_types_and_reopens_persisted_data() {
    let (vfs, mut database) = database();

    complete(
        execute(
            &mut database,
            &compile(
                "CREATE TABLE items (name TEXT, qty INTEGER, amount REAL, payload BLOB, note TEXT)",
            )
            .unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("CREATE INDEX item_name ON items (name)").unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("INSERT INTO items VALUES ('pear', 4, 2.5, X'00ff', 'it''s ripe')").unwrap(),
        )
        .unwrap(),
    );
    complete(execute(&mut database, &compile("INSERT INTO items (qty, name, amount, payload, note) VALUES (9, 'plum', -1.25, X'', NULL)").unwrap()).unwrap());

    let selected = execute(
        &mut database,
        &compile("SELECT name, note FROM items WHERE name = 'pear'").unwrap(),
    )
    .unwrap();
    assert_eq!(
        selected,
        ExecutionResult::Rows {
            columns: vec!["name".into(), "note".into()],
            rows: vec![vec![
                SqlValue::Text("pear".into()),
                SqlValue::Text("it's ripe".into())
            ]]
        }
    );

    complete(
        execute(
            &mut database,
            &compile("UPDATE items SET qty=5, note='updated' WHERE name='pear'").unwrap(),
        )
        .unwrap(),
    );
    let updated = execute(
        &mut database,
        &compile("SELECT * FROM items WHERE name='pear'").unwrap(),
    )
    .unwrap();
    assert_eq!(
        updated,
        ExecutionResult::Rows {
            columns: vec![
                "name".into(),
                "qty".into(),
                "amount".into(),
                "payload".into(),
                "note".into()
            ],
            rows: vec![vec![
                SqlValue::Text("pear".into()),
                SqlValue::Integer(5),
                SqlValue::Real(2.5),
                SqlValue::Blob(vec![0, 255]),
                SqlValue::Text("updated".into()),
            ]]
        }
    );

    complete(
        execute(
            &mut database,
            &compile("DELETE FROM items WHERE name='plum'").unwrap(),
        )
        .unwrap(),
    );
    drop(database);
    let mut reopened = Database::open(&vfs, Path::new("vdbe.db"), 8).unwrap();
    let persisted = execute(
        &mut reopened,
        &compile("SELECT name, qty FROM items").unwrap(),
    )
    .unwrap();
    assert_eq!(
        persisted,
        ExecutionResult::Rows {
            columns: vec!["name".into(), "qty".into()],
            rows: vec![vec![SqlValue::Text("pear".into()), SqlValue::Integer(5)]]
        }
    );
}

#[test]
fn index_plan_uses_index_and_matches_scan_results() {
    let (_vfs, mut database) = database();
    complete(
        execute(
            &mut database,
            &compile("CREATE TABLE items (name TEXT, qty INTEGER)").unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("INSERT INTO items VALUES ('pear', 4)").unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("INSERT INTO items VALUES ('plum', 9)").unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("CREATE INDEX item_name ON items (name)").unwrap(),
        )
        .unwrap(),
    );

    let indexed = plan(&mut database, "SELECT qty FROM items WHERE name='pear'").unwrap();
    assert_eq!(
        indexed.access_path,
        AccessPath::IndexLookup {
            name: "item_name".into(),
            column: 0
        }
    );
    let indexed_rows = execute_plan(&mut database, &indexed).unwrap();
    assert_eq!(
        indexed_rows,
        ExecutionResult::Rows {
            columns: vec!["qty".into()],
            rows: vec![vec![SqlValue::Integer(4)]]
        }
    );

    let update = plan(&mut database, "UPDATE items SET qty=10 WHERE name='plum'").unwrap();
    assert!(matches!(update.access_path, AccessPath::IndexLookup { .. }));
    complete(execute_plan(&mut database, &update).unwrap());

    let delete = plan(&mut database, "DELETE FROM items WHERE name='pear'").unwrap();
    assert!(matches!(delete.access_path, AccessPath::IndexLookup { .. }));
    complete(execute_plan(&mut database, &delete).unwrap());

    let scan = plan(&mut database, "SELECT qty FROM items WHERE qty=10").unwrap();
    assert_eq!(scan.access_path, AccessPath::FullTableScan);
    assert_eq!(
        execute_plan(&mut database, &scan).unwrap(),
        ExecutionResult::Rows {
            columns: vec!["qty".into()],
            rows: vec![vec![SqlValue::Integer(10)]]
        }
    );
    assert_eq!(
        execute(
            &mut database,
            &compile("SELECT * FROM items WHERE name='plum'").unwrap()
        )
        .unwrap(),
        ExecutionResult::Rows {
            columns: vec!["name".into(), "qty".into()],
            rows: vec![vec![SqlValue::Text("plum".into()), SqlValue::Integer(10)]]
        }
    );
}

#[test]
fn create_table_can_reuse_the_default_records_tree_on_a_fresh_database() {
    let (_vfs, mut database) = database();
    complete(
        execute(
            &mut database,
            &compile("CREATE TABLE records (label TEXT)").unwrap(),
        )
        .unwrap(),
    );
    complete(
        execute(
            &mut database,
            &compile("INSERT INTO records VALUES ('sql-owned')").unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        execute(
            &mut database,
            &compile("SELECT label FROM records WHERE label='sql-owned'").unwrap(),
        )
        .unwrap(),
        ExecutionResult::Rows {
            columns: vec!["label".into()],
            rows: vec![vec![SqlValue::Text("sql-owned".into())]]
        }
    );
}
