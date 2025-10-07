# FSCT-Roon Extension

## Overview
This extension bridges Roon audio system with FSCT (Ferrum Streaming Control Technology) devices, allowing Roon outputs to control FSCT devices.

**Status**: ✅ Compiled and tested successfully

## Architecture

### Core Components

1. **Mapping (`src/mapping.rs`)**: Stores output_id → device_uuid mappings in JSON
2. **Player Manager (`src/player_manager.rs`)**: Manages FSCT player lifecycle (register/unregister)
3. **Conversions (`src/conversions.rs`)**: Converts Roon types to FSCT PlayerState
4. **Settings (`src/settings.rs`)**: Read-only settings UI showing current mappings
5. **Main (`src/main.rs`)**: Event loop handling Roon outputs/zones changes

### Player Lifecycle

- **One player per mapped Roon output** (not one player for all of Roon)
- Players are registered when:
  - Mapping exists (output_id → device_uuid)
  - Roon output is available
- Device does NOT need to be available at registration time
- Players are unregistered when output becomes unavailable

### Dependencies

- `fsct-core`: Core FSCT types (PlayerState, TrackMetadata, FsctStatus, DeviceInfo)
- `fsct-client::IpcDriver`: Direct IPC communication with FSCT driver
- `roon-api`: Roon integration with settings, transport, status features
- All types from fsct-core are used directly - no custom wrappers

## Settings UI Limitation

The Roon settings API requires static string references (`&'static str`) for setting field names, which prevents creating dynamic OutputDropdown widgets for each device UUID at runtime.

**Current Solution**: Settings UI is read-only, displaying current mappings as labels:
- Lists all FSCT devices (detected + previously mapped)
- Shows which Roon output each device is mapped to
- Notes that mappings must be changed via config file

**Future Enhancement**: Could implement a custom web UI or CLI tool for managing mappings.

## Configuration

- **Mappings file**: `./fsct_roon_mappings.json`
- **Roon state file**: `./fsct_roon_state.txt`

Edit `fsct_roon_mappings.json` manually to set up mappings:

```json
{
  "map": {
    "output-id-1": "device-uuid-1",
    "output-id-2": "device-uuid-2"
  }
}
```

## Event Flow

1. Extension connects to FSCT driver and Roon Core
2. When Roon outputs change:
   - Check which outputs have mappings
   - Register players for available mapped outputs
   - Unregister players for unavailable outputs
3. When Roon zones change:
   - Update player state for all registered players in the zone
   - Sync metadata (title, artist, album) and timeline (position, duration)
   - Sync playback status (playing, paused, stopped)

## Usage

### 1. List Roon Outputs (to get Output IDs)
```cmd
cargo run --example list_outputs
```

This will connect to Roon Core and display all available outputs with their IDs.

### 2. Configure Mappings

Edit `fsct_roon_mappings.json` with the Output IDs from step 1:

```json
{
  "map": {
    "output-id-from-roon": "fsct-device-uuid"
  }
}
```

### 3. Run the Extension
```cmd
cargo run
```

## Development

### Build and Test
```cmd
cargo build
cargo test
```

### Requirements for Integration Testing
- FSCT driver running with at least one device
- Roon Core running on the network
- Configured mappings in `fsct_roon_mappings.json`
