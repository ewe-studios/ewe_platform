#![cfg(feature = "db")]

//! `SqlPolicyStore` over a real (temporary) Turso database.

use foundation_cedar::{load_from_store, PolicyStore, SqlPolicyStore};
use foundation_core::valtron::valtron_test;
use foundation_db::{QueryStore, DataValue, TursoStorage};

const SCHEMA: &str = r#"
entity User;
entity Album;
action view appliesTo {
  principal: [User],
  resource: [Album]
};
"#;

const POLICIES: &str = r#"
permit(
    principal == User::"alice",
    action == Action::"view",
    resource == Album::"trip"
);
"#;

async fn turso() -> (TursoStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = dir.path().join("cedar.db").to_string_lossy().into_owned();
    let store = TursoStorage::new(&url).await.expect("open turso");
    (store, dir)
}

async fn seed(store: &TursoStorage, ns: &str, entities: Option<&str>) {
    let entities = entities.map_or(DataValue::Null, |e| DataValue::Text(e.to_string()));
    store
        .execute_async(
            "INSERT INTO cedar_policies (namespace, policy_text, schema_text, entities_json, version) VALUES (?, ?, ?, ?, ?)",
            &[
                DataValue::Text(ns.to_string()),
                DataValue::Text(POLICIES.to_string()),
                DataValue::Text(SCHEMA.to_string()),
                entities,
                DataValue::Text("7".to_string()),
            ],
        )
        .await
        .expect("seed cedar_policies");
}

#[valtron_test]
async fn sql_store_loads_a_namespace() {
    let (db, _dir) = turso().await;
    let store = SqlPolicyStore::new(&db, "authz");
    store.ensure_table().await.expect("create table");
    seed(&db, "authz", None).await;

    assert!(store.load_schema().await.unwrap().contains("entity User"));
    assert!(store.load_policies().await.unwrap().contains("permit"));
    // A NULL `entities_json` means no entities.
    assert!(store.load_entities().await.unwrap().is_none());
    assert_eq!(store.version().await.unwrap(), "7");
    assert!(load_from_store(&store).await.is_ok());
}

#[valtron_test]
async fn sql_store_reads_entities() {
    let (db, _dir) = turso().await;
    let store = SqlPolicyStore::new(&db, "authz");
    store.ensure_table().await.expect("create table");
    seed(&db, "authz", Some("[]")).await;

    assert_eq!(store.load_entities().await.unwrap().as_deref(), Some("[]"));
}

#[valtron_test]
async fn sql_store_missing_namespace_is_an_error() {
    let (db, _dir) = turso().await;
    let store = SqlPolicyStore::new(&db, "missing");
    store.ensure_table().await.expect("create table");

    assert!(store.load_policies().await.is_err());
}
