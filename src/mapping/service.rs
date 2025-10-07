use crate::settings::config::{DeviceMapping, DeviceMappings, FsctDevice, RoonOutput};
use parking_lot::RwLock;
use std::sync::Arc;

/// Service for managing device mappings with thread-safe access
pub struct MappingService {
    mappings: Arc<RwLock<DeviceMappings>>,
}

impl MappingService {
    pub fn new() -> Self {
        Self {
            mappings: Arc::new(RwLock::new(DeviceMappings::new())),
        }
    }

    pub fn from_mappings(mappings: DeviceMappings) -> Self {
        Self {
            mappings: Arc::new(RwLock::new(mappings)),
        }
    }

    /// Get a clone of current mappings for persistence
    pub fn get_mappings(&self) -> DeviceMappings {
        self.mappings.read().clone()
    }

    /// Find FSCT device by Roon output ID
    pub fn find_fsct_by_roon(&self, roon_output_id: &str) -> Option<FsctDevice> {
        self.mappings
            .read()
            .find_fsct_by_roon(roon_output_id)
            .cloned()
    }

    /// Find Roon output by FSCT device ID
    pub fn find_roon_by_fsct(&self, fsct_device_id: &str) -> Option<RoonOutput> {
        self.mappings
            .read()
            .find_roon_by_fsct(fsct_device_id)
            .cloned()
    }

    /// Add or update a mapping
    pub fn upsert(&self, mapping: DeviceMapping) {
        self.mappings.write().upsert(mapping);
    }

    /// Remove mapping by Roon output ID
    pub fn remove_by_roon(&self, roon_output_id: &str) -> bool {
        self.mappings.write().remove_by_roon(roon_output_id)
    }

    /// Update Roon output availability status
    pub fn update_roon_status(&self, roon_output_id: &str, is_available: bool) {
        self.mappings
            .write()
            .update_roon_status(roon_output_id, is_available);
    }

    /// Update FSCT device availability status
    pub fn update_fsct_status(&self, fsct_device_id: &str, is_available: bool) {
        self.mappings
            .write()
            .update_fsct_status(fsct_device_id, is_available);
    }

    /// Get all mappings as a vector
    pub fn list_all(&self) -> Vec<DeviceMapping> {
        self.mappings.read().mappings.clone()
    }
}

impl Default for MappingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_service_operations() {
        let service = MappingService::new();

        let mapping = DeviceMapping {
            roon_output: RoonOutput {
                output_id: "roon1".to_string(),
                display_name: "Room 1".to_string(),
            },
            fsct_device: FsctDevice {
                device_id: "fsct1".to_string(),
                display_name: "Device 1".to_string(),
            },
            is_active: true,
        };

        service.upsert(mapping.clone());

        assert!(service.find_fsct_by_roon("roon1").is_some());
        assert!(service.find_roon_by_fsct("fsct1").is_some());

        service.update_roon_status("roon1", false);
        let mappings = service.get_mappings();
        assert!(!mappings.mappings[0].is_active);

        assert!(service.remove_by_roon("roon1"));
        assert!(service.find_fsct_by_roon("roon1").is_none());
    }
}
