use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Path to store device mappings
    #[serde(default = "default_mappings_path")]
    pub mappings_path: String,

    /// Path to store Roon state
    #[serde(default = "default_roon_state_path")]
    pub roon_state_path: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            mappings_path: default_mappings_path(),
            roon_state_path: default_roon_state_path(),
        }
    }
}

fn default_mappings_path() -> String {
    "./mappings.json".to_string()
}

fn default_roon_state_path() -> String {
    "./roon_state.json".to_string()
}

/// Represents a Roon output device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RoonOutput {
    pub output_id: String,
    pub display_name: String,
}

/// Represents an FSCT device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FsctDevice {
    pub device_id: String,
    pub display_name: String,
}

/// Represents a mapping between Roon output and FSCT device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMapping {
    pub roon_output: RoonOutput,
    pub fsct_device: FsctDevice,

    /// Indicates if both devices are currently available
    #[serde(default)]
    pub is_active: bool,
}

/// Collection of all device mappings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceMappings {
    pub mappings: Vec<DeviceMapping>,
}

impl DeviceMappings {
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
        }
    }

    /// Find FSCT device by Roon output ID
    pub fn find_fsct_by_roon(&self, roon_output_id: &str) -> Option<&FsctDevice> {
        self.mappings
            .iter()
            .find(|m| m.roon_output.output_id == roon_output_id)
            .map(|m| &m.fsct_device)
    }

    /// Find Roon output by FSCT device ID
    pub fn find_roon_by_fsct(&self, fsct_device_id: &str) -> Option<&RoonOutput> {
        self.mappings
            .iter()
            .find(|m| m.fsct_device.device_id == fsct_device_id)
            .map(|m| &m.roon_output)
    }

    /// Add or update a mapping
    pub fn upsert(&mut self, mapping: DeviceMapping) {
        // Remove existing mapping for this Roon output if any
        self.mappings
            .retain(|m| m.roon_output.output_id != mapping.roon_output.output_id);

        self.mappings.push(mapping);
    }

    /// Remove mapping by Roon output ID
    pub fn remove_by_roon(&mut self, roon_output_id: &str) -> bool {
        let before = self.mappings.len();
        self.mappings
            .retain(|m| m.roon_output.output_id != roon_output_id);
        self.mappings.len() < before
    }

    /// Update active status for a Roon output
    pub fn update_roon_status(&mut self, roon_output_id: &str, is_available: bool) {
        if let Some(mapping) = self.mappings
            .iter_mut()
            .find(|m| m.roon_output.output_id == roon_output_id)
        {
            mapping.is_active = is_available;
        }
    }

    /// Update active status for an FSCT device
    pub fn update_fsct_status(&mut self, fsct_device_id: &str, is_available: bool) {
        if let Some(mapping) = self.mappings
            .iter_mut()
            .find(|m| m.fsct_device.device_id == fsct_device_id)
        {
            mapping.is_active = is_available;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_fsct_by_roon() {
        let mut mappings = DeviceMappings::new();
        mappings.upsert(DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Living Room".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct1".to_string(),
                display_name: "FSCT Device 1".to_string(),
            },
            is_active: true,
        });

        let device = mappings.find_fsct_by_roon("roon1");
        assert!(device.is_some());
        assert_eq!(device.unwrap().device_id, "fsct1");
    }

    #[test]
    fn test_upsert_replaces_existing() {
        let mut mappings = DeviceMappings::new();

        mappings.upsert(DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Room".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct1".to_string(),
                display_name: "Device 1".to_string(),
            },
            is_active: true,
        });

        mappings.upsert(DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Room".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct2".to_string(),
                display_name: "Device 2".to_string(),
            },
            is_active: true,
        });

        assert_eq!(mappings.mappings.len(), 1);
        assert_eq!(mappings.find_fsct_by_roon("roon1").unwrap().device_id, "fsct2");
    }

    #[test]
    fn test_remove_by_roon() {
        let mut mappings = DeviceMappings::new();

        mappings.upsert(DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Room".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct1".to_string(),
                display_name: "Device 1".to_string(),
            },
            is_active: true,
        });

        assert!(mappings.remove_by_roon("roon1"));
        assert_eq!(mappings.mappings.len(), 0);
        assert!(!mappings.remove_by_roon("roon1"));
    }
}
