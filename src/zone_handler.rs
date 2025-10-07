use crate::conversions::{convert_zone_to_player_state, build_timeline_from_seek};
use crate::player_manager::PlayerManager;
use crate::state_cache::StateCache;
use fsct::FsctDriver;
use roon_api::transport::{Zone, ZoneSeek};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Handles Roon zone events and updates FSCT players
pub struct ZoneEventHandler<D: FsctDriver> {
    player_manager: Arc<RwLock<PlayerManager>>,
    state_cache: Arc<RwLock<StateCache>>,
    driver: Arc<D>,
}

impl<D: FsctDriver> ZoneEventHandler<D> {
    pub fn new(
        player_manager: Arc<RwLock<PlayerManager>>,
        state_cache: Arc<RwLock<StateCache>>,
        driver: Arc<D>,
    ) -> Self {
        Self {
            player_manager,
            state_cache,
            driver,
        }
    }

    /// Handle full zone state change (play, pause, track change, etc.)
    pub async fn handle_zone_changed(&self, zone: Zone) {
        // Update zone mapping
        {
            let mut pm = self.player_manager.write().await;
            let output_ids: Vec<String> = zone.outputs.iter()
                .map(|o| o.output_id.clone())
                .collect();
            pm.update_zone_mapping(zone.zone_id.clone(), output_ids);
        }

        let pm = self.player_manager.read().await;
        let mut cache = self.state_cache.write().await;

        // Update state for all outputs in this zone
        for output in &zone.outputs {
            if let Some(player_id) = pm.get_player(&output.output_id) {
                let player_state = convert_zone_to_player_state(&zone);

                // Save timeline if present for future seek updates
                if let Some(ref timeline) = player_state.timeline {
                    cache.save_timeline(player_id, timeline.clone());
                }

                log::debug!(
                    "Updating player {:?} for output {} - status: {:?}",
                    player_id, output.output_id, player_state.status
                );

                if let Err(e) = self.driver.update_player_state(player_id, player_state).await {
                    log::error!("Error updating player state: {}", e);
                }
            }
        }
    }

    /// Handle seek-only update (timeline position change)
    pub async fn handle_zone_seek(&self, zone_seek: ZoneSeek) {
        let pm = self.player_manager.read().await;
        let mut cache = self.state_cache.write().await;

        // Get all players for this zone
        let player_ids = pm.get_players_for_zone(&zone_seek.zone_id);

        if player_ids.is_empty() {
            log::debug!("No players found for zone {}, skipping seek update", zone_seek.zone_id);
            return;
        }

        if zone_seek.seek_position.is_none() {
            log::debug!("No seek position in ZoneSeek event for zone {}", zone_seek.zone_id);
            return;
        }

        log::debug!(
            "Updating timeline for {} players in zone {} - position: {}s",
            player_ids.len(),
            zone_seek.zone_id,
            zone_seek.seek_position.unwrap()
        );

        // Update timeline for all players in this zone
        for player_id in player_ids {
            // Get cached timeline and build updated timeline from seek
            let cached = cache.get_timeline(player_id);
            let timeline = build_timeline_from_seek(cached, &zone_seek);

            if let Some(timeline) = timeline {
                // Save updated timeline
                cache.save_timeline(player_id, timeline.clone());

                // Send update to driver
                if let Err(e) = self.driver.update_player_timeline(player_id, Some(timeline)).await {
                    log::error!("Error updating player timeline for {:?}: {}", player_id, e);
                }
            }
        }
    }
}
