# Changelog

All notable changes to `spodeian-core` are documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and Semantic Versioning.

## [3.3.0] - 2026-11-28

### Added
- `spodeian-cache` 1.3: `ContentAddressedStorage::collect_garbage` deletes content no named reference points at, keeping anything written in the last `GARBAGE_GRACE` (60 s) so a concurrent write is never swept. `put` of content that already exists now refreshes its timestamp for the same reason.

### Fixed
- `spodeian-cache`: overwriting or removing a named reference left the old content on disk forever, so native `AsyncTieredStore`, `PreferentialStore` and `TieredMemoryStore` directories grew with every update. They now collect garbage when opened; keep unnamed content out of their directories.
- `spodeian-persist` 0.1.1: native `save_state_multi_tier` without a disk path stored a new file under `<temp>/<cas_namespace>` on every save and never removed any. It now keeps one named entry per key and sweeps superseded copies, including ones left by earlier versions. With a disk path it no longer writes the unused CAS copy as well.

## [3.2.0] - 2026-11-28

### Added
- `spodeian-cache` 1.2: batch operations that run in one IndexedDB transaction: `IndexedDbStore::{put_many_bytes, get_many_bytes, delete_many}` and `AsyncTieredStore::{put_many, get_many, remove_many}`. Loading or saving N records costs one transaction instead of N sequential ones.

### Changed
- `spodeian-cache`: `PreferentialStore` repeat saves of a key to the same tier no longer delete it again from the tiers read after that one (the first save per session still does). A save still always clears tiers that would shadow the new value.

## [3.1.0] - 2026-11-23

### Added
- `spodeian-core`: Added `preset-egui`, `preset-leptos-edge`, `preset-leptos-desktop`, `kernel`, `wgpu`, and `schema` feature flags, plus `docs/TEMPLATE_INTEGRATION_GUIDE.md`.
- `spodeian-forecast`: Added GARCH(1,1) QMLE and Holt linear parameter estimation (`fit`).
- `spodeian-stats`: Added MacKinnon ADF critical values and p-value approximation.
- `spodeian-ui` / `spodeian-leptos-ui` / `spodeian-web-utils` / `spodeian-export` / `spodeian-parallel` / `sml-kernel`: Added searchable `CommandPalette`, PWA/share-link web helpers, SVG/HTML export helpers, parallel sweep helpers, and WGSL compute context utilities.

## [3.0.1] - 2026-11-17

### Fixed
- `spodeian-forecast`: `Garch11::filter_and_forecast` returned a first forecast two steps ahead. `filtered[t]` is now the variance of `residuals[t]` (`filtered[0]` is the sample variance) and `forecasts[0]` is the one-step-ahead variance. Covered by tests against hand-computed values.

## [3.0.0] - 2026-11-14

### Added
- `spodeian-persist`: versioned JSON/RON app-state persistence, backup recovery, and schema migration helpers.
- `spodeian-leptos-ui`: Leptos 0.8 theme/dyslexia DOM bridges, modal focus trapping, telemetry summary formatting, and `ErrorBoundaryRecovery`.
- `spodeian-provenance`: whole-token SPDX/CC `LicenseVerifier`, unified `LedgerRecord`, and deterministic `ATTRIBUTIONS.txt` renderer.
- `spodeian-stats`: normal/Student-t/chi2 CDF & quantiles, OLS/WLS/Ridge/Huber/HAC regressions, Jarque-Bera, ADF, KPSS, Ljung-Box, Hurst, and Engle-Granger cointegration.
- `spodeian-forecast`: Holt linear ETS, `KalmanFilter1D`, `Garch11`, split-conformal intervals, pinball loss, and Gaussian CRPS.
- `spodeian-audio`: pure-Rust WAV PCM 16/24/32f encoder/decoder, linear resampling, acoustic quality metrics, and zero-allocation tanh soft-clipper.
- `sml-runtime`: `SELF_OPTIMIZING_META_INTEGRATOR_WGSL` kernel, `test_meta_proptest_invariants`, and `sml_meta_bench`.
- `sml-py-bridge`: `PySelfOptimizingMeta`, `py_meta_integrate`, and `spodeian_ml_optim.py`.

### Changed
- Separated `AI-Examples` curriculum crates (`sml-candle`, `sml-tools`, curriculum `kernels/`/`weights/`) out of `spodeian-core` into `ai-tests/curriculum/`.
- Salvaged `dir_walk` (`dir2txt`) and `serde_compat` (`u64_i64_compat`) into `spodeian-export`.

### Removed
- Removed deprecated shim crates `spodeian-ml-utils` and `spodeian-lake`.
