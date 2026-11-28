//! Browser app-state persistence in IndexedDB ([`WebStateStore`]).
//!
//! **Stability: Stable** — semver-guaranteed since v0.1.2. Available on `wasm32` only.
//!
//! The whole serialized state is one IndexedDB record. It is read once when the store
//! opens and written asynchronously afterwards: saves run one at a time, a burst of
//! changes collapses into one write of the latest state, and an unchanged state is not
//! rewritten. The UI thread never blocks on storage, and nothing reports success before
//! the browser confirms the write.
//!
//! Immediate-mode UIs (egui) poll [`WebStateStore::take_loaded`] and
//! [`WebStateStore::take_outcome`] each frame; the `on_change` callback passed to
//! [`WebStateStore::open`] can request a repaint so those results are picked up promptly.

use spodeian_cache::CacheError;
use spodeian_cache::web::IndexedDbStore;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What a finished save did.
#[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SaveOutcome {
    /// The state is durable in IndexedDB.
    Saved,
    /// The write failed; the message says why. It contains `QuotaExceeded` when the
    /// browser's storage quota is full.
    Failed(String),
}

/// Where to look for state saved by earlier versions, and clear it once migrated.
#[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
#[derive(Debug, Clone, Default)]
pub struct LegacyLocations {
    /// `localStorage` keys that may hold the state. They are removed after the first
    /// successful save, once IndexedDB holds the state.
    pub local_storage_keys: Vec<String>,
    /// Whole IndexedDB databases from earlier storage code, deleted after the first
    /// successful save.
    pub databases: Vec<String>,
}

struct Inner {
    key: String,
    legacy: LegacyLocations,
    on_change: Box<dyn Fn()>,
    db: RefCell<Option<IndexedDbStore>>,
    opening: Cell<bool>,
    pending: RefCell<Option<String>>,
    saving: Cell<bool>,
    last_saved: RefCell<Option<String>>,
    loaded: RefCell<Option<Option<String>>>,
    outcome: RefCell<Option<SaveOutcome>>,
}

/// App state stored as one IndexedDB record, loaded once and saved in the background.
///
/// Cloning gives another handle to the same store.
#[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
#[derive(Clone)]
pub struct WebStateStore {
    inner: Rc<Inner>,
}

impl std::fmt::Debug for WebStateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebStateStore")
            .field("key", &self.inner.key)
            .finish_non_exhaustive()
    }
}

impl WebStateStore {
    /// Opens the store and starts loading `key` from the IndexedDB database `db_name`.
    ///
    /// The loaded state arrives through [`Self::take_loaded`]. Saves queued before the
    /// load finishes wait for it; if a saved state is found they are dropped, because
    /// the loaded state replaces whatever the app started with. `on_change` runs whenever
    /// a load or save result becomes available (for example to request a repaint).
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub fn open(
        db_name: &str,
        key: &str,
        legacy: LegacyLocations,
        on_change: impl Fn() + 'static,
    ) -> Self {
        let store = Self {
            inner: Rc::new(Inner {
                key: key.to_string(),
                legacy,
                on_change: Box::new(on_change),
                db: RefCell::new(None),
                opening: Cell::new(true),
                pending: RefCell::new(None),
                saving: Cell::new(false),
                last_saved: RefCell::new(None),
                loaded: RefCell::new(None),
                outcome: RefCell::new(None),
            }),
        };
        let this = store.clone();
        let db_name = db_name.to_string();
        wasm_bindgen_futures::spawn_local(async move { this.load(&db_name).await });
        store
    }

    async fn load(&self, db_name: &str) {
        let inner = &self.inner;
        let db = match IndexedDbStore::open(db_name, "state").await {
            Ok(db) => db,
            Err(e) => {
                inner.opening.set(false);
                *inner.loaded.borrow_mut() = Some(None);
                return self.report(SaveOutcome::Failed(format!("Could not open storage: {e}")));
            }
        };
        let saved = match db.get_bytes(&inner.key).await {
            Ok(bytes) => bytes.and_then(|b| String::from_utf8(b).ok()),
            Err(e) => {
                self.report(SaveOutcome::Failed(format!(
                    "Could not load saved data: {e}"
                )));
                None
            }
        };
        if saved.is_some() {
            inner.pending.borrow_mut().take();
            inner.last_saved.borrow_mut().clone_from(&saved);
        }
        *inner.loaded.borrow_mut() = Some(saved);
        *inner.db.borrow_mut() = Some(db);
        inner.opening.set(false);
        (inner.on_change)();
        self.run_saves();
    }

