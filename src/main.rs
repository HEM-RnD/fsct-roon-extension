mod conversions;
mod mapping;
mod player_manager;
mod settings;
mod state_cache;
mod zone_handler;

use anyhow::Result;
use fsct::FsctDriver;
use fsct_client::IpcDriver;
use mapping::Mappings;
use player_manager::PlayerManager;
use roon_api::{settings as roon_settings, CoreEvent, Info, Parsed, RoonApi, Services, Svc};
use settings::{make_layout, mappings_to_settings, settings_to_mappings, ExtensionSettings};
use state_cache::StateCache;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use zone_handler::ZoneEventHandler;

const MAPPINGS_FILE: &str = "./fsct_roon_mappings.json";
const ROON_STATE_FILE: &str = "./fsct_roon_state.json";

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("=== FSCT-Roon Extension Starting ===");

    // Load mappings
    let mappings = Arc::new(RwLock::new(Mappings::load(MAPPINGS_FILE)?));
    log::info!("Loaded mappings");

    // Connect to FSCT driver
    let driver = Arc::new(IpcDriver::connect().await?);
    log::info!("Connected to FSCT driver");

    // Player manager and state cache
    let player_manager = Arc::new(RwLock::new(PlayerManager::new()));
    let state_cache = Arc::new(RwLock::new(StateCache::new()));
    log::info!("Player manager and state cache initialized");

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
    let available_outputs_clone = Arc::new(RwLock::new(Vec::new()));
    let available_outputs_for_layout = available_outputs_clone.clone();

    let get_layout = move |settings: Option<ExtensionSettings>| {
        // Get available FSCT devices
        let devices = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                driver_clone
                    .get_detected_devices()
                    .await
                    .unwrap_or_default()
            })
        });

        // Get current mappings
        let current_mappings = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { mappings_clone.read().await.all().clone() })
        });

        // Get available outputs
        let outputs = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { available_outputs_for_layout.read().await.clone() })
        });

        // Convert settings or create from current mappings
        let settings =
            settings.unwrap_or_else(|| mappings_to_settings(&current_mappings, &outputs));

        make_layout(settings, devices, &current_mappings, outputs)
    };

    let (svc, settings_service) = roon_settings::Settings::new(&roon, Box::new(get_layout));

    let services = Some(vec![
        Services::Transport(roon_api::transport::Transport::new()),
        Services::Settings(settings_service),
    ]);

    let provided: HashMap<String, Svc> = HashMap::from([(roon_settings::SVCNAME.to_owned(), svc)]);

    // Connection callback
    let on_connect = move || RoonApi::load_roon_state(ROON_STATE_FILE);

    // Start Roon discovery
    log::info!("Starting Roon discovery...");
    let result = roon
        .start_discovery(Box::new(on_connect), provided, services)
        .await;

    if let Some((mut handlers, mut core_rx)) = result {
        log::info!("Roon discovery started");

        // Clone for handlers
        let mappings_h = mappings.clone();
        let driver_h = driver.clone();
        let player_manager_h = player_manager.clone();
        let available_outputs_h = available_outputs_clone.clone();
        let zone_handler_h = zone_handler.clone();
        let state_cache_h = state_cache.clone();

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
                            driver_h.clone(),
                            player_manager_h.clone(),
                            available_outputs_h.clone(),
                            zone_handler_h.clone(),
                            state_cache_h.clone(),
                        )
                        .await;
                    }
                }
            }
        });

        // Periodic save mappings
        let mappings_save = mappings.clone();
        handlers.spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let mappings = mappings_save.read().await;
                if let Err(e) = mappings.save(MAPPINGS_FILE) {
                    log::error!("Error saving mappings: {}", e);
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

async fn handle_message(
    _msg: serde_json::Value,
    parsed: Parsed,
    mappings: Arc<RwLock<Mappings>>,
    driver: Arc<IpcDriver>,
    player_manager: Arc<RwLock<PlayerManager>>,
    available_outputs: Arc<RwLock<Vec<roon_api::transport::Output>>>,
    zone_handler: Arc<ZoneEventHandler<IpcDriver>>,
    state_cache: Arc<RwLock<StateCache>>,
) {
    match parsed {
        Parsed::RoonState(state) => {
            if let Err(e) = RoonApi::save_roon_state(ROON_STATE_FILE, state) {
                log::error!("Error saving Roon state: {}", e);
            }
        }
        Parsed::SettingsSaved(settings_value) => {
            log::info!("Settings saved, updating mappings");

            // Deserialize settings
            if let Ok(settings) = serde_json::from_value::<ExtensionSettings>(settings_value) {
                // Convert settings to mappings
                let new_mappings = settings_to_mappings(&settings);

                // Get old mappings before updating
                let old_mappings = {
                    let mappings_read = mappings.read().await;
                    mappings_read.all().clone()
                };

                // Update mappings
                {
                    let mut mappings_write = mappings.write().await;
                    mappings_write.clear();
                    for (output_id, device_uuid) in &new_mappings {
                        mappings_write.set(output_id.clone(), *device_uuid);
                    }

                    // Save mappings immediately
                    if let Err(e) = mappings_write.save(MAPPINGS_FILE) {
                        log::error!("Error saving mappings: {}", e);
                    }
                }

                // Handle mapping changes: unregister old players, register new ones
                handle_mapping_changes(
                    old_mappings,
                    new_mappings,
                    driver.clone(),
                    player_manager.clone(),
                    state_cache.clone(),
                )
                .await;
            } else {
                log::error!("Failed to deserialize settings");
            }
        }
        Parsed::Outputs(outputs) => {
            log::info!("Outputs changed: {} outputs", outputs.len());

            // Update available outputs for settings UI
            {
                let mut available_outputs_write = available_outputs.write().await;
                *available_outputs_write = outputs.clone();
            }

            handle_outputs_changed(outputs, mappings, driver, player_manager).await;
        }
        Parsed::Zones(zones) => {
            log::info!("Zones changed: {} zones", zones.len());
            for zone in zones {
                zone_handler.handle_zone_changed(zone).await;
            }
        }
        Parsed::ZonesSeek(zones) => {
            log::debug!("Zones seek: {} zones", zones.len());
            for zone in zones {
                zone_handler.handle_zone_seek(zone).await;
            }
        }
        _ => {}
    }
}

/// Handle changes in mappings - unregister old players, register new ones
async fn handle_mapping_changes(
    old_mappings: HashMap<String, uuid::Uuid>,
    new_mappings: HashMap<String, uuid::Uuid>,
    driver: Arc<IpcDriver>,
    player_manager: Arc<RwLock<PlayerManager>>,
    state_cache: Arc<RwLock<StateCache>>,
) {
    let mut pm = player_manager.write().await;
    let cache = state_cache.read().await;

    // Find outputs that were unmapped (in old but not in new)
    for (old_output_id, _old_device_uuid) in &old_mappings {
        if !new_mappings.contains_key(old_output_id) {
            log::info!(
                "Output {} was unmapped, unregistering player",
                old_output_id
            );
            if let Err(e) = pm.unregister(&*driver, old_output_id).await {
                log::error!("Error unregistering player for {}: {}", old_output_id, e);
            }
        }
    }

    // Find outputs with changed device mapping or newly mapped
    for (output_id, new_device_uuid) in &new_mappings {
        if let Some(old_device_uuid) = old_mappings.get(output_id) {
            if old_device_uuid != new_device_uuid {
                log::info!(
                    "Output {} remapped from device {} to {}, re-registering player",
                    output_id,
                    old_device_uuid,
                    new_device_uuid
                );
                // Unregister old player
                if let Err(e) = pm.unregister(&*driver, output_id).await {
                    log::error!("Error unregistering old player for {}: {}", output_id, e);
                }
                // Register new player
                if let Err(e) = pm
                    .register(&*driver, output_id.clone(), *new_device_uuid)
                    .await
                {
                    log::error!("Error registering new player for {}: {}", output_id, e);
                } else {
                    // Send cached state to newly registered player
                    if let Some(cached_state) = cache.get_output_state(output_id) {
                        if let Some(player_id) = pm.get_player(output_id) {
                            log::info!("Sending cached state to remapped player {:?}", player_id);
                            if let Err(e) = driver
                                .update_player_state(player_id, cached_state.clone())
                                .await
                            {
                                log::error!("Error sending cached state to remapped player: {}", e);
                            }
                        }
                    }
                }
            }
        } else {
            // New mapping - register player
            log::info!(
                "Output {} newly mapped to device {}, registering player",
                output_id,
                new_device_uuid
            );
            if let Err(e) = pm
                .register(&*driver, output_id.clone(), *new_device_uuid)
                .await
            {
                log::error!("Error registering player for {}: {}", output_id, e);
            } else {
                // Send cached state to newly registered player
                if let Some(cached_state) = cache.get_output_state(output_id) {
                    if let Some(player_id) = pm.get_player(output_id) {
                        log::info!(
                            "Sending cached state to newly mapped player {:?}",
                            player_id
                        );
                        if let Err(e) = driver
                            .update_player_state(player_id, cached_state.clone())
                            .await
                        {
                            log::error!("Error sending cached state to newly mapped player: {}", e);
                        }
                    }
                }
            }
        }
    }
}

async fn handle_outputs_changed(
    outputs: Vec<roon_api::transport::Output>,
    mappings: Arc<RwLock<Mappings>>,
    driver: Arc<IpcDriver>,
    player_manager: Arc<RwLock<PlayerManager>>,
) {
    let mappings_read = mappings.read().await;
    let mut pm = player_manager.write().await;

    // Get list of currently available output IDs
    let available_outputs: Vec<String> = outputs.iter().map(|o| o.output_id.clone()).collect();

    // Unregister players for outputs that are no longer available
    for registered_output in pm.registered_outputs() {
        if !available_outputs.contains(&registered_output) {
            log::info!(
                "Output {} no longer available, unregistering player",
                registered_output
            );
            if let Err(e) = pm.unregister(&*driver, &registered_output).await {
                log::error!(
                    "Error unregistering player for {}: {}",
                    registered_output,
                    e
                );
            }
        }
    }

    // Register players for newly available mapped outputs
    for output in outputs {
        if let Some(device_uuid) = mappings_read.get(&output.output_id) {
            if !pm.has_player(&output.output_id) {
                log::info!(
                    "Output {} is mapped to device {}, registering player",
                    output.output_id,
                    device_uuid
                );
                if let Err(e) = pm
                    .register(&*driver, output.output_id.clone(), device_uuid)
                    .await
                {
                    log::error!("Error registering player: {}", e);
                }
            }
        }
    }
}
