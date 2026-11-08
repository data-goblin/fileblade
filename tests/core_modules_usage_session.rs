use fileblade::core_modules::usage::sql::Sql;
use tempfile::tempdir;

#[test]
fn one_store_session_answers_scripts_reports_errors_and_recovers() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("store.sqlite3");
    let database = Sql::open(&path).unwrap();
    database
        .execute("PRAGMA journal_mode = WAL;\nCREATE TABLE item (name TEXT)")
        .unwrap();
    database
        .execute("BEGIN IMMEDIATE;\nINSERT INTO item VALUES ('a');\nINSERT INTO item VALUES ('b');\nCOMMIT;\n")
        .unwrap();
    let rows = database
        .query("SELECT 1;\nSELECT name FROM item ORDER BY name")
        .unwrap();
    assert_eq!(
        rows.iter().map(|row| row.text(0)).collect::<Vec<_>>(),
        ["a", "b"]
    );
    let refused = database
        .execute("BEGIN IMMEDIATE;\nINSERT INTO item VALUES ('c');\nINSERT INTO missing VALUES (1);\nCOMMIT;\n")
        .unwrap_err();
    assert!(refused.message().contains("no such table"), "{refused}");
    let count = database
        .query_one("SELECT count(*) FROM item")
        .unwrap()
        .unwrap();
    assert_eq!(count.integer(0), 2);
    assert!(
        database
            .query("SELECT name FROM item WHERE name = 'z'")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_store_replaced_at_the_same_path_is_read_from_its_new_file() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("opencode.db");
    let replacement = temporary.path().join("opencode.db.new");
    Sql::open(&path)
        .unwrap()
        .execute("CREATE TABLE item (name TEXT);\nINSERT INTO item VALUES ('old')")
        .unwrap();
    Sql::open(&replacement)
        .unwrap()
        .execute("CREATE TABLE item (name TEXT);\nINSERT INTO item VALUES ('new')")
        .unwrap();
    let reader = Sql::open_readonly(&path).unwrap();
    let names = |database: &Sql| {
        database
            .query("SELECT name FROM item")
            .unwrap()
            .iter()
            .map(|row| row.text(0).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&reader), ["old"]);
    std::fs::rename(&replacement, &path).unwrap();
    assert_eq!(names(&reader), ["new"]);
}
