use crate::mapping::Mappings;
use crate::player_manager::PlayerManager;
use crate::state_cache::StateCache;
use fsct::FsctDriver;
use roon_api::transport::Output;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Manages Roon outputs and their player registrations
///
/// Responsibilities:
/// - Track available Roon outputs
/// - Register/unregister FSCT players when outputs appear/disappear
/// - Handle mapping changes between outputs and devices
pub struct OutputManager<D: FsctDriver> {
    /// Currently available Roon outputs
    available_outputs: HashMap<String, Output>,
    /// Mappings between output IDs and device UUIDs
    mappings: Arc<RwLock<Mappings>>,
    /// FSCT player manager
    player_manager: Arc<RwLock<PlayerManager>>,
    /// State cache for preserving player states
    state_cache: Arc<RwLock<StateCache>>,
    /// FSCT driver for player operations
    driver: Arc<D>,
}

impl<D: FsctDriver> OutputManager<D> {
    pub fn new(
        mappings: Arc<RwLock<Mappings>>,
        player_manager: Arc<RwLock<PlayerManager>>,
        state_cache: Arc<RwLock<StateCache>>,
        driver: Arc<D>,
    ) -> Self {
        Self {
            available_outputs: HashMap::new(),
            mappings,
            player_manager,
            state_cache,
            driver,
        }
    }

    /// Get all currently available outputs (for settings UI)
    pub fn get_available_outputs(&self) -> &HashMap<String, Output> {
        &self.available_outputs
    }

    /// Handle Roon outputs changed event
    /// Updates available outputs and registers/unregisters players as needed
    pub async fn handle_outputs_changed(&mut self, outputs: Vec<Output>) {
        log::info!("Outputs changed: {} outputs", outputs.len());

        // Update available outputs
        for output in outputs {
            self.available_outputs.insert(output.output_id.clone(), output);
        }

        self.sync_players().await;
    }

    /// Handle Roon outputs removed event
    /// Removes outputs from tracking and unregisters associated players
    pub async fn handle_outputs_removed(&mut self, removed_output_ids: Vec<String>) {
        log::info!("Outputs removed: {:?}", removed_output_ids);

        // Remove from available outputs
        for output_id in removed_output_ids {
            self.available_outputs.remove(&output_id);
        }

        self.sync_players().await;
    }

    /// Handle mapping changes from settings
    /// Registers/unregisters/re-registers players based on new mappings
    pub async fn handle_mappings_changed(&mut self, new_mappings: HashMap<String, Uuid>) {
        log::info!("Mappings changed, updating player registrations");

        // Get old mappings
        let old_mappings = {
            let mappings_read = self.mappings.read().await;
            mappings_read.all().clone()
        };

        // Update mappings
        {
            let mut mappings_write = self.mappings.write().await;
            mappings_write.clear();
            for (output_id, device_uuid) in &new_mappings {
                mappings_write.set(output_id.clone(), *device_uuid);
            }
        }

        // Handle the changes
        self.handle_mapping_differences(old_mappings, new_mappings).await;
    }

    /// Synchronize player registrations with current outputs and mappings
    /// - Unregister players for outputs that are no longer available
    /// - Register players for newly available mapped outputs
    async fn sync_players(&self) {
        let mappings_read = self.mappings.read().await;
        let mut pm = self.player_manager.write().await;

        let available_output_ids: Vec<String> = self.available_outputs.keys().cloned().collect();

        // Unregister players for outputs that are no longer available
        for registered_output in pm.registered_outputs() {
            if !available_output_ids.contains(&registered_output) {
                log::info!(
                    "Output {} no longer available, unregistering player",
                    registered_output
                );
                if let Err(e) = pm.unregister(&*self.driver, &registered_output).await {
                    log::error!(
                        "Error unregistering player for {}: {}",
                        registered_output,
                        e
                    );
                }
            }
        }

        // Register players for newly available mapped outputs
        for output_id in available_output_ids {
            if let Some(device_uuid) = mappings_read.get(&output_id) {
                if !pm.has_player(&output_id) {
                    log::info!(
                        "Output {} is mapped to device {}, registering player",
                        output_id,
                        device_uuid
                    );
                    if let Err(e) = pm
                        .register(&*self.driver, output_id.clone(), device_uuid)
                        .await
                    {
                        log::error!("Error registering player for {}: {}", output_id, e);
                    }
                }
            }
        }
    }

