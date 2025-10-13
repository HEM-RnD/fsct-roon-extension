use crate::conversions::{convert_zone_to_player_state, build_timeline_from_seek};
use crate::player_manager::PlayerManager;
use crate::state_cache::StateCache;
use fsct::FsctDriver;
use roon_api::transport::{Zone, ZoneSeek};
use std::sync::Arc;

/// Handles Roon zone events and updates FSCT players
pub struct ZoneEventHandler<D: FsctDriver> {
    player_manager: Arc<PlayerManager>,
    state_cache: Arc<StateCache>,
    driver: Arc<D>,
}

impl<D: FsctDriver> ZoneEventHandler<D> {
    pub fn new(
        player_manager: Arc<PlayerManager>,
        state_cache: Arc<StateCache>,
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
        let output_ids: Vec<String> = zone.outputs.iter()
            .map(|o| o.output_id.clone())
            .collect();
        self.player_manager.update_zone_mapping(zone.zone_id.clone(), output_ids);

        // Collect updates for all outputs in this zone
        let mut player_updates = Vec::new();
        for output in &zone.outputs {
            let player_state = convert_zone_to_player_state(&zone);

            // Save full state for this output (includes timeline)
            self.state_cache.save_output_state(output.output_id.clone(), player_state.clone());

            if let Some(player_id) = self.player_manager.get_player(&output.output_id) {
                log::debug!(
                    "Updating player {:?} for output {} - status: {:?}",
                    player_id, output.output_id, player_state.status
                );

                player_updates.push((player_id, player_state));
            }
        }

        // Send updates to driver
        for (player_id, player_state) in player_updates {
            if let Err(e) = self.driver.update_player_state(player_id, player_state).await {
                log::error!("Error updating player state: {}", e);
            }
        }
    }

    /// Handle seek-only update (timeline position change)
    pub async fn handle_zone_seek(&self, zone_seek: ZoneSeek) {
        // Get all outputs for this zone
        let output_ids = self.player_manager.get_outputs_for_zone(&zone_seek.zone_id);

        if output_ids.is_empty() {
            log::debug!("No outputs found for zone {}, skipping seek update", zone_seek.zone_id);
            return;
        }

        if zone_seek.seek_position.is_none() {
            log::debug!("No seek position in ZoneSeek event for zone {}", zone_seek.zone_id);
            return;
        }

        log::debug!(
            "Updating timeline for {} outputs in zone {} - position: {}s",
            output_ids.len(),
            zone_seek.zone_id,
            zone_seek.seek_position.unwrap()
        );

        // Collect timeline updates for all outputs in this zone
        let mut timeline_updates = Vec::new();
        for output_id in output_ids {
            // Get cached timeline from output state and build updated timeline from seek
            let cached = self.state_cache.get_timeline(&output_id);
            let timeline = build_timeline_from_seek(cached.as_ref(), &zone_seek);

            if let Some(timeline) = timeline {
                // Update timeline in cached state
                self.state_cache.update_timeline(&output_id, &timeline);

                // If this output has a registered player, send the update
                if let Some(player_id) = self.player_manager.get_player(&output_id) {
                    timeline_updates.push((player_id, timeline));
                }
            }
        }

        // Send updates to driver
        for (player_id, timeline) in timeline_updates {
            if let Err(e) = self.driver.update_player_timeline(player_id, Some(timeline)).await {
                log::error!("Error updating player timeline for {:?}: {}", player_id, e);
            }
        }
    }
}
