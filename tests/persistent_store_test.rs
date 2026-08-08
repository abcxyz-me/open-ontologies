use open_ontologies::config::{resolve_storage_mode, StorageConfig, StorageMode};
use open_ontologies::graph::GraphStore;
use tempfile::TempDir;

#[test]
fn persistent_store_survives_drop_and_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("triplestore");

    let ttl = r#"
        @prefix ex: <http://example.org/> .
        ex:Alice a ex:Person .
        ex:Bob   a ex:Person .
    "#;

    {
        let store = GraphStore::open_persistent(&path).unwrap();
        assert_eq!(store.triple_count(), 0);
        store.load_turtle(ttl, None).unwrap();
        assert_eq!(store.triple_count(), 2);
    }

    {
        let store = GraphStore::open_persistent(&path).unwrap();
        assert_eq!(store.triple_count(), 2);
        let json = store
            .sparql_select("SELECT ?s WHERE { ?s a <http://example.org/Person> }")
            .unwrap();
        assert!(json.contains("Alice"));
        assert!(json.contains("Bob"));
    }
}

#[test]
fn storage_mode_resolves_default_to_memory() {
    let cfg = StorageConfig::default();
    // Guard against a stray env var in the test runner shell.
    // SAFETY: tests run single-threaded for env mutation by default in cargo;
    // this is only touched here, so no cross-test interference.
    unsafe {
        std::env::remove_var("OPEN_ONTOLOGIES_STORAGE_MODE");
    }
    assert_eq!(resolve_storage_mode(&cfg), StorageMode::Memory);
}

#[test]
fn storage_mode_parses_persistent() {
    let cfg = StorageConfig {
        mode: "persistent".to_string(),
    };
    unsafe {
        std::env::remove_var("OPEN_ONTOLOGIES_STORAGE_MODE");
    }
    assert_eq!(resolve_storage_mode(&cfg), StorageMode::Persistent);
}
