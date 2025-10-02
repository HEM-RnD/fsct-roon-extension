use std::collections::HashMap;
use std::time::Duration;
use roon_api::{RoonApi, Info, Services, Svc, Core, CoreEvent, Parsed, settings};
use roon_api::{transport, status};
use roon_api::settings::{BoxedSerTrait, Dropdown, Integer, Layout, SerTrait, Settings, Widget};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::sleep;

async fn handle_roon_message(msg: Value, parsed: Parsed) {
    match parsed {
        Parsed::RoonState(state) => {
            RoonApi::save_roon_state("./some_state.txt", state).expect("Failed to save roon state");

        }
        Parsed::Outputs(outputs) => println!("Outputs: {:?}", outputs.iter().map(|o| o.display_name.clone()).collect::<Vec<String>>()),
        Parsed::Zones(zones) => println!("Zones: {:?}", zones.iter().map(|z| (z.display_name.clone(), z.state.clone()
                                                                              , z.now_playing.clone(), z.outputs.clone()))
            .collect::<Vec<_>>()),
        p => println!("Other message: {:?}", p)
    }
}

async fn handle_roon_core_event(event: CoreEvent) {
    match event {
        CoreEvent::None => println!("None core, event"),
        CoreEvent::Discovered(core, id) => println!("Discovered core: {:?}, string: {:?}", core, id),
        CoreEvent::Registered(mut core, id) => {
            let transport = core.get_transport().cloned();
            if let Some(transport) = transport {
                transport.subscribe_outputs().await;
                transport.subscribe_zones().await;
            }
        },
        CoreEvent::Lost(core) => println!("Lost core {:?}", core),
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct MySettings {
    state: bool,
    integer: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct Entry {
    title: String,
    value: bool,
}

#[typetag::serde]
impl SerTrait for Entry {}


impl Entry {
    fn new(title: &str, value: bool) -> BoxedSerTrait {
        Box::new(
            Entry {
                title: title.to_owned(),
                value,
            }
        ) as BoxedSerTrait
    }
}


fn make_layout(settings: MySettings) -> Layout<MySettings> {
    let mut has_error = false;
    let values = vec![
        Entry::new("Disabled", false),
        Entry::new("Enabled", true),
    ];
    let dropdown = Dropdown {
        title: "Dropdown",
        subtitle: None,
        values,
        setting: "state",
    };
    let mut integer = Integer {
        title: "Integer",
        subtitle: None,
        min: "0".to_owned(),
        max: "100".to_owned(),
        setting: "integer",
        error: None,
    };

    if let Ok(out_of_range) = integer.out_of_range(&settings.integer) {
        if out_of_range {
            integer.error = Some(format!("Value should be between {} and {}", integer.min, integer.max));
            has_error = true;
        }
    }

    let widgets = vec![
        Widget::Dropdown(dropdown),
        Widget::Integer(integer),
    ];

    Layout {
        settings,
        widgets,
        has_error,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut roon = RoonApi::new(Info::new(
        "com.hem-e.fsct".to_string(),
        "Ferrum Streaming Control Technology™",
        "0.1.0",
        Some("HEM Sp. z o.o."),
        "info@hem-e.com",
        Some("https://github.com/HEM-RnD/fsct-roon-extension"),
    ));

    let on_connect = move || {
        let roon_state = RoonApi::load_roon_state("./some_state.txt");
        println!("Roon state: {:?}", roon_state);
        roon_state
    };
    let get_layout = |settings: Option<MySettings>| -> Layout<MySettings> {
        let settings = settings.unwrap_or_else(|| {
            let value = RoonApi::load_config("config.json", "settings");
            serde_json::from_value(value).unwrap_or_default()
        });

        make_layout(settings)
    };
    let (svc, settings) = Settings::new(&roon, Box::new(get_layout));

    let services = Some(vec![
        Services::Transport(transport::Transport::new()),
        Services::Settings(settings)
    ]);
    let provided: HashMap<String, Svc> = HashMap::from([
        (settings::SVCNAME.to_owned(), svc),
    ]);

    let result = roon.start_discovery(Box::new(on_connect), provided, services).await;


    if let Some((mut handlers, mut core_rx)) = result {

        handlers.spawn(async move {
            loop {
                if let Some((core_event, msg)) = core_rx.recv().await {
                    handle_roon_core_event(core_event).await;
                    if let Some((msg, parsed)) = msg {
                        handle_roon_message(msg, parsed).await;
                    }
                }
            }
        });
        println!("Waiting for handlers to join");
        handlers.join_next().await;
        println!("Joined");
    }

    sleep(Duration::from_secs(10)).await;
    Ok(())
}
