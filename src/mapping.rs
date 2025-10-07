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
}
