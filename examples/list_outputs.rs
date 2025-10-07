/// Example: List all Roon outputs and their IDs
///
/// This example connects to Roon Core and displays all available outputs
/// with their IDs, which can be used for creating mappings.
///
/// Usage: cargo run --example list_outputs

use roon_api::{CoreEvent, Info, Parsed, RoonApi, Services};
use roon_api::transport::Transport;
use std::collections::HashMap;

const ROON_STATE_FILE: &str = "./examples_roon_state.txt";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Roon Outputs Lister ===");
    println!("Connecting to Roon Core...\n");

    // Initialize Roon API
    let mut roon = RoonApi::new(Info::new(
        "com.hem-e.fsct.list-outputs".to_string(),
        "FSCT Outputs Lister",
        "0.1.0",
        Some("HEM Sp. z o.o."),
        "info@hem-e.com",
        Some("https://github.com/HEM-RnD/fsct-roon-extension"),
    ));

    // We need transport service to get outputs
    let services = Some(vec![
        Services::Transport(Transport::new()),
    ]);

    let provided: HashMap<String, roon_api::Svc> = HashMap::new();

    // Connection callback
    let on_connect = move || RoonApi::load_roon_state(ROON_STATE_FILE);

    // Start Roon discovery
    let result = roon
        .start_discovery(Box::new(on_connect), provided, services)
        .await;

    if let Some((mut handlers, mut core_rx)) = result {
        println!("Waiting for Roon Core...");

        // Message handler
        handlers.spawn(async move {
            let mut outputs_received = false;

            loop {
                if let Some((core_event, msg)) = core_rx.recv().await {
                    match core_event {
                        CoreEvent::Discovered(_core, id) => {
                            println!("Roon core discovered: {:?}", id);
                        }
                        CoreEvent::Registered(mut core, id) => {
                            println!("Roon core registered: {:?}\n", id);

                            // Subscribe to outputs
                            if let Some(transport) = core.get_transport() {
                                transport.subscribe_outputs().await;
                            }
                        }
                        CoreEvent::Lost(_core) => {
                            println!("Roon core connection lost");
                            break;
                        }
                        CoreEvent::None => {}
                    }

                    if let Some((_msg, parsed)) = msg {
                        match parsed {
                            Parsed::RoonState(state) => {
                                if let Err(e) = RoonApi::save_roon_state(ROON_STATE_FILE, state) {
                                    eprintln!("Error saving Roon state: {}", e);
                                }
                            }
                            Parsed::Outputs(outputs) => {
                                if !outputs_received {
                                    outputs_received = true;

                                    println!("=== Available Roon Outputs ===\n");

                                    if outputs.is_empty() {
                                        println!("No outputs found. Make sure you have audio devices configured in Roon.");
                                    } else {
                                        for output in &outputs {
                                            println!("Output:");
                                            println!("  Name:       {}", output.display_name);
                                            println!("  Output ID:  {}", output.output_id);
                                            println!("  Zone ID:    {}", output.zone_id);

                                            // Show source controls if available
                                            if let Some(source_controls) = &output.source_controls {
                                                for control in source_controls {
                                                    println!("  Source:     {} ({:?})",
                                                        control.display_name, control.status);
                                                }
                                            }

                                            println!();
                                        }

                                        println!("=== How to use these IDs ===");
                                        println!("Copy the 'Output ID' value and paste it into fsct_roon_mappings.json");
                                        println!("Example:");
                                        println!("{{");
                                        println!("  \"map\": {{");
                                        if let Some(first) = outputs.first() {
                                            println!("    \"{}\": \"your-fsct-device-uuid-here\"", first.output_id);
                                        }
                                        println!("  }}");
                                        println!("}}");
                                    }

                                    println!("\nPress Ctrl+C to exit...");
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        handlers.join_next().await;
    } else {
        println!("Failed to start Roon discovery");
    }

    Ok(())
}
