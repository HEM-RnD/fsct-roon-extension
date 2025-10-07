use anyhow::Result;
use fsct::{FsctDriver, ManagedPlayerId};
use std::collections::HashMap;
use uuid::Uuid;

/// Tracks active FSCT players for Roon outputs
pub struct PlayerManager {
    /// output_id -> (player_id, device_uuid)
    players: HashMap<String, (ManagedPlayerId, Uuid)>,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }

    /// Register a player for a Roon output and assign to FSCT device
    pub async fn register(
        &mut self,
        driver: &impl FsctDriver,
        output_id: String,
        device_uuid: Uuid,
    ) -> Result<ManagedPlayerId> {
        // Check if already registered
        if let Some((player_id, _)) = self.players.get(&output_id) {
            log::debug!(
                "Player already registered for output {}: {:?}",
                output_id, player_id
            );
            return Ok(*player_id);
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
        self.players.insert(output_id.clone(), (player_id, device_uuid));

        Ok(player_id)
    }

    /// Unregister a player for a Roon output
    pub async fn unregister(
        &mut self,
        driver: &impl FsctDriver,
        output_id: &str,
    ) -> Result<()> {
        if let Some((player_id, device_uuid)) = self.players.remove(output_id) {
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
        self.players.get(output_id).map(|(id, _)| *id)
    }

    /// Check if output has registered player
    pub fn has_player(&self, output_id: &str) -> bool {
        self.players.contains_key(output_id)
    }

    /// Get all registered output IDs
    pub fn registered_outputs(&self) -> Vec<String> {
        self.players.keys().cloned().collect()
    }

    /// Get count of registered players
    pub fn count(&self) -> usize {
        self.players.len()
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
