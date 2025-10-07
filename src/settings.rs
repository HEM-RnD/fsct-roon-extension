use fsct::DeviceInfo;
use roon_api::settings::{Layout, Widget, Label};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Settings data structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExtensionSettings {
    /// Device mappings as list of device UUID + output ID pairs
    pub mappings: Vec<DeviceMapping>,
}

/// Single device-to-output mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMapping {
    pub device_uuid: String,
    pub device_id: String,
}

/// Create settings layout
/// Lists all FSCT devices (detected + previously mapped but not detected)
/// For each device, provides a dropdown with available Roon outputs
pub fn make_layout(
    settings: ExtensionSettings,
    available_devices: Vec<DeviceInfo>,
    current_mappings: &HashMap<String, Uuid>,
    available_outputs: Vec<roon_api::transport::Output>,
) -> Layout<ExtensionSettings> {
    let mut widgets: Vec<Widget> = Vec::new();

    // Collect all device UUIDs: detected + previously mapped
    let mut all_device_uuids = HashSet::new();

    // Add detected devices
    for device in &available_devices {
        all_device_uuids.insert(device.id);
    }

    // Add previously mapped devices (they might not be currently detected)
    for device_uuid in current_mappings.values() {
        all_device_uuids.insert(*device_uuid);
    }

    // Create a map of device UUID to name for display
    let mut device_names: HashMap<Uuid, String> = HashMap::new();
    for device in &available_devices {
        let name = device
            .name
            .as_ref()
            .map(|n| n.clone())
            .unwrap_or_else(|| format!("Device {}", device.id));
        device_names.insert(device.id, name);
    }

    // Sort devices for consistent display
    let mut sorted_devices: Vec<Uuid> = all_device_uuids.into_iter().collect();
    sorted_devices.sort();

    // Create reverse mapping from device_uuid to output_id from current mappings
    let device_to_output: HashMap<Uuid, String> = current_mappings
        .iter()
        .map(|(output_id, device_uuid)| (*device_uuid, output_id.clone()))
        .collect();

    if sorted_devices.is_empty() {
        // No devices available
        widgets.push(Widget::Label(Label {
            title: "No FSCT devices available".to_string(),
            subtitle: Some("Connect an FSCT device to continue".to_string()),
        }));
    } else {
        widgets.push(Widget::Label(Label {
            title: "FSCT Device Mappings".to_string(),
            subtitle: Some("Map each FSCT device to a Roon output".to_string()),
        }));

        // Note: This is a simplified UI - we'll show the current state
        // In a full implementation, we'd need a way to edit these mappings interactively
        // For now, we'll just display the current mappings as labels
        for device_uuid in sorted_devices {
            let device_name = device_names.get(&device_uuid).cloned().unwrap_or_else(|| {
                format!("Device {}", device_uuid)
            });

            let output_name = device_to_output
                .get(&device_uuid)
                .and_then(|output_id| {
                    available_outputs
                        .iter()
                        .find(|o| &o.output_id == output_id)
                        .map(|o| o.display_name.clone())
                })
                .unwrap_or_else(|| "Not mapped".to_string());

            widgets.push(Widget::Label(Label {
                title: format!("{} → {}", device_name, output_name),
                subtitle: None,
            }));
        }

        widgets.push(Widget::Label(Label {
            title: String::new(),
            subtitle: Some("Note: To change mappings, use the command-line interface or config file".to_string()),
        }));
    }

    Layout {
        settings,
        widgets,
        has_error: false,
    }
}
