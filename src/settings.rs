use fsct::DeviceInfo;
use roon_api::settings::{BoxedSerTrait, Dropdown, Group, Label, Layout, SerTrait, Widget};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Special output ID that indicates "not mapped"
const UNMAPPED_OUTPUT_ID: &str = "__UNMAPPED__";

/// Settings data structure
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct ExtensionSettings {
    /// Device mappings: map from device UUID (as string) to output_id
    /// Using String instead of OutputSetting to keep dropdown values simple
    #[serde(flatten)]
    pub device_outputs: HashMap<String, String>,
}

/// Dropdown option for output selection
/// Using output_id as the value (simple string instead of complex object)
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputOption {
    pub title: String,
    pub value: String,  // Just the output_id
}

#[typetag::serde]
impl SerTrait for OutputOption {}

/// Format device info for display: "name (VID:PID) s/n serial"
fn format_device_display(device: &DeviceInfo) -> String {
    let name = device.name.as_deref().unwrap_or("Unnamed Device");
    format!("{}", name)
}

fn format_device_second_line_display(device: &DeviceInfo) -> String {
    let sn = device.serial_number.as_deref().unwrap_or("Unknown");
    format!("S/N: {}", sn)
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

    // Find mapped devices that are not currently detected (disconnected)
    let mapped_device_uuids: HashSet<Uuid> = current_mappings.values().copied().collect();
    let detected_device_set: HashSet<Uuid> = detected_devices.iter().copied().collect();
    let mut disconnected_devices: Vec<Uuid> = mapped_device_uuids
        .difference(&detected_device_set)
        .copied()
        .collect();
    disconnected_devices.sort();

    // Create reverse mapping from device_uuid to output_id from current mappings
    let device_to_output: HashMap<Uuid, String> = current_mappings
        .iter()
        .map(|(output_id, device_uuid)| (*device_uuid, output_id.clone()))
        .collect();

    // Populate settings with current mappings if not already set
    for (device_uuid, output_id) in &device_to_output {
        let setting_key = format!("device_{}", device_uuid);
        if !settings.device_outputs.contains_key(&setting_key) {
            settings.device_outputs.insert(setting_key, output_id.clone());
        }
    }

    // Ensure all detected devices have a setting entry (default to "Not mapped")
    for device_uuid in &detected_devices {
        let setting_key = format!("device_{}", device_uuid);
        if !settings.device_outputs.contains_key(&setting_key) {
            settings.device_outputs.insert(setting_key, UNMAPPED_OUTPUT_ID.to_string());
        }
    }

    // Ensure all disconnected devices have a setting entry (default to "Not mapped")
    for device_uuid in &disconnected_devices {
        let setting_key = format!("device_{}", device_uuid);
        if !settings.device_outputs.contains_key(&setting_key) {
            settings.device_outputs.insert(setting_key, UNMAPPED_OUTPUT_ID.to_string());
        }
    }

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
                let device_display = format_device_display(device_info);
                let subtitle = format_device_second_line_display(device_info);

                let setting_key = format!("device_{}", device_uuid);

                // Create dropdown options: "Not mapped" + all available outputs
                let mut dropdown_values: Vec<BoxedSerTrait> = Vec::new();

                // Add "Not mapped" option first
                dropdown_values.push(Box::new(OutputOption {
                    title: "Not mapped".to_string(),
                    value: UNMAPPED_OUTPUT_ID.to_string(),
                }));

                // Add all available Roon outputs
                for output in &available_outputs {
                    dropdown_values.push(Box::new(OutputOption {
                        title: output.display_name.clone(),
                        value: output.output_id.clone(),
                    }));
                }

                detected_widgets.push(Widget::Dropdown(Dropdown {
                    title: Box::leak(device_display.into_boxed_str()),
                    subtitle: Some(subtitle),
                    values: dropdown_values,
                    setting: Box::leak(setting_key.into_boxed_str()),
                }));
            }

            widgets.push(Widget::Group(Group {
                title: "Available FSCT Devices",
                subtitle: Some("Assign each device to a Roon output, or select 'Not mapped' to ignore".to_string()),
                collapsable: false,
                items: detected_widgets,
            }));
        }

        // Disconnected devices section
        if !disconnected_devices.is_empty() {
            let mut disconnected_widgets = Vec::new();

            for device_uuid in &disconnected_devices {
                let setting_key = format!("device_{}", device_uuid);

                // Create dropdown options: "Not mapped" + all available outputs
                let mut dropdown_values: Vec<BoxedSerTrait> = Vec::new();

                // Add "Not mapped" option first
                dropdown_values.push(Box::new(OutputOption {
                    title: "Not mapped".to_string(),
                    value: UNMAPPED_OUTPUT_ID.to_string(),
                }));

                // Add all available Roon outputs
                for output in &available_outputs {
                    dropdown_values.push(Box::new(OutputOption {
                        title: output.display_name.clone(),
                        value: output.output_id.clone(),
                    }));
                }

                disconnected_widgets.push(Widget::Dropdown(Dropdown {
                    title: Box::leak(format!("Device {}", device_uuid).into_boxed_str()),
                    subtitle: Some("(Disconnected) | Select 'Not mapped' to remove from list".to_string()),
                    values: dropdown_values,
                    setting: Box::leak(setting_key.into_boxed_str()),
                }));
            }

            widgets.push(Widget::Group(Group {
                title: "Previously Assigned (Disconnected)",
                subtitle: Some("These devices are no longer detected. Select 'Not mapped' to remove.".to_string()),
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
/// Ignores devices with "Not mapped" selection
pub fn settings_to_mappings(settings: &ExtensionSettings) -> HashMap<String, Uuid> {
    let mut mappings = HashMap::new();

    for (key, output_id) in &settings.device_outputs {
        // Extract device UUID from key (format: "device_{uuid}")
        if let Some(device_uuid_str) = key.strip_prefix("device_") {
            if let Ok(device_uuid) = Uuid::parse_str(device_uuid_str) {
                // If output_id is not the special "unmapped" value, add mapping
                if output_id != UNMAPPED_OUTPUT_ID {
                    mappings.insert(output_id.clone(), device_uuid);
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
    _available_outputs: &[roon_api::transport::Output],
) -> ExtensionSettings {
    let mut device_outputs = HashMap::new();

    // Create reverse mapping: device_uuid -> output_id
    for (output_id, device_uuid) in current_mappings {
        let key = format!("device_{}", device_uuid);
        device_outputs.insert(key, output_id.clone());
    }

    ExtensionSettings { device_outputs }
}
