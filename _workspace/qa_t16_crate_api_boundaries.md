# T16: Crate API Boundary Verification Report

Date: 2026-04-02
Status: PASS (all boundaries verified)

## Summary

All inter-crate API boundaries have been verified. 143 tests pass across 4 crates. No boundary mismatches or orphaned APIs found.

## Test Count Update

| Crate | Before (Phase 7) | After (T1-T4) | Delta | New Modules |
|---|---|---|---|---|
| nsynergy-core | 83 | 93 | +10 | capture.rs (rdev integration), inject.rs (enigo integration) |
| nsynergy-net | 23 | 23 | 0 | - |
| nsynergy-server | 9 | 14 | +5 | server.rs (main loop) |
| nsynergy-client | 9 | 13 | +4 | client.rs (main loop) |
| **Total** | **124** | **143** | **+19** | |

## Crate Dependency Graph

```
nsynergy-core (foundation - no internal crate deps)
  |
  +-- nsynergy-net (depends on: core)
  |
  +-- nsynergy-server (depends on: core, net)
  |
  +-- nsynergy-client (depends on: core, net, server)
  |
  +-- nsynergy-tauri (depends on: core)
```

**Note**: nsynergy-client depends on nsynergy-server for the `ServerMessage` enum (shared protocol). This is the correct pattern -- the server defines the wire protocol and the client imports it.

## Boundary 1: core -> net -- PASS

| API | Producer (core) | Consumer (net) | Status |
|---|---|---|---|
| `protocol::serialize_event` | `protocol.rs:19` | `udp.rs:26` | PASS |
| `protocol::deserialize_event` | `protocol.rs:31` | `udp.rs:68` | PASS |
| `protocol::MAX_UDP_PAYLOAD` | `protocol.rs:16` | `udp.rs:56` (buf sizing) | PASS |
| `event::TimestampedEvent` | `event.rs:72` | `udp.rs:25`, `tcp.rs:189` | PASS |

**Verification**: UDP and TCP tests in net crate correctly serialize/deserialize core's TimestampedEvent using core's protocol module.

## Boundary 2: core -> server -- PASS

| API | Producer (core) | Consumer (server) | Status |
|---|---|---|---|
| `config::AppConfig` | `config.rs:36` | `server.rs:63` (From impl) | PASS |
| `config::ScreenPosition` | `config.rs:7` | `server.rs:2`, `handler.rs:2` | PASS |
| `event::TimestampedEvent` | `event.rs:72` | `server.rs:106`, `handler.rs:3` | PASS |
| `event::InputEvent` | `event.rs:46` | `handler.rs:97` | PASS |
| `screen::DisplayInfo` | `screen.rs` | `server.rs:6`, `handler.rs:4` | PASS |
| `screen::ScreenEdge` | `screen.rs` | `handler.rs:4` | PASS |
| `screen::detect_edge` | `screen.rs` | `handler.rs:102` | PASS |
| `screen::map_position` | `screen.rs` | `handler.rs:106` | PASS |
| `protocol::serialize_event` | `protocol.rs:19` | `server.rs:224` | PASS |
| `discovery::ServiceRegistration` | `discovery.rs:29` | `server.rs:131` | PASS |

**Verification**: Server correctly uses core's screen edge detection and coordinate mapping for event routing. ServerConfig correctly derives from AppConfig via From trait.

## Boundary 3: core -> client -- PASS

| API | Producer (core) | Consumer (client) | Status |
|---|---|---|---|
| `event::TimestampedEvent` | `event.rs:72` | `client.rs:55`, `handler.rs:52` | PASS |
| `event::InputEvent` | `event.rs:46` | `handler.rs:73` | PASS |
| `inject::InputInjector` (trait) | `inject.rs:6` | `handler.rs:3` | PASS |
| `inject::inject_event` | `inject.rs:14` | `handler.rs:57` | PASS |
| `inject::remap_coordinates` | `inject.rs:46` | `handler.rs:77` | PASS |
| `screen::DisplayInfo` | `screen.rs` | `client.rs:6`, `handler.rs:5` | PASS |
| `protocol::deserialize_event` | `protocol.rs:31` | `client.rs:196` | PASS |
| `protocol::MAX_UDP_PAYLOAD` | `protocol.rs:16` | `client.rs:187` | PASS |
| `config::ScreenPosition` | `config.rs:7` | `client.rs:2` | PASS |
| `discovery::DiscoveryEvent` | `discovery.rs:22` | `client.rs:3` | PASS |
| `discovery::PeerInfo` | `discovery.rs:13` | `client.rs:3` | PASS |