    /// Handle differences between old and new mappings
    /// - Unmap: unregister player
    /// - Remap: unregister old player, register new player, restore state
    /// - New map: register player, restore state if available
    async fn handle_mapping_differences(
        &self,
        old_mappings: HashMap<String, Uuid>,
        new_mappings: HashMap<String, Uuid>,
    ) {
        let mut pm = self.player_manager.write().await;
        let cache = self.state_cache.read().await;

        // Find outputs that were unmapped (in old but not in new)
        for (old_output_id, _old_device_uuid) in &old_mappings {
            if !new_mappings.contains_key(old_output_id) {
                log::info!(
                    "Output {} was unmapped, unregistering player",
                    old_output_id
                );
                if let Err(e) = pm.unregister(&*self.driver, old_output_id).await {
                    log::error!("Error unregistering player for {}: {}", old_output_id, e);
                }
            }
        }

        // Find outputs with changed device mapping or newly mapped
        for (output_id, new_device_uuid) in &new_mappings {
            if let Some(old_device_uuid) = old_mappings.get(output_id) {
                if old_device_uuid != new_device_uuid {
                    // Remapped to different device
                    log::info!(
                        "Output {} remapped from device {} to {}, re-registering player",
                        output_id,
                        old_device_uuid,
                        new_device_uuid
                    );

                    // Unregister old player
                    if let Err(e) = pm.unregister(&*self.driver, output_id).await {
                        log::error!("Error unregistering old player for {}: {}", output_id, e);
                    }

                    // Register new player
                    if let Err(e) = pm
                        .register(&*self.driver, output_id.clone(), *new_device_uuid)
                        .await
                    {
                        log::error!("Error registering new player for {}: {}", output_id, e);
                    } else {
                        // Send cached state to newly registered player
                        self.send_cached_state_to_player(&pm, &cache, output_id).await;
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
                    .register(&*self.driver, output_id.clone(), *new_device_uuid)
                    .await
                {
                    log::error!("Error registering player for {}: {}", output_id, e);
                } else {
                    // Send cached state to newly registered player
                    self.send_cached_state_to_player(&pm, &cache, output_id).await;
                }
            }
        }
    }

    /// Send cached state to a player (helper to avoid duplication)
    async fn send_cached_state_to_player(
        &self,
        pm: &PlayerManager,
        cache: &StateCache,
        output_id: &str,
    ) {
        if let Some(cached_state) = cache.get_output_state(output_id) {
            if let Some(player_id) = pm.get_player(output_id) {
                log::info!(
                    "Sending cached state to player {:?} for output {}",
                    player_id,
                    output_id
                );
                if let Err(e) = self
                    .driver
                    .update_player_state(player_id, cached_state.clone())
                    .await
                {
                    log::error!(
                        "Error sending cached state to player {:?}: {}",
                        player_id,
                        e
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsct::{DeviceInfo, ManagedPlayerId, PlayerState};
    use anyhow::Result;

    // Mock driver for testing
    struct MockDriver {
        registered_players: Arc<RwLock<Vec<String>>>,
        unregistered_players: Arc<RwLock<Vec<ManagedPlayerId>>>,
    }

    impl MockDriver {
        fn new() -> Self {
            Self {
                registered_players: Arc::new(RwLock::new(Vec::new())),
                unregistered_players: Arc::new(RwLock::new(Vec::new())),
            }
        }
    }

    #[async_trait::async_trait]
    impl FsctDriver for MockDriver {
        async fn register_player(&self, name: String) -> Result<ManagedPlayerId> {
            self.registered_players.write().await.push(name.clone());
            Ok(ManagedPlayerId::new(1).unwrap())
        }

        async fn unregister_player(&self, player_id: ManagedPlayerId) -> Result<()> {
            self.unregistered_players.write().await.push(player_id);
            Ok(())
        }

        async fn assign_player_to_device(
            &self,
            _player_id: ManagedPlayerId,
            _device_uuid: Uuid,
        ) -> Result<()> {
            Ok(())
        }

        async fn unassign_player_from_device(
            &self,
            _player_id: ManagedPlayerId,
            _device_uuid: Uuid,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_player_state(
            &self,
            _player_id: ManagedPlayerId,
            _state: PlayerState,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_player_status(
            &self,
            _player_id: ManagedPlayerId,
            _status: fsct::FsctStatus,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_player_timeline(
            &self,
            _player_id: ManagedPlayerId,
            _timeline: Option<fsct::TimelineInfo>,
        ) -> Result<()> {
            Ok(())
        }

        async fn update_player_metadata(
            &self,
            _player_id: ManagedPlayerId,
            _metadata: fsct::FsctTextMetadata,
            _image_url: Option<String>,
        ) -> Result<()> {
            Ok(())
        }

        async fn get_player_assigned_device(
            &self,
            _player_id: ManagedPlayerId,
        ) -> Result<Option<Uuid>> {
            Ok(None)
        }

        async fn get_detected_devices(&self) -> Result<Vec<DeviceInfo>> {
            Ok(Vec::new())
        }
    }

    fn create_test_output(output_id: &str, display_name: &str) -> Output {
        Output {
            output_id: output_id.to_string(),
            zone_id: "zone1".to_string(),
            can_group_with_output_ids: Vec::new(),
            display_name: display_name.to_string(),
            volume: None,
            source_controls: None,
        }
    }

    #[tokio::test]
    async fn test_handle_outputs_changed_tracks_outputs() {
        let mappings = Arc::new(RwLock::new(Mappings::new()));
        let player_manager = Arc::new(RwLock::new(PlayerManager::new()));
        let state_cache = Arc::new(RwLock::new(StateCache::new()));
        let driver = Arc::new(MockDriver::new());

        let mut output_manager = OutputManager::new(
            mappings,
            player_manager,
            state_cache,
            driver,
        );

        let outputs = vec![
            create_test_output("output1", "Output 1"),
            create_test_output("output2", "Output 2"),
        ];

        output_manager.handle_outputs_changed(outputs).await;

        assert_eq!(output_manager.get_available_outputs().len(), 2);
        assert!(output_manager.get_available_outputs().contains_key("output1"));
        assert!(output_manager.get_available_outputs().contains_key("output2"));
    }

    #[tokio::test]
    async fn test_handle_outputs_removed_removes_outputs() {
        let mappings = Arc::new(RwLock::new(Mappings::new()));
        let player_manager = Arc::new(RwLock::new(PlayerManager::new()));
        let state_cache = Arc::new(RwLock::new(StateCache::new()));
        let driver = Arc::new(MockDriver::new());

        let mut output_manager = OutputManager::new(
            mappings,
            player_manager,
            state_cache,
            driver,
        );

        // Add some outputs
        let outputs = vec![
            create_test_output("output1", "Output 1"),
            create_test_output("output2", "Output 2"),
        ];
        output_manager.handle_outputs_changed(outputs).await;

        // Remove one output
        output_manager.handle_outputs_removed(vec!["output1".to_string()]).await;

        assert_eq!(output_manager.get_available_outputs().len(), 1);
        assert!(!output_manager.get_available_outputs().contains_key("output1"));
        assert!(output_manager.get_available_outputs().contains_key("output2"));
    }

    #[tokio::test]
    async fn test_handle_mappings_changed_registers_new_players() {
        let mappings = Arc::new(RwLock::new(Mappings::new()));
        let player_manager = Arc::new(RwLock::new(PlayerManager::new()));
        let state_cache = Arc::new(RwLock::new(StateCache::new()));
        let driver = Arc::new(MockDriver::new());

        let mut output_manager = OutputManager::new(
            mappings.clone(),
            player_manager.clone(),
            state_cache,
            driver.clone(),
        );

        // Add output
        output_manager.handle_outputs_changed(vec![
            create_test_output("output1", "Output 1"),
        ]).await;

        // Create mapping
        let device_uuid = Uuid::new_v4();
        let mut new_mappings = HashMap::new();
        new_mappings.insert("output1".to_string(), device_uuid);

        output_manager.handle_mappings_changed(new_mappings).await;

        // Verify mapping was saved
        let mappings_read = mappings.read().await;
        assert_eq!(mappings_read.get("output1"), Some(device_uuid));

        // Verify player was registered
        let pm = player_manager.read().await;
        assert!(pm.has_player("output1"));
    }

    #[tokio::test]
    async fn test_handle_mappings_changed_unregisters_unmapped_players() {
        let mappings = Arc::new(RwLock::new(Mappings::new()));
        let player_manager = Arc::new(RwLock::new(PlayerManager::new()));
        let state_cache = Arc::new(RwLock::new(StateCache::new()));
        let driver = Arc::new(MockDriver::new());

        let mut output_manager = OutputManager::new(
            mappings.clone(),
            player_manager.clone(),
            state_cache,
            driver.clone(),
        );

        // Add output and create initial mapping
        output_manager.handle_outputs_changed(vec![
            create_test_output("output1", "Output 1"),
        ]).await;

        let device_uuid = Uuid::new_v4();
        let mut initial_mappings = HashMap::new();
        initial_mappings.insert("output1".to_string(), device_uuid);
        output_manager.handle_mappings_changed(initial_mappings).await;

        // Verify player was registered
        {
            let pm = player_manager.read().await;
            assert!(pm.has_player("output1"));
        }

        // Remove mapping (empty HashMap)
        output_manager.handle_mappings_changed(HashMap::new()).await;

        // Verify player was unregistered
        let pm = player_manager.read().await;
        assert!(!pm.has_player("output1"));
    }
}
