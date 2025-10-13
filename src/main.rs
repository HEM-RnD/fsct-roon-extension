mod conversions;
mod mapping;
mod output_manager;
mod player_manager;
mod settings;
mod state_cache;
mod zone_handler;

use anyhow::Result;
use fsct::FsctDriver;
use fsct_client::IpcDriver;
use mapping::Mappings;
use output_manager::OutputManager;
use player_manager::PlayerManager;
use roon_api::{settings as roon_settings, CoreEvent, Info, Parsed, RoonApi, Services, Svc};
use settings::{make_layout, mappings_to_settings, settings_to_mappings, ExtensionSettings};
use state_cache::StateCache;
use std::collections::HashMap;
use std::sync::Arc;
use zone_handler::ZoneEventHandler;

const MAPPINGS_FILE: &str = "./fsct_roon_mappings.json";
const ROON_STATE_FILE: &str = "./fsct_roon_state.json";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("=== FSCT-Roon Extension Starting ===");

    // Load mappings
    let mappings = Arc::new(Mappings::load(MAPPINGS_FILE).await?);
    log::info!("Loaded mappings");

    // Connect to FSCT driver
    let driver = Arc::new(IpcDriver::connect().await?);
    log::info!("Connected to FSCT driver");

    // Player manager and state cache
    let player_manager = Arc::new(PlayerManager::new());
    let state_cache = Arc::new(StateCache::new());
    log::info!("Player manager and state cache initialized");

    // Output manager
    let output_manager = Arc::new(OutputManager::new(
        mappings.clone(),
        player_manager.clone(),
        state_cache.clone(),
        driver.clone(),
    ));
    log::info!("Output manager initialized");

    // Zone event handler
    let zone_handler = Arc::new(ZoneEventHandler::new(
        player_manager.clone(),
        state_cache.clone(),
        driver.clone(),
    ));

    // Initialize Roon API
    let mut roon = RoonApi::new(Info::new(
        "com.hem-e.fsct".to_string(),
        "Ferrum Streaming Control Technology™",
        "0.1.0",
        Some("HEM Sp. z o.o."),
        "info@hem-e.com",
        Some("https://github.com/HEM-RnD/fsct-roon-extension"),
    ));
    log::info!("Roon API initialized");

    // Setup settings callback
    let mappings_clone = mappings.clone();
    let driver_clone = driver.clone();
    let output_manager_for_layout = output_manager.clone();

    let get_layout = move |settings: Option<ExtensionSettings>| {
        // Get available FSCT devices
        let devices = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                 driver_clone
                    .get_detected_devices()
                    .await
                     .map_err(|e| log::error!("Error getting detected devices: {}", e))
                    .unwrap_or_default()
            })
        });

        // Get current mappings
        let current_mappings = mappings_clone.all();

        // Get available outputs from output manager
        let outputs_vec: Vec<roon_api::transport::Output> =
            output_manager_for_layout.get_available_outputs().values().cloned().collect();

        // Convert settings or create from current mappings
        let settings =
            settings.unwrap_or_else(|| mappings_to_settings(&current_mappings, &outputs_vec));

        make_layout(settings, devices, &current_mappings, outputs_vec)
    };

    let (svc, settings_service) = roon_settings::Settings::new(&roon, Box::new(get_layout));

    let services = Some(vec![
        Services::Transport(roon_api::transport::Transport::new()),
        Services::Settings(settings_service),
    ]);

    let provided: HashMap<String, Svc> = HashMap::from([(roon_settings::SVCNAME.to_owned(), svc)]);

    // Connection callback
    let on_connect = move || load_roon_state(ROON_STATE_FILE);

    // Start Roon discovery
    log::info!("Starting Roon discovery...");
    let result = roon
        .start_discovery(Box::new(on_connect), provided, services)
        .await;

    if let Some((mut handlers, mut core_rx)) = result {
        log::info!("Roon discovery started");

        // Clone for handlers
        let mappings_h = mappings.clone();
        let output_manager_h = output_manager.clone();
        let zone_handler_h = zone_handler.clone();

        // Roon message handler
        handlers.spawn(async move {
            loop {
                if let Some((core_event, msg)) = core_rx.recv().await {
                    handle_core_event(core_event).await;

                    if let Some((msg, parsed)) = msg {
                        handle_message(
                            msg,
                            parsed,
                            mappings_h.clone(),
                            output_manager_h.clone(),
                            zone_handler_h.clone(),
                        )
                        .await;
                    }
                }
            }
        });

        log::info!("Handlers spawned, running...");
        handlers.join_next().await;
    }

    log::info!("=== FSCT-Roon Extension Stopped ===");
    Ok(())
}