**Verification**: Client correctly uses core's injection system, coordinate remapping, and protocol deserialization.

## Boundary 4: net -> server -- PASS

| API | Producer (net) | Consumer (server) | Status |
|---|---|---|---|
| `tcp::TcpTransport` | `tcp.rs:54` | `server.rs:7`, `server.rs:113` | PASS |
| `tcp::send_message` | `tcp.rs:13` | `server.rs:326` | PASS |
| `tcp::recv_message` | `tcp.rs:29` | `server.rs:293` | PASS |

## Boundary 5: net -> client -- PASS

| API | Producer (net) | Consumer (client) | Status |
|---|---|---|---|
| `tcp::connect` | `tcp.rs:82` | `client.rs:148` | PASS |
| `tcp::send_message` | `tcp.rs:13` | `client.rs:158` | PASS |
| `tcp::recv_message` | `tcp.rs:29` | `client.rs:161` | PASS |
| `reconnect::ReconnectConfig` | `reconnect.rs` | `client.rs:7`, `client.rs:23` | PASS |
| `reconnect::ReconnectState` | `reconnect.rs` | `client.rs:81` | PASS |

## Boundary 6: server -> client -- PASS

| API | Producer (server) | Consumer (client) | Status |
|---|---|---|---|
| `server::ServerMessage` enum | `server.rs:18` | `client.rs:9`, `client.rs:151` | PASS |
| `server::ServerConfig` | `server.rs:53` | `client.rs:264` (tests only) | PASS |
| `server::start_server` | `server.rs:104` | `client.rs:303` (tests only) | PASS |
| `server::ServerStatus` | `server.rs:91` | `client.rs:264` (tests only) | PASS |

**Note**: The client imports ServerMessage for the shared TCP protocol (Hello, Welcome, Goodbye, Ping, Pong). The server-specific types (ServerConfig, start_server, ServerStatus) are only used in client tests for integration testing.

## Boundary 7: core -> tauri -- PASS

| API | Producer (core) | Consumer (tauri) | Status |
|---|---|---|---|
| `config::AppConfig` | `config.rs:36` | `commands.rs:1`, `lib.rs:6` | PASS |
| `config::Role` | `config.rs:27` | `commands.rs:1` | PASS |
| `config::ScreenPosition` | `config.rs:7` | `commands.rs:1` | PASS |
| `permissions::check_permissions` | `permissions.rs:25` | `commands.rs:139` | PASS |
| `permissions::permission_instructions` | `permissions.rs:74` | `commands.rs:145` | PASS |
| `permissions::PermissionCheck` | `permissions.rs:16` | `commands.rs:2` | PASS |
| `security::generate_pairing_code` | `security.rs:95` | `commands.rs:151` | PASS |

## Wire Protocol Consistency

The `ServerMessage` enum (server.rs:18-38) is serialized with bincode and used across the TCP boundary:
- Server serializes Welcome/Pong/Goodbye -> Client deserializes
- Client serializes Hello/Ping/Goodbye -> Server deserializes

Both sides use `bincode::serialize` / `bincode::deserialize` with the same type, ensuring wire compatibility. Verified in integration tests:
- `server::tests::client_connects_and_receives_welcome`
- `server::tests::ping_pong_heartbeat`
- `client::tests::client_connects_to_server`

## Key Mapping Consistency

capture.rs (rdev -> Key) and inject.rs (Key -> enigo) use the same u32 keycode scheme:
- Letters: 0x41-0x5A
- Digits: 0x29-0x32
- Modifiers: 0x01, 0x05-0x06, 0x19-0x1A, 0x1F-0x20
- Function keys: 0x0B-0x16

Both modules are tested independently and the keycode mapping is bidirectional consistent.
