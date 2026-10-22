use dbcraft_core::{ConnectionKind, ConnectionProfile};
use dbcraft_driver::{connect, execute_sql, list_databases, list_table_columns, list_tables};

#[tokio::test]
async fn sqlite_memory_roundtrip() {
    let profile = ConnectionProfile {
        kind: ConnectionKind::Sqlite,
        name: "test".into(),
        file_path: ":memory:".into(),
        ..ConnectionProfile::default()
    };
    let conn = connect(&profile, "").await.expect("connect");
    execute_sql(&conn, "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .expect("create");
    execute_sql(&conn, "INSERT INTO items (name) VALUES ('alpha'), ('beta')")
        .await
        .expect("insert");
    let result = execute_sql(&conn, "SELECT id, name FROM items ORDER BY id")
        .await
        .expect("select");
    assert_eq!(result.row_count(), 2);
    assert_eq!(result.columns.len(), 2);

    let dbs = list_databases(&conn).await.expect("dbs");
    assert_eq!(dbs.len(), 1);
    let tables = list_tables(&conn, "main").await.expect("tables");
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].name, "items");
    let cols = list_table_columns(&conn, "main", "items")
        .await
        .expect("cols");
    assert!(cols.len() >= 2);
}