async fn handle_core_event(event: CoreEvent) {
    match event {
        CoreEvent::Discovered(_core, id) => {
            log::info!("Roon core discovered: {:?}", id);
        }
        CoreEvent::Registered(mut core, id) => {
            log::info!("Roon core registered: {:?}", id);
            // Subscribe to transport events
            if let Some(transport) = core.get_transport() {
                transport.subscribe_outputs().await;
                transport.subscribe_zones().await;
            }
        }
        CoreEvent::Lost(_core) => {
            log::warn!("Roon core connection lost");
        }
        CoreEvent::None => {}
    }
}

fn load_roon_state(file: &str) -> roon_api::RoonState {
    if !std::path::Path::new(file).exists() {
        log::info!("Roon state file not found, starting with empty state");
        return roon_api::RoonState::default();
    }
    std::fs::read_to_string(file).ok()
        .and_then(|content| {
            serde_json::from_str(&content).ok()}
        ).unwrap_or_default()
}
async fn save_roon_state(file: &str, state: &roon_api::RoonState) -> Result<()> {
    let content = serde_json::to_string_pretty(&state)?;
    tokio::fs::write(file, content).await?;
    log::debug!("Saved Roon state to {}", ROON_STATE_FILE);
    Ok(())
}

async fn handle_message(
    _msg: serde_json::Value,
    parsed: Parsed,
    mappings: Arc<Mappings>,
    output_manager: Arc<OutputManager<IpcDriver>>,
    zone_handler: Arc<ZoneEventHandler<IpcDriver>>,
) {
    match parsed {
        Parsed::RoonState(state) => {
            if let Err(e) = save_roon_state(ROON_STATE_FILE, &state).await {
                log::error!("Error saving Roon state: {}", e);
            }
        }
        Parsed::SettingsSaved(settings_value) => {
            log::info!("Settings saved, updating mappings");

            // Deserialize settings
            if let Ok(settings) = serde_json::from_value::<ExtensionSettings>(settings_value) {
                // Convert settings to mappings
                let new_mappings = settings_to_mappings(&settings);

                output_manager.handle_mappings_changed(new_mappings).await;
                if let Err(e) = mappings.save(MAPPINGS_FILE).await {
                    log::error!("Error saving mappings: {}", e);
                }
            } else {
                log::error!("Failed to deserialize settings");
            }
        }
        Parsed::Outputs(outputs) => {
            log::debug!("Outputs changed: {:#?}", outputs.len());
            output_manager.handle_outputs_changed(outputs).await;
        }
        Parsed::OutputsRemoved(removed_outputs) => {
            log::debug!("Outputs removed: {:#?}", removed_outputs);
            output_manager.handle_outputs_removed(removed_outputs).await;
        }
        Parsed::Zones(zones) => {
            log::debug!("Zones changed: {:#?}", zones);
            for zone in zones {
                zone_handler.handle_zone_changed(zone).await;
            }
        }
        Parsed::ZonesSeek(zones) => {
            log::debug!("Zones seek: {:#?}", zones);
            for zone in zones {
                zone_handler.handle_zone_seek(zone).await;
            }
        }
        _ => {}
    }
}