    /// The state loaded at open, once: `Some(Some(json))` when one was saved,
    /// `Some(None)` when nothing was (first run, or storage unavailable), and `None`
    /// while the load is still running or after the result has been taken.
    ///
    /// On `Some(None)`, save whatever state the app started with (for example one read
    /// from [`LegacyLocations`]) to move it into IndexedDB.
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub fn take_loaded(&self) -> Option<Option<String>> {
        self.inner.loaded.borrow_mut().take()
    }

    /// The result of the most recent finished save, once.
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub fn take_outcome(&self) -> Option<SaveOutcome> {
        self.inner.outcome.borrow_mut().take()
    }

    /// Queues `json` to be written. Returns immediately; the result arrives through
    /// [`Self::take_outcome`]. A state identical to the last one written is skipped.
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub fn queue_save(&self, json: String) {
        if self.inner.last_saved.borrow().as_deref() == Some(json.as_str()) {
            return;
        }
        *self.inner.pending.borrow_mut() = Some(json);
        self.run_saves();
    }

    /// Whether the load has finished and no save is queued or running.
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub fn is_idle(&self) -> bool {
        !self.inner.opening.get()
            && !self.inner.saving.get()
            && self.inner.pending.borrow().is_none()
    }

    /// Resolves once [`Self::is_idle`] is true.
    #[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
    pub async fn settled(&self) {
        while !self.is_idle() {
            sleep_ms(10).await;
        }
    }

    fn report(&self, outcome: SaveOutcome) {
        *self.inner.outcome.borrow_mut() = Some(outcome);
        (self.inner.on_change)();
    }

    fn run_saves(&self) {
        let Some(db) = self.inner.db.borrow().clone() else {
            // Still opening: the load starts the saves when it finishes. Opening failed:
            // nothing can be written, so say so instead of queueing forever.
            if !self.inner.opening.get() && self.inner.pending.borrow_mut().take().is_some() {
                self.report(SaveOutcome::Failed("Storage is unavailable".into()));
            }
            return;
        };
        if self.inner.pending.borrow().is_none() || self.inner.saving.replace(true) {
            return;
        }
        let this = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let inner = &this.inner;
            loop {
                let Some(json) = inner.pending.borrow_mut().take() else {
                    break;
                };
                match db.put_bytes(&inner.key, json.as_bytes()).await {
                    Ok(()) => {
                        let first_save = inner.last_saved.borrow().is_none();
                        *inner.last_saved.borrow_mut() = Some(json);
                        if first_save {
                            this.remove_legacy_copies();
                        }
                        this.report(SaveOutcome::Saved);
                    }
                    Err(e) => this.report(SaveOutcome::Failed(format!(
                        "Changes could not be saved: {e}"
                    ))),
                }
            }
            inner.saving.set(false);
        });
    }

    fn remove_legacy_copies(&self) {
        let legacy = &self.inner.legacy;
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(Some(storage)) = window.local_storage() {
            for key in &legacy.local_storage_keys {
                let _ = storage.remove_item(key);
            }
        }
        if let Ok(Some(factory)) = window.indexed_db() {
            for name in &legacy.databases {
                let _ = factory.delete_database(name);
            }
        }
    }
}

async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// Reads the raw state stored under `key` in `db_name`, for tools and tests that need it
/// without opening a [`WebStateStore`].
#[doc = "**Stability: Stable** — semver-guaranteed since v0.1.2"]
pub async fn read_saved_state(db_name: &str, key: &str) -> Result<Option<String>, CacheError> {
    let db = IndexedDbStore::open(db_name, "state").await?;
    let bytes = db.get_bytes(key).await?;
    db.close();
    Ok(bytes.and_then(|b| String::from_utf8(b).ok()))
}
