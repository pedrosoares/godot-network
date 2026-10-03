# godot_network

GDExtension client for [mw-server](https://github.com/pedrosoares/mw-server).
All networking runs on background threads, so connecting, sending and
receiving never block a frame.

## Classes

**`NetworkClient`** (Node, usually an autoload)

- `connect_to_server(host, tcp_port, udp_port, name)`: emits `connected(id)`
  on success or `error(message)` on failure.
- Lobby methods: `search_for_matches()`, `stop_search_for_matches()`,
  `create_match(name)`, `join_match(id)`, `leave_match()`, `delete_match()`,
  `start_match(map)` and `spawn_players(positions)`.
- State: `is_on_a_match()`, `get_player()` and `get_match()`.
- Replication:
  - `spawn_remote_object` and `despawn_remote_object`.
  - `send_location`: UDP with newest-wins delivery, falling back to TCP until UDP is ready.
  - `remote_object_call(player_id, object_id, method, params, broadcast)`.
- Voice and custom data: `send_voice(frame)`, `send_datagram(channel, payload)`
  (channels 16–255), `send_chat(text)` and `send_game(kind, payload)`.
- Signals: `on_match_*`, `on_player_joined`, `on_player_leaved`,
  `on_spawn_remote_object`, `on_remote_object_location`, `on_remote_call`,
  `on_voice`, `on_datagram`, `on_chat`, `server_error(code, message)`, and more.
  On UDP signals, the player id comes from the server, so it can't be spoofed.
  With a server running `--host-migration`, `on_owner_changed(owner_id)` fires
  when the owner leaves, and `get_match()` reports the new owner. With
  `--late-join`, joining a started match emits `on_match_started` and an
  `on_spawn_remote_object` for every live object right after `on_match_joined`.

**`NetworkSyncClientNode3d`** (Node3D)

Replicates a node:
- It spawns the object on `_ready`.
- While the node moves, it sends `sync_rate` updates per second (default 20)
  from the physics tick.
- While the node is still, it sends a keepalive every `keepalive` seconds.
- It despawns the object when it leaves the tree.

`object_id` must be unique among the player's live objects.

## Build

```sh
cargo build --release   # target/release/libgodot_network.{so,dylib} / godot_network.dll
```

CI builds the library for Linux, Windows and macOS and uploads each as an
artifact named after its file.

## Tests

`tests/run.sh` builds the extension, starts an mw-server, and runs
`tests/godot/test.gd` headless against it:

```sh
GODOT=/path/to/godot MW_SERVER=/path/to/mw-server/target/debug/network_manager tests/run.sh
```
