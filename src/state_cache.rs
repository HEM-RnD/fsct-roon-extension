use fsct::{ManagedPlayerId, PlayerState, TimelineInfo};
use std::collections::HashMap;
use std::sync::Mutex;

/// Caches player state for efficient updates
/// Uses interior mutability to allow concurrent access without external locking
pub struct StateCache {
    /// player_id -> last TimelineInfo - cached state for seek updates
    player_timelines: Mutex<HashMap<ManagedPlayerId, TimelineInfo>>,
    /// output_id -> full PlayerState - cached state for remapping
    output_states: Mutex<HashMap<String, PlayerState>>,
}

impl StateCache {
    pub fn new() -> Self {
        Self {
            player_timelines: Mutex::new(HashMap::new()),
            output_states: Mutex::new(HashMap::new()),
        }
    }

    /// Save timeline info for a player
    pub fn save_timeline(&self, player_id: ManagedPlayerId, timeline: TimelineInfo) {
        self.player_timelines.lock().unwrap().insert(player_id, timeline);
    }

    /// Get saved timeline info for a player
    pub fn get_timeline(&self, player_id: ManagedPlayerId) -> Option<TimelineInfo> {
        self.player_timelines.lock().unwrap().get(&player_id).cloned()
    }

    /// Remove timeline for a player (called when player is unregistered)
    #[allow(dead_code)]
    pub fn remove_timeline(&self, player_id: ManagedPlayerId) {
        self.player_timelines.lock().unwrap().remove(&player_id);
    }

    /// Save full player state for an output (for remapping)
    pub fn save_output_state(&self, output_id: String, state: PlayerState) {
        self.output_states.lock().unwrap().insert(output_id, state);
    }

    /// Get saved player state for an output
    pub fn get_output_state(&self, output_id: &str) -> Option<PlayerState> {
        self.output_states.lock().unwrap().get(output_id).cloned()
    }

    /// Remove state for an output
    #[allow(dead_code)]
    pub fn remove_output_state(&self, output_id: &str) {
        self.output_states.lock().unwrap().remove(output_id);
    }

    /// Clear all cached timelines
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.player_timelines.lock().unwrap().clear();
        self.output_states.lock().unwrap().clear();
    }
}

impl Default for StateCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_timeline_cache() {
        let mut cache = StateCache::new();
        let player_id = ManagedPlayerId::new(1).unwrap();

        let timeline = TimelineInfo {
            position: Duration::from_secs(10),
            update_time: SystemTime::now(),
            duration: Duration::from_secs(300),
            rate: 1.0,
        };

        cache.save_timeline(player_id, timeline.clone());
        assert!(cache.get_timeline(player_id).is_some());

        cache.remove_timeline(player_id);
        assert!(cache.get_timeline(player_id).is_none());
    }
}
