Project-specific development guidelines

Audience and scope
- Audience: Advanced Rust developers familiar with async (Tokio) and service integrations.
- Scope: Build/configuration details unique to this repo, testing guidance that actually works here, and practical dev notes for iterating on the Roon <-> FSCT bridge.

Build and configuration
- Toolchain
  - Rust 1.78+ (edition = "2021"). Tokio 1.47 is used with macros + rt. No nightly features required.
  - Dependencies include several Git-based crates pulled at specific refs/branches:
    - fsct-client and fsct-core from https://github.com/HEM-RnD/fsct-host.git, branch FSCT-7-Roon-extension
    - roon-api from https://github.com/TheAppgineer/rust-roon-api.git, tag 0.3.1, features: settings, status, transport
  - First build requires network access to fetch the deps.

- Build options
  - Run: cargo build

- Runtime configuration and persisted state
  - Mappings file: ./fsct_roon_mappings.json
    - JSON persisted by mapping::Mappings. Auto-created if missing. Periodically saved every 60s while running.
  - Roon state file: ./fsct_roon_state.json
    - Saved/loaded by roon_api::RoonApi to restore Roon session state.
  - Extension identity (src/main.rs):
    - Roon Info: name "com.hem-e.fsct", display "Ferrum Streaming Control Technology™", vendor/email/URL set for this project.
  - FSCT connectivity:
    - Uses fsct_client::IpcDriver::connect(). Ensure FSCT host/IPC service is reachable on the local system.
  - Roon discovery:
    - Roon Core must be discoverable on the network for end-to-end manual tests. Unit tests do not require Roon/FSCT running.

Testing
- Running tests
  - Run: cargo test

- What’s covered by unit tests right now
  - src/conversions.rs: converts Roon states/zones to FSCT domain types.
  - src/mapping.rs: in-memory and persisted mapping operations (saves to a temp JSON file; cleans it up).
  - src/player_manager.rs: bookkeeping-only behaviors that don’t require a live driver.
  - src/state_cache.rs: timeline caching logic.
  - All of the above run in both the library and binary test harnesses (due to module layout).

- Adding new tests
  - Unit tests (preferred for most logic):
    - Place inside the relevant module: 
      #[cfg(test)]
      mod tests {
          use super::*;
          #[test]
          fn behavior_is_correct() {
              // Assert on pure logic or structs; avoid Roon/FSCT I/O here.
          }
      }
  - Integration tests (when cross-module behavior matters):
    - Create tests/*.rs with #[tokio::test] for async flows.
    - Prefer mocking traits from fsct::FsctDriver to avoid touching a real FSCT service. If adding such tests, introduce a thin mock implementing FsctDriver in the test module/crate.
  - I/O boundaries:
    - Avoid hitting the real FSCT IPC or Roon Core in tests. Constrain tests to pure transformations (e.g., conversions, mapping, caching). Persist only to temp files and remove them after assertions (see mapping::tests::test_save_load).

Guidelines for development
- Code style and structure
  - Rust 2021 idioms with module-per-domain (conversions, mapping, player_manager, settings, state_cache, zone_handler). Keep runtime side effects at boundaries (main/handlers). Prefer pure functions inside conversions and mapping for testability.
  - Logging via env_logger; default level is controlled by RUST_LOG. Add targeted log::debug! in hot paths; keep log::info! for lifecycle events.
  - Async runtime: Tokio, minimal feature set (macros, rt). Avoid spawning long-lived, unbounded tasks; reuse existing handlers.spawn in roon_api integration.
  - Serialization: serde/serde_json for persistence. Maintain backward compatibility of fsct_roon_mappings.json where possible.

- Roon settings UI
  - settings::make_layout builds a dynamic UI whose dropdowns are populated by available FSCT devices (queried from the driver) and available Roon outputs (tracked via Parsed::Outputs/OutputsRemoved). When adding fields:
    - Extend ExtensionSettings (serde Serialize/Deserialize), and update mappings_to_settings/settings_to_mappings.
    - Keep layout rendering cheap; calls may occur on UI open and on changes.

- Mapping lifecycle
  - handle_mapping_changes in main.rs coordinates unregister/register when mappings change, and replays cached state to the new mapping when available. When evolving this logic:
    - Always unregister before re-registering on remap.
    - Pull cached timeline from StateCache and push via driver.update_player_state after a successful register.

- State caching
  - StateCache stores per-output PlayerState/TimelineInfo to enable fast replay on (re)registration and on seek-only updates.
  - conversions::{convert_zone_to_timeline_info, build_timeline_from_seek} are designed to be side-effect free; favor extending them with pure inputs/outputs to remain testable.

- Error handling
  - anyhow for top-level errors; log and continue where possible (I/O and mapping persistence). Prefer bubbling errors at driver boundaries and swallow/log on UI refresh paths.

- Windows-specific notes
  - The repo includes build.ps1 and test.ps1 that hardcode short path versions of CARGO_HOME/RUSTUP_HOME to C:\Users\PAWEGO~1. If this does not match your environment, run plain cargo build/test or adjust the scripts.

Useful commands
- Build: cargo build
- Run (with logs): set RUST_LOG=info; cargo run
- Test (unit): cargo test
- Clean: cargo clean

Housekeeping
- JSON persistence files in the repo root (fsct_roon_mappings.json, fsct_roon_state.json) are part of the runtime state. Tests should not modify these; use temporary files under target/ or the working dir and delete them afterward (see mapping tests).

Changelog for developer onboarding
- 2025-10-08: Fixed unit test expectation in src/conversions.rs for State::Loading -> FsctStatus::Buffering. Test suite verified green with cargo test on Windows via test.ps1.
