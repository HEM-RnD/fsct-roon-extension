use fsct::{PlayerState, TimelineInfo};
use std::collections::HashMap;
use std::sync::Mutex;

/// Caches player state for efficient updates
/// Uses interior mutability to allow concurrent access without external locking
pub struct StateCache {
    /// output_id -> full PlayerState - cached state for remapping and seek updates
    output_states: Mutex<HashMap<String, PlayerState>>,
}

impl StateCache {
    pub fn new() -> Self {
        Self {
            output_states: Mutex::new(HashMap::new()),
        }
    }

    /// Get timeline info for an output (from cached PlayerState)
    pub fn get_timeline(&self, output_id: &str) -> Option<TimelineInfo> {
        self.output_states
            .lock()
            .unwrap()
            .get(output_id)
            .and_then(|state| state.timeline.clone())
    }

    /// Update timeline info for an output (modifies timeline in existing PlayerState)
    pub fn update_timeline(&self, output_id: &str, timeline: &TimelineInfo) {
        let mut states = self.output_states.lock().unwrap();
        if let Some(state) = states.get_mut(output_id) {
            state.timeline = Some(timeline.clone());
        } else {
            let mut player_state = PlayerState::default();
            player_state.timeline = Some(timeline.clone());
            states.insert(output_id.to_string(), player_state);
        }
    }

    /// Save full player state for an output (for remapping and state tracking)
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

    /// Clear all cached states
    #[allow(dead_code)]
    pub fn clear(&self) {
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
    use fsct::{FsctStatus, TrackMetadata};
    use std::time::{Duration, SystemTime};

    #[test]
    fn test_output_state_cache() {
        let cache = StateCache::new();
        let output_id = "output1";

        let timeline = TimelineInfo {
            position: Duration::from_secs(10),
            update_time: SystemTime::now(),
            duration: Duration::from_secs(300),
            rate: 1.0,
        };

        let state = PlayerState {
            status: FsctStatus::Playing,
            timeline: Some(timeline.clone()),
            texts: TrackMetadata::default(),
        };

        // Save full state
        cache.save_output_state(output_id.to_string(), state);

        // Get timeline from state
        assert!(cache.get_timeline(output_id).is_some());
        assert_eq!(cache.get_timeline(output_id).unwrap().position, Duration::from_secs(10));

        // Get full state
        assert!(cache.get_output_state(output_id).is_some());

        // Remove state
        cache.remove_output_state(output_id);
        assert!(cache.get_output_state(output_id).is_none());
        assert!(cache.get_timeline(output_id).is_none());
    }

    #[test]
    fn test_update_timeline() {
        let cache = StateCache::new();
        let output_id = "output1";

        let initial_timeline = TimelineInfo {
            position: Duration::from_secs(10),
            update_time: SystemTime::now(),
            duration: Duration::from_secs(300),
            rate: 1.0,
        };

        let state = PlayerState {
            status: FsctStatus::Playing,
            timeline: Some(initial_timeline.clone()),
            texts: TrackMetadata::default(),
        };

        // Save initial state
        cache.save_output_state(output_id.to_string(), state);

        // Update timeline
        let updated_timeline = TimelineInfo {
            position: Duration::from_secs(20),
            update_time: SystemTime::now(),
            duration: Duration::from_secs(300),
            rate: 1.0,
        };
        cache.update_timeline(output_id, &updated_timeline);

        // Verify timeline was updated
        let retrieved = cache.get_timeline(output_id).unwrap();
        assert_eq!(retrieved.position, Duration::from_secs(20));

        // Verify full state has updated timeline
        let full_state = cache.get_output_state(output_id).unwrap();
        assert_eq!(full_state.timeline.unwrap().position, Duration::from_secs(20));
    }

    #[test]
    fn test_update_timeline_nonexistent_output() {
        let cache = StateCache::new();
        let output_id = "nonexistent";

        let timeline = TimelineInfo {
            position: Duration::from_secs(10),
            update_time: SystemTime::now(),
            duration: Duration::from_secs(300),
            rate: 1.0,
        };

        // Updating timeline for non-existent output should not panic
        cache.update_timeline(output_id, &timeline);
        assert_eq!(cache.get_timeline(output_id), Some(timeline));
    }
}
