//! Browser tests for `WebStateStore`. Run with wasm-bindgen-test in a headless browser.
#![cfg(target_arch = "wasm32")]

use spodeian_persist::web_state::read_saved_state;
use spodeian_persist::{LegacyLocations, SaveOutcome, WebStateStore};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

fn unique(name: &str) -> String {
    format!("{name}-{}", (js_sys::Math::random() * 1e12) as u64)
}

#[wasm_bindgen_test]
async fn first_run_loads_nothing_then_saves_and_reloads() {
    let db = unique("state");
    let changes = Rc::new(Cell::new(0));
    let counter = changes.clone();
    let store = WebStateStore::open(&db, "app", LegacyLocations::default(), move || {
        counter.set(counter.get() + 1)
    });
    store.settled().await;
    assert_eq!(store.take_loaded(), Some(None));
    assert_eq!(store.take_loaded(), None, "the load result is taken once");

    store.queue_save(r#"{"v":1}"#.into());
    store.queue_save(r#"{"v":2}"#.into()); // coalesces with the first
    store.settled().await;
    assert_eq!(store.take_outcome(), Some(SaveOutcome::Saved));
    assert!(
        changes.get() >= 2,
        "on_change ran for the load and the save"
    );
    assert_eq!(
        read_saved_state(&db, "app").await.unwrap().as_deref(),
        Some(r#"{"v":2}"#)
    );

    let reopened = WebStateStore::open(&db, "app", LegacyLocations::default(), || {});
    reopened.settled().await;
    assert_eq!(reopened.take_loaded(), Some(Some(r#"{"v":2}"#.to_string())));
    // An unchanged state is not rewritten, so no outcome is reported.
    reopened.queue_save(r#"{"v":2}"#.into());
    reopened.settled().await;
    assert_eq!(reopened.take_outcome(), None);
}

#[wasm_bindgen_test]
async fn saves_queued_before_the_load_do_not_overwrite_saved_state() {
    let db = unique("state-race");
    let first = WebStateStore::open(&db, "app", LegacyLocations::default(), || {});
    first.queue_save("saved".into());
    first.settled().await;

    let second = WebStateStore::open(&db, "app", LegacyLocations::default(), || {});
    second.queue_save("startup defaults".into()); // before the load completes
    second.settled().await;
    assert_eq!(second.take_loaded(), Some(Some("saved".to_string())));
    assert_eq!(
        read_saved_state(&db, "app").await.unwrap().as_deref(),
        Some("saved")
    );
}

#[wasm_bindgen_test]
async fn first_save_removes_legacy_local_storage_copies() {
    let storage = web_sys::window().unwrap().local_storage().unwrap().unwrap();
    let legacy_key = unique("legacy-key");
    storage.set_item(&legacy_key, "old").unwrap();
    let legacy = LegacyLocations {
        local_storage_keys: vec![legacy_key.clone()],
        databases: vec![],
    };
    let store = WebStateStore::open(&unique("state-legacy"), "app", legacy, || {});
    store.settled().await;
    assert_eq!(store.take_loaded(), Some(None));
    store.queue_save("migrated".into());
    store.settled().await;
    assert_eq!(store.take_outcome(), Some(SaveOutcome::Saved));
    assert_eq!(storage.get_item(&legacy_key).unwrap(), None);
}
