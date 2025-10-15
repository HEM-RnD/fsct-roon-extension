use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;
use fsct::{DeviceInfo, DeviceChangeEvent, FsctDriver};

/// Thread-safe manager that maintains a cache of FSCT device infos keyed by device id
///
/// Responsibilities:
/// - Keep a map of device_id -> DeviceInfo in a Mutex for cheap interior mutability
/// - Provide read accessors for settings/UI
/// - Provide mutation helpers that react to driver events and initial snapshot
#[derive(Default)]
pub struct FsctDeviceManager {
    devices: Mutex<HashMap<Uuid, DeviceInfo>>, // protected by mutex
}

impl FsctDeviceManager {
    pub fn new() -> Self { Self { devices: Mutex::new(HashMap::new()) } }

    /// Returns a copy of all currently known devices (values cloned for UI)
    pub fn get_all(&self) -> Vec<DeviceInfo> {
        let map = self.devices.lock().unwrap();
        map.values().cloned().collect()
    }

    /// Populate/refresh manager from a list of device IDs by fetching their DeviceInfo from driver
    pub async fn upsert_from_ids(&self, driver: &dyn FsctDriver, ids: &[Uuid]) {
        let mut to_insert: Vec<(Uuid, DeviceInfo)> = Vec::with_capacity(ids.len());
        for id in ids {
            match driver.get_device_info(*id).await {
                Ok(info) => to_insert.push((*id, info)),
                Err(err) => log::warn!("Failed to get device info for {}: {}", id, err),
            }
        }
        let mut map = self.devices.lock().unwrap();
        for (id, info) in to_insert { map.insert(id, info); }
    }

    /// Apply a single device change event
    pub async fn handle_event(&self, driver: &dyn FsctDriver, evt: DeviceChangeEvent) {
        match evt {
            DeviceChangeEvent::Added(id) => {
                if let Ok(info) = driver.get_device_info(id).await {
                    self.devices.lock().unwrap().insert(id, info);
                } else {
                    log::warn!("Device added but get_device_info failed: {}", id);
                }
            }
            DeviceChangeEvent::Removed(id) => {
                self.devices.lock().unwrap().remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Error;
    use async_trait::async_trait;
    use fsct::{FsctStatus, FsctTextMetadata, TimelineInfo, ManagedPlayerId};
    use fsct::player_state::PlayerState;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::broadcast;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct MockDriver {
        infos: Arc<Mutex<HashMap<Uuid, DeviceInfo>>>,
        _reg: Arc<AtomicUsize>,
    }

    impl MockDriver {
        fn with_devices(devs: Vec<DeviceInfo>) -> Self {
            let mut map = HashMap::new();
            for d in devs { map.insert(d.id, d); }
            Self { infos: Arc::new(Mutex::new(map)), _reg: Arc::new(AtomicUsize::new(0)) }
        }
    }

    #[async_trait]
    impl FsctDriver for MockDriver {
        async fn register_player(&self, _self_id: String) -> Result<ManagedPlayerId, Error> { Err(Error::msg("not used")) }
        async fn unregister_player(&self, _player_id: ManagedPlayerId) -> Result<(), Error> { Ok(()) }
        async fn assign_player_to_device(&self, _player_id: ManagedPlayerId, _device_id: Uuid) -> Result<(), Error> { Ok(()) }
        async fn unassign_player_from_device(&self, _player_id: ManagedPlayerId, _device_id: Uuid) -> Result<(), Error> { Ok(()) }
        async fn update_player_state(&self, _player_id: ManagedPlayerId, _new_state: PlayerState) -> Result<(), Error> { Ok(()) }
        async fn update_player_status(&self, _player_id: ManagedPlayerId, _new_status: FsctStatus) -> Result<(), Error> { Ok(()) }
        async fn update_player_timeline(&self, _player_id: ManagedPlayerId, _new_timeline: Option<TimelineInfo>) -> Result<(), Error> { Ok(()) }
        async fn update_player_metadata(&self, _player_id: ManagedPlayerId, _metadata_id: FsctTextMetadata, _new_text: Option<String>) -> Result<(), Error> { Ok(()) }
        async fn get_player_assigned_device(&self, _player_id: ManagedPlayerId) -> Result<Option<Uuid>, Error> { Ok(None) }
        async fn get_detected_devices(&self) -> Result<Vec<Uuid>, Error> { Ok(self.infos.lock().unwrap().keys().cloned().collect()) }
        async fn subscribe_device_changes(&self) -> Result<broadcast::Receiver<DeviceChangeEvent>, Error> { let (_tx, rx) = broadcast::channel(1); Ok(rx) }
        async fn get_device_info(&self, device_id: Uuid) -> Result<DeviceInfo, Error> { self.infos.lock().unwrap().get(&device_id).cloned().ok_or_else(|| Error::msg("not found")) }
    }

    fn mk_info(id: Uuid, name: &str) -> DeviceInfo {
        DeviceInfo {
            id,
            name: Some(name.to_string()),
            manufacturer: Some("MFR".into()),
            vendor_id: 0x1234,
            product_id: 0x5678,
            serial_number: Some("SN".into()),
        }
    }

    #[tokio::test]
    async fn populates_from_ids_and_updates() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let d1 = mk_info(id1, "A");
        let d2 = mk_info(id2, "B");
        let driver = MockDriver::with_devices(vec![d1.clone(), d2.clone()]);
        let mgr = FsctDeviceManager::new();

        mgr.upsert_from_ids(&driver, &[id1, id2]).await;
        let mut all = mgr.get_all();
        all.sort_by_key(|d| d.name.clone());
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, id1);
        assert_eq!(all[1].id, id2);

        // remove id2
        mgr.handle_event(&driver, DeviceChangeEvent::Removed(id2)).await;
        let all = mgr.get_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id1);

        // add id2 back
        mgr.handle_event(&driver, DeviceChangeEvent::Added(id2)).await;
        let all = mgr.get_all();
        assert_eq!(all.len(), 2);
    }
}
