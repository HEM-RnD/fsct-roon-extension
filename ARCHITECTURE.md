# FSCT-Roon Extension Architecture

## Overview

This extension acts as a bridge between Roon audio system and FSCT (Ferrum Streaming Control Technology™) devices. It enables automatic synchronization of playback state and metadata from Roon to paired FSCT devices.

## Core Components

### 1. Roon Integration Layer
Handles Roon Core connection, registration, and event processing.

### 2. FSCT Integration Layer
Manages FSCT driver communication and device discovery.

### 3. Device Mapping Service
Maintains bidirectional mappings between Roon outputs and FSCT devices.

### 4. State Synchronization Engine
Propagates playback state and metadata between systems.

### 5. Persistence Layer
Stores device mappings and configuration.

### 6. Settings UI
Exposes Roon settings interface for device pairing.

## Module Structure

```
src/
├── main.rs                      # Application entry point, initialization
├── roon/
│   ├── mod.rs                   # Roon module public interface
│   ├── client.rs                # Roon API client wrapper
│   ├── events.rs                # Roon event handlers
│   └── transport.rs             # Transport service integration
├── fsct/
│   ├── mod.rs                   # FSCT module public interface
│   ├── client.rs                # FSCT client wrapper
│   ├── events.rs                # FSCT event handlers
│   └── device_monitor.rs        # Device discovery and monitoring
├── mapping/
│   ├── mod.rs                   # Mapping module public interface
│   ├── service.rs               # Device mapping business logic
│   └── persistence.rs           # Save/load mappings to disk
├── sync/
│   ├── mod.rs                   # Sync module public interface
│   ├── engine.rs                # State synchronization logic
│   └── metadata.rs              # Metadata transformation helpers
├── settings/
│   ├── mod.rs                   # Settings module public interface
│   ├── ui.rs                    # Roon settings UI layout
│   └── config.rs                # Configuration data structures
└── lib.rs                       # Library exports for testing
```

## Task Breakdown

### Phase 1: Project Structure & Core Infrastructure

**Task 1.1**: Create modular project structure
- Create module directories and mod.rs files
- Define public interfaces for each module
- Set up lib.rs for testability

**Task 1.2**: Define core data structures
- Device mapping structures (RoonOutput ↔ FsctDevice)
- Configuration and settings models
- Shared state structures with appropriate synchronization primitives

**Task 1.3**: Implement persistence layer
- File-based storage for device mappings (JSON)
- Configuration load/save with error handling
- Migration strategy for future schema changes

### Phase 2: Roon Integration

**Task 2.1**: Enhance Roon client wrapper
- Encapsulate RoonApi initialization
- Handle connection lifecycle (discovery, registration, lost)
- Implement reconnection logic

**Task 2.2**: Implement Roon event handling
- Process transport zone changes
- Handle output availability changes
- Track playback state (playing, paused, stopped)
- Extract metadata (track, artist, album, artwork)

**Task 2.3**: Build Roon transport integration
- Subscribe to outputs and zones on registration
- Query current outputs list
- Handle zone state updates

### Phase 3: FSCT Integration

**Task 3.1**: Implement FSCT client wrapper
- Initialize fsct-client connection
- Handle connection lifecycle
- Implement error handling and reconnection

**Task 3.2**: Implement device discovery and monitoring
- Detect available FSCT devices
- Monitor device connection/disconnection
- Maintain device availability status

**Task 3.3**: Implement FSCT control operations
- Set playback metadata (track info, artwork)
- Update playback state (play, pause, stop)
- Handle command errors gracefully

### Phase 4: Device Mapping Service

**Task 4.1**: Implement mapping service core logic
- Bidirectional lookup (Roon output ID ↔ FSCT device ID)
- Add/remove/update mappings
- Validate mapping consistency

**Task 4.2**: Integrate with persistence layer
- Load mappings on startup
- Save mappings on changes
- Handle persistence errors

