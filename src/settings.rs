use fsct::DeviceInfo;
use roon_api::settings::{Layout, Widget, Label, OutputDropdown, OutputSetting, Group};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Settings data structure
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ExtensionSettings {
    /// Device mappings: map from device UUID (as string) to output setting
    #[serde(flatten)]
    pub device_outputs: HashMap<String, Option<OutputSetting>>,
}

/// Create settings layout
/// Lists all FSCT devices (detected + previously mapped but not detected)
/// For each device, provides a dropdown with available Roon outputs
pub fn make_layout(
    mut settings: ExtensionSettings,
    available_devices: Vec<DeviceInfo>,
    current_mappings: &HashMap<String, Uuid>,
    available_outputs: Vec<roon_api::transport::Output>,
) -> Layout<ExtensionSettings> {
    let mut widgets: Vec<Widget> = Vec::new();

    // Create a map of device UUID to name for display
    let mut device_info_map: HashMap<Uuid, &DeviceInfo> = HashMap::new();
    for device in &available_devices {
        device_info_map.insert(device.id, device);
    }

    // Collect detected devices
    let mut detected_devices: Vec<Uuid> = available_devices.iter().map(|d| d.id).collect();
    detected_devices.sort();

    // Create reverse mapping from device_uuid to output_id from current mappings
    let device_to_output: HashMap<Uuid, String> = current_mappings
        .iter()
        .map(|(output_id, device_uuid)| (*device_uuid, output_id.clone()))
        .collect();

    // Populate settings with current mappings if not already set
    for (device_uuid, output_id) in &device_to_output {
        let setting_key = format!("device_{}", device_uuid);
        if !settings.device_outputs.contains_key(&setting_key) {
            if let Some(output) = available_outputs.iter().find(|o| &o.output_id == output_id) {
                settings.device_outputs.insert(
                    setting_key,
                    Some(OutputSetting {
                        name: output.display_name.clone(),
                        output_id: output.output_id.clone(),
                    }),
                );
            }
        }
    }

    // Find mapped devices that are not currently detected (disconnected)
    let mapped_device_uuids: HashSet<Uuid> = current_mappings.values().copied().collect();
    let detected_device_set: HashSet<Uuid> = detected_devices.iter().copied().collect();
    let mut disconnected_devices: Vec<Uuid> = mapped_device_uuids
        .difference(&detected_device_set)
        .copied()
        .collect();
    disconnected_devices.sort();

    // Section 1: Detected FSCT Devices
    if detected_devices.is_empty() && disconnected_devices.is_empty() {
        widgets.push(Widget::Label(Label {
            title: "No FSCT devices available".to_string(),
            subtitle: Some("Connect an FSCT device to continue".to_string()),
        }));
    } else {
        // Detected devices section
        if !detected_devices.is_empty() {
            let mut detected_widgets = Vec::new();

            for device_uuid in &detected_devices {
                let device_info = device_info_map.get(device_uuid).unwrap();
                let device_name = device_info
                    .name
                    .as_ref()
                    .map(|n| n.as_str())
                    .unwrap_or("Unnamed Device");

                let setting_key = format!("device_{}", device_uuid);

                detected_widgets.push(Widget::OutputDropdown(OutputDropdown {
                    title: Box::leak(format!("{}", device_name).into_boxed_str()),
                    subtitle: Some(format!("FSCT Device: {}", device_uuid)),
                    setting: Box::leak(setting_key.into_boxed_str()),
                }));
            }

            widgets.push(Widget::Group(Group {
                title: "Available FSCT Devices",
                subtitle: Some("Assign each device to a Roon output".to_string()),
                collapsable: false,
                items: detected_widgets,
            }));
        }

        // Disconnected devices section
        if !disconnected_devices.is_empty() {
            let mut disconnected_widgets = Vec::new();

            for device_uuid in &disconnected_devices {
                let setting_key = format!("device_{}", device_uuid);

                disconnected_widgets.push(Widget::OutputDropdown(OutputDropdown {
                    title: Box::leak(format!("Device {}", device_uuid).into_boxed_str()),
                    subtitle: Some("(Disconnected) - Clear selection to remove".to_string()),
                    setting: Box::leak(setting_key.into_boxed_str()),
                }));
            }

            widgets.push(Widget::Group(Group {
                title: "Previously Assigned (Disconnected)",
                subtitle: Some("These devices are no longer detected. Clear selection to remove.".to_string()),
                collapsable: true,
                items: disconnected_widgets,
            }));
        }
    }

    Layout {
        settings,
        widgets,
        has_error: false,
    }
}

/// Convert settings to mappings
/// Returns a HashMap of output_id -> device_uuid
pub fn settings_to_mappings(settings: &ExtensionSettings) -> HashMap<String, Uuid> {
    let mut mappings = HashMap::new();

    for (key, output_setting) in &settings.device_outputs {
        // Extract device UUID from key (format: "device_{uuid}")
        if let Some(device_uuid_str) = key.strip_prefix("device_") {
            if let Ok(device_uuid) = Uuid::parse_str(device_uuid_str) {
                // If output is set, add mapping
                if let Some(output) = output_setting {
                    mappings.insert(output.output_id.clone(), device_uuid);
                }
            }
        }
    }

    mappings
}

/// Convert current mappings to settings structure
/// Takes output_id -> device_uuid mappings and available outputs
pub fn mappings_to_settings(
    current_mappings: &HashMap<String, Uuid>,
    available_outputs: &[roon_api::transport::Output],
) -> ExtensionSettings {
    let mut device_outputs = HashMap::new();

    // Create reverse mapping: device_uuid -> output
    let device_to_output: HashMap<Uuid, &roon_api::transport::Output> = current_mappings
        .iter()
        .filter_map(|(output_id, device_uuid)| {
            available_outputs
                .iter()
                .find(|o| &o.output_id == output_id)
                .map(|output| (*device_uuid, output))
        })
        .collect();

    // Create settings entries for all mapped devices
    for (device_uuid, output) in device_to_output {
        let key = format!("device_{}", device_uuid);
        device_outputs.insert(
            key,
            Some(OutputSetting {
                name: output.display_name.clone(),
                output_id: output.output_id.clone(),
            }),
        );
    }

    ExtensionSettings { device_outputs }
}
