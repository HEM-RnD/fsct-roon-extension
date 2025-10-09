use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Stores mappings between Roon output IDs and FSCT device UUIDs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Mappings {
    /// output_id -> device_uuid
    map: HashMap<String, Uuid>,
}

impl Mappings {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Load mappings from file
    pub fn load(path: &str) -> Result<Self> {
        if !Path::new(path).exists() {
            log::info!("Mappings file not found, starting with empty mappings");
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path)?;
        let mappings: Self = serde_json::from_str(&content)?;
        log::info!("Loaded {} mappings from {}", mappings.map.len(), path);
        Ok(mappings)
    }

    /// Save mappings to file
    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(&self)?;
        fs::write(path, content)?;
        log::debug!("Saved {} mappings to {}", self.map.len(), path);
        Ok(())
    }

    /// Set mapping for output
    pub fn set(&mut self, output_id: String, device_uuid: Uuid) {
        self.map.insert(output_id, device_uuid);
    }

    /// Remove mapping for output
    #[allow(dead_code)]
    pub fn remove(&mut self, output_id: &str) -> bool {
        self.map.remove(output_id).is_some()
    }

    /// Get device UUID for output
    pub fn get(&self, output_id: &str) -> Option<Uuid> {
        self.map.get(output_id).copied()
    }

    /// Get all mappings
    pub fn all(&self) -> &HashMap<String, Uuid> {
        &self.map
    }

    /// Check if output has mapping
    #[allow(dead_code)]
    pub fn has(&self, output_id: &str) -> bool {
        self.map.contains_key(output_id)
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_operations() {
        let mut mappings = Mappings::new();
        let uuid = Uuid::new_v4();

        mappings.set("output1".to_string(), uuid);
        assert_eq!(mappings.get("output1"), Some(uuid));
        assert!(mappings.has("output1"));

        assert!(mappings.remove("output1"));
        assert_eq!(mappings.get("output1"), None);
        assert!(!mappings.has("output1"));
    }

    #[test]
    fn test_save_load() {
        let temp_file = "./test_mappings_temp.json";
        let mut mappings = Mappings::new();
        let uuid = Uuid::new_v4();

        mappings.set("output1".to_string(), uuid);
        mappings.save(temp_file).unwrap();

        let loaded = Mappings::load(temp_file).unwrap();
        assert_eq!(loaded.get("output1"), Some(uuid));

        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_mappings_saved_after_change() {
        // Test that verifies mappings are persisted after modification
        let temp_file = "./test_mappings_after_change.json";

        // Start with empty mappings
        let mut mappings = Mappings::new();

        // Simulate settings change - add multiple mappings
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        let uuid3 = Uuid::new_v4();

        mappings.set("output_zone1".to_string(), uuid1);
        mappings.set("output_zone2".to_string(), uuid2);
        mappings.set("output_zone3".to_string(), uuid3);

        // Save after settings change (this is what happens in main.rs after SettingsSaved)
        mappings.save(temp_file).unwrap();

        // Verify file exists and contains correct data
        assert!(std::path::Path::new(temp_file).exists(), "Mappings file should exist after save");

        // Load from file to verify persistence
        let loaded = Mappings::load(temp_file).unwrap();
        assert_eq!(loaded.get("output_zone1"), Some(uuid1));
        assert_eq!(loaded.get("output_zone2"), Some(uuid2));
        assert_eq!(loaded.get("output_zone3"), Some(uuid3));

        // Simulate another settings change - modify existing mapping
        let mut mappings2 = loaded;
        let new_uuid = Uuid::new_v4();
        mappings2.set("output_zone2".to_string(), new_uuid);
        mappings2.save(temp_file).unwrap();

        // Verify the change was persisted
        let loaded2 = Mappings::load(temp_file).unwrap();
        assert_eq!(loaded2.get("output_zone1"), Some(uuid1));
        assert_eq!(loaded2.get("output_zone2"), Some(new_uuid)); // Changed value
        assert_eq!(loaded2.get("output_zone3"), Some(uuid3));

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_clear_and_save() {
        // Test that clearing mappings (e.g., unmapping all) persists correctly
        let temp_file = "./test_mappings_clear.json";

        // Create mappings with some data
        let mut mappings = Mappings::new();
        mappings.set("output1".to_string(), Uuid::new_v4());
        mappings.set("output2".to_string(), Uuid::new_v4());
        mappings.save(temp_file).unwrap();

        // Clear all mappings (simulates user unmapping everything)
        mappings.clear();
        mappings.save(temp_file).unwrap();

        // Verify empty mappings were saved
        let loaded = Mappings::load(temp_file).unwrap();
        assert_eq!(loaded.all().len(), 0, "Mappings should be empty after clear and save");

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }
}
