use anyhow::Result;
use fsct::{FsctDriver, ManagedPlayerId};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// Tracks active FSCT players for Roon outputs
/// Uses interior mutability to allow concurrent access without external locking
pub struct PlayerManager {
    /// output_id -> (player_id, device_uuid)
    players: Mutex<HashMap<String, (ManagedPlayerId, Uuid)>>,
    /// zone_id -> Vec<output_id> - tracks which outputs belong to which zone
    zone_outputs: Mutex<HashMap<String, Vec<String>>>,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            players: Mutex::new(HashMap::new()),
            zone_outputs: Mutex::new(HashMap::new()),
        }
    }

    /// Register a player for a Roon output and assign to FSCT device
    pub async fn register(
        &self,
        driver: &impl FsctDriver,
        output_id: String,
        device_uuid: Uuid,
    ) -> Result<ManagedPlayerId> {
        // Check if already registered
        {
            let players = self.players.lock().unwrap();
            if let Some((player_id, _)) = players.get(&output_id) {
                log::debug!(
                    "Player already registered for output {}: {:?}",
                    output_id, player_id
                );
                return Ok(*player_id);
            }
        }

        // Register player with FSCT driver
        let player_name = format!("roon-{}", output_id);
        let player_id = driver.register_player(player_name).await?;
        log::info!("Registered player {:?} for output {}", player_id, output_id);

        // Assign player to device
        driver
            .assign_player_to_device(player_id, device_uuid)
            .await?;
        log::info!(
            "Assigned player {:?} to device {}",
            player_id, device_uuid
        );

        // Track the player
        self.players.lock().unwrap().insert(output_id.clone(), (player_id, device_uuid));

        Ok(player_id)
    }

    /// Unregister a player for a Roon output
    pub async fn unregister(
        &self,
        driver: &impl FsctDriver,
        output_id: &str,
    ) -> Result<()> {
        let player_info = self.players.lock().unwrap().remove(output_id);

        if let Some((player_id, device_uuid)) = player_info {
            log::info!("Unregistering player {:?} for output {}", player_id, output_id);

            // Unassign from device
            driver
                .unassign_player_from_device(player_id, device_uuid)
                .await?;

            // Unregister player
            driver.unregister_player(player_id).await?;

            log::info!("Unregistered player {:?}", player_id);
        }

        Ok(())
    }

    /// Get player ID for output
    pub fn get_player(&self, output_id: &str) -> Option<ManagedPlayerId> {
        self.players.lock().unwrap().get(output_id).map(|(id, _)| *id)
    }

    /// Check if output has registered player
    pub fn has_player(&self, output_id: &str) -> bool {
        self.players.lock().unwrap().contains_key(output_id)
    }

    /// Get all registered output IDs
    pub fn registered_outputs(&self) -> Vec<String> {
        self.players.lock().unwrap().keys().cloned().collect()
    }

    /// Get count of registered players
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.players.lock().unwrap().len()
    }

    /// Update zone mapping - track which outputs belong to which zone
    pub fn update_zone_mapping(&self, zone_id: String, output_ids: Vec<String>) {
        self.zone_outputs.lock().unwrap().insert(zone_id, output_ids);
    }

    /// Get all output IDs for a zone
    pub fn get_outputs_for_zone(&self, zone_id: &str) -> Vec<String> {
        let zone_outputs = self.zone_outputs.lock().unwrap();
        zone_outputs
            .get(zone_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all player IDs for outputs in a zone
    #[allow(dead_code)]
    pub fn get_players_for_zone(&self, zone_id: &str) -> Vec<ManagedPlayerId> {
        let zone_outputs = self.zone_outputs.lock().unwrap();
        let players = self.players.lock().unwrap();

        zone_outputs
            .get(zone_id)
            .map(|output_ids| {
                output_ids
                    .iter()
                    .filter_map(|output_id| players.get(output_id).map(|(id, _)| *id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_tracking() {
        let manager = PlayerManager::new();
        let _uuid = Uuid::new_v4();

        // Can't test actual registration without mock driver
        assert!(!manager.has_player("output1"));
        assert_eq!(manager.count(), 0);
    }
}