**Task 4.3**: Implement orphaned device handling
- Track unavailable devices in mappings
- Automatically restore mappings when devices reconnect
- Provide mapping status information

### Phase 5: Settings UI

**Task 5.1**: Design settings layout
- List all available Roon outputs
- List all available FSCT devices
- Display current mappings with status indicators

**Task 5.2**: Implement pairing interface
- Dropdown/selection widget for pairing
- Add new mapping action
- Remove existing mapping action

**Task 5.3**: Settings state management
- Load current mappings into settings UI
- Handle user changes and validation
- Apply changes and persist

### Phase 6: State Synchronization

**Task 6.1**: Implement sync engine core
- Event queue for Roon zone changes
- Mapping resolution (zone → output → FSCT device)
- Async command dispatch to FSCT

**Task 6.2**: Implement metadata synchronization
- Transform Roon metadata format to FSCT format
- Handle artwork URLs/data
- Handle missing metadata fields gracefully

**Task 6.3**: Implement playback state synchronization
- Map Roon playback states to FSCT states
- Handle transition edge cases (zone group changes)
- Rate limiting and debouncing if needed

### Phase 7: Integration & Main Application

**Task 7.1**: Wire all components in main.rs
- Initialize all services with proper dependencies
- Set up message channels between components
- Configure graceful shutdown handling

**Task 7.2**: Implement application lifecycle
- Startup sequence (load config → connect Roon → connect FSCT)
- Health monitoring
- Shutdown sequence with resource cleanup

**Task 7.3**: Error handling and logging
- Basic logging throughout application
- Error recovery strategies
- User-visible error messages in settings

### Phase 8: Testing

**Task 8.1**: Unit tests for core logic
- Mapping service tests
- Metadata transformation tests
- State machine tests

**Task 8.2**: Integration tests
- Mock Roon/FSCT clients for testing
- End-to-end mapping flow tests
- Persistence tests

**Task 8.3**: Manual testing scenarios
- Document test cases
- Device reconnection scenarios
- Concurrent playback handling

### Phase 9: Documentation & Polish

**Task 9.1**: Code documentation
- Add doc comments to public interfaces
- Document complex algorithms
- Add usage examples

**Task 9.2**: Error message improvements
- User-friendly error descriptions
- Troubleshooting hints
- Validation messages

**Task 9.3**: Performance optimization (if needed)
- Identify bottlenecks
- Optimize event processing
- Memory usage analysis

## Key Technical Considerations

### Concurrency Model
- Use Tokio async runtime with single-threaded flavor (current_thread)
- Use `tokio::sync::mpsc` for message passing between components
- Use `std::sync::RwLock` or `parking_lot::RwLock` for shared state
- One main event loop coordinator, separate tasks for Roon and FSCT monitoring

### Data Flow
```
Roon Event → Event Handler → Sync Engine → Mapping Service → FSCT Client → Device
                                ↓
                          Persistence Layer
```

### Error Handling Strategy
- Use `anyhow::Result` for application errors
- Use `thiserror` for domain-specific errors (optional, can use anyhow for minimal code)
- Never panic in production code
- Log errors with context using println!/eprintln! or simple logging

### Configuration
- Settings file (JSON) for application config
- Separate file for device mappings
- Runtime settings via Roon settings UI

## Additional Dependencies (minimal set)

```toml
parking_lot = "0.12"  # For efficient RwLock (optional, can use std::sync)
thiserror = "2.0"     # Better error types (optional, can use anyhow only)
```

## Design Principles

- **Minimal code**: Keep implementation simple and focused
- **Clean separation**: Each module has clear responsibility
- **Testable**: Components can be tested independently
- **Async/concurrent**: Non-blocking operations
- **Persistent mappings**: Survive restarts
- **Automatic reconnection**: Handle device/service disconnections gracefully
- **User-friendly**: Clear settings interface for pairing devices
