use crate::settings::config::DeviceMappings;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Handles persistence of device mappings to/from disk
pub struct MappingPersistence {
    file_path: String,
}

impl MappingPersistence {
    pub fn new(file_path: String) -> Self {
        Self { file_path }
    }

    /// Load mappings from disk. Returns empty mappings if file doesn't exist.
    pub fn load(&self) -> Result<DeviceMappings> {
        let path = Path::new(&self.file_path);

        if !path.exists() {
            println!("Mappings file not found at {}, starting with empty mappings", self.file_path);
            return Ok(DeviceMappings::new());
        }

        let content = fs::read_to_string(path)
            .context(format!("Failed to read mappings from {}", self.file_path))?;

        let mappings: DeviceMappings = serde_json::from_str(&content)
            .context(format!("Failed to parse mappings from {}", self.file_path))?;

        println!("Loaded {} device mappings from {}", mappings.mappings.len(), self.file_path);

        Ok(mappings)
    }

    /// Save mappings to disk
    pub fn save(&self, mappings: &DeviceMappings) -> Result<()> {
        let content = serde_json::to_string_pretty(mappings)
            .context("Failed to serialize mappings")?;

        fs::write(&self.file_path, content)
            .context(format!("Failed to write mappings to {}", self.file_path))?;

        println!("Saved {} device mappings to {}", mappings.mappings.len(), self.file_path);

        Ok(())
    }

    /// Check if mappings file exists
    pub fn exists(&self) -> bool {
        Path::new(&self.file_path).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::config::{DeviceMapping, FsctDevice, RoonOutput};
    use std::fs;

    #[test]
    fn test_save_and_load() {
        let temp_file = "./test_mappings.json";
        let persistence = MappingPersistence::new(temp_file.to_string());

        // Clean up if exists
        let _ = fs::remove_file(temp_file);

        let mut mappings = DeviceMappings::new();
        mappings.upsert(DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Room 1".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct1".to_string(),
                display_name: "Device 1".to_string(),
            },
            is_active: true,
        });

        persistence.save(&mappings).unwrap();
        assert!(persistence.exists());

        let loaded = persistence.load().unwrap();
        assert_eq!(loaded.mappings.len(), 1);
        assert_eq!(loaded.mappings[0].roon_output.output_id, "roon1");
        assert_eq!(loaded.mappings[0].fsct_device.device_id, "fsct1");

        // Clean up
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let persistence = MappingPersistence::new("./nonexistent_file.json".to_string());
        let result = persistence.load();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().mappings.len(), 0);
    }
}
