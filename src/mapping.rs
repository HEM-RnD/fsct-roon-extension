use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;
use tokio::fs;

/// Internal data structure for serialization
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MappingsData {
    map: HashMap<String, Uuid>,
}

/// Stores mappings between Roon output IDs and FSCT device UUIDs
/// Uses interior mutability to allow concurrent access without external locking
pub struct Mappings {
    /// output_id -> device_uuid
    data: Mutex<MappingsData>,
}

impl Mappings {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(MappingsData { map: HashMap::new() }),
        }
    }

    /// Load mappings from file
    pub async fn load(path: &str) -> Result<Self> {
        if !Path::new(path).exists() {
            log::info!("Mappings file not found, starting with empty mappings");
            return Ok(Self::new());
        }

        let content = fs::read_to_string(path).await?;
        let mappings_data: MappingsData = serde_json::from_str(&content)?;
        log::info!("Loaded {} mappings from {}", mappings_data.map.len(), path);
        Ok(Self {
            data: Mutex::new(mappings_data),
        })
    }

    /// Save mappings to file
    pub async fn save(&self, path: &str) -> Result<()> {
        let (content, count) = {
            let data = self.data.lock().unwrap();
            let content = serde_json::to_string_pretty(&*data)?;
            (content, data.map.len())
        };
        fs::write(path, content).await?;
        log::debug!("Saved {} mappings to {}", count, path);
        Ok(())
    }

    /// Set mapping for output
    pub fn set(&self, output_id: String, device_uuid: Uuid) {
        self.data.lock().unwrap().map.insert(output_id, device_uuid);
    }

    /// Remove mapping for output
    #[allow(dead_code)]
    pub fn remove(&self, output_id: &str) -> bool {
        self.data.lock().unwrap().map.remove(output_id).is_some()
    }

    /// Get device UUID for output
    pub fn get(&self, output_id: &str) -> Option<Uuid> {
        self.data.lock().unwrap().map.get(output_id).copied()
    }

    /// Get all mappings (returns a clone)
    pub fn all(&self) -> HashMap<String, Uuid> {
        self.data.lock().unwrap().map.clone()
    }

    /// Check if output has mapping
    #[allow(dead_code)]
    pub fn has(&self, output_id: &str) -> bool {
        self.data.lock().unwrap().map.contains_key(output_id)
    }

    /// Clear all mappings
    pub fn clear(&self) {
        self.data.lock().unwrap().map.clear();
    }
}

impl Default for Mappings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_operations() {
        let mappings = Mappings::new();
        let uuid = Uuid::new_v4();

        mappings.set("output1".to_string(), uuid);
        assert_eq!(mappings.get("output1"), Some(uuid));
        assert!(mappings.has("output1"));

        assert!(mappings.remove("output1"));
        assert_eq!(mappings.get("output1"), None);
        assert!(!mappings.has("output1"));
    }

    #[tokio::test]
    async fn test_save_load() {
        let temp_file = "./test_mappings_temp.json";
        let mappings = Mappings::new();
        let uuid = Uuid::new_v4();

        mappings.set("output1".to_string(), uuid);
        mappings.save(temp_file).await.unwrap();

        let loaded = Mappings::load(temp_file).await.unwrap();
        assert_eq!(loaded.get("output1"), Some(uuid));

        let _ = fs::remove_file(temp_file);
    }

    #[tokio::test]
    async fn test_mappings_saved_after_change() {
        // Test that verifies mappings are persisted after modification
        let temp_file = "./test_mappings_after_change.json";

        // Start with empty mappings
        let mappings = Mappings::new();

        // Simulate settings change - add multiple mappings
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        let uuid3 = Uuid::new_v4();

        mappings.set("output_zone1".to_string(), uuid1);
        mappings.set("output_zone2".to_string(), uuid2);
        mappings.set("output_zone3".to_string(), uuid3);

        // Save after settings change (this is what happens in main.rs after SettingsSaved)
        mappings.save(temp_file).await.unwrap();

        // Verify file exists and contains correct data
        assert!(std::path::Path::new(temp_file).exists(), "Mappings file should exist after save");

        // Load from file to verify persistence
        let loaded = Mappings::load(temp_file).await.unwrap();
        assert_eq!(loaded.get("output_zone1"), Some(uuid1));
        assert_eq!(loaded.get("output_zone2"), Some(uuid2));
        assert_eq!(loaded.get("output_zone3"), Some(uuid3));

        // Simulate another settings change - modify existing mapping
        let mappings2 = loaded;
        let new_uuid = Uuid::new_v4();
        mappings2.set("output_zone2".to_string(), new_uuid);
        mappings2.save(temp_file).await.unwrap();

        // Verify the change was persisted
        let loaded2 = Mappings::load(temp_file).await.unwrap();
        assert_eq!(loaded2.get("output_zone1"), Some(uuid1));
        assert_eq!(loaded2.get("output_zone2"), Some(new_uuid)); // Changed value
        assert_eq!(loaded2.get("output_zone3"), Some(uuid3));

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }

    #[tokio::test]
    async fn test_clear_and_save() {
        // Test that clearing mappings (e.g., unmapping all) persists correctly
        let temp_file = "./test_mappings_clear.json";

        // Create mappings with some data
        let mappings = Mappings::new();
        mappings.set("output1".to_string(), Uuid::new_v4());
        mappings.set("output2".to_string(), Uuid::new_v4());
        mappings.save(temp_file).await.unwrap();

        // Clear all mappings (simulates user unmapping everything)
        mappings.clear();
        mappings.save(temp_file).await.unwrap();

        // Verify empty mappings were saved
        let loaded = Mappings::load(temp_file).await.unwrap();
        assert_eq!(loaded.all().len(), 0, "Mappings should be empty after clear and save");

        // Cleanup
        let _ = fs::remove_file(temp_file);
    }
}
