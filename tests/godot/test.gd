## End-to-end tests for NetworkClient / NetworkSyncClientNode3d against a
## live mw-server. Run through tests/run.sh.
extends SceneTree

const ERROR_MATCH_STARTED := 6

var failures := 0


func _initialize() -> void:
	_run.call_deferred()


func check(condition: bool, what: String) -> void:
	if condition:
		print("ok   ", what)
	else:
		failures += 1
		print("FAIL ", what)


func wait_until(condition: Callable, what: String, timeout_ms := 4000) -> bool:
	var deadline := Time.get_ticks_msec() + timeout_ms
	while not condition.call():
		if Time.get_ticks_msec() > deadline:
			check(false, what + " (timed out)")
			return false
		await process_frame
	check(true, what)
	return true


func wait(ms: int) -> void:
	var deadline := Time.get_ticks_msec() + ms
	while Time.get_ticks_msec() < deadline:
		await process_frame


func new_client() -> NetworkClient:
	var client := NetworkClient.new()
	root.add_child(client)
	return client


func login(client: NetworkClient, name: String, tcp: int, udp: int) -> void:
	client.connect_to_server("127.0.0.1", tcp, udp, name)
	await wait_until(func(): return client.is_connected_to_server(), name + " logs in")


func _run() -> void:
	var args := OS.get_cmdline_user_args()
	var tcp := int(args[0])
	var udp := int(args[1])

	# ---- connecting never blocks the main thread ----
	var stray := new_client()
	var errors := []
	stray.error.connect(func(m): errors.append(m))
	var longest_frame := 0
	var last := Time.get_ticks_msec()
	stray.connect_to_server("127.0.0.1", 1, 1, "nobody")  # refused
	while errors.is_empty() and Time.get_ticks_msec() - last < 3000:
		await process_frame
		var now := Time.get_ticks_msec()
		longest_frame = maxi(longest_frame, now - last)
		last = now
	check(errors.size() == 1, "refused connection reports an error")
	check(longest_frame < 100, "no frame blocked while connecting (%d ms)" % longest_frame)
	stray.queue_free()

	# ---- lobby ----
	var host := new_client()
	var guest := new_client()
	await login(host, "host", tcp, udp)
	await login(guest, "guest", tcp, udp)
	var host_id := host.get_player().id
	var guest_id := guest.get_player().id

	# Handlers may call back into the client while it emits (re-entrancy).
	var created := []
	host.on_match_created.connect(func(m):
		created.append(m)
		check(host.get_player().id == m.owner_id, "re-entrant call inside a signal handler"))
	host.create_match("arena")
	await wait_until(func(): return created.size() == 1, "match created")
	var room: int = created[0].id

	var joined := []
	guest.on_match_joined.connect(func(m): joined.append(m))
	guest.join_match(room)
	await wait_until(func(): return joined.size() == 1, "match joined")
	check(joined[0].owner_id == host_id, "joiner learns the owner (%d)" % joined[0].owner_id)
	check(joined[0].number_of_players == 2, "joiner learns the player count")
	check(guest.is_on_a_match() and guest.get_room_id() == room, "guest is on the match")
	await wait_until(func(): return host.is_udp_ready() and guest.is_udp_ready(), "udp ready")

	# ---- RPCs keep every component ----
	var calls := []
	guest.on_remote_call.connect(func(p, o, m, params): calls.append([p, o, m, params]))
	var typed: Array[int] = [7, 8]
	host.remote_object_call(host_id, 50, "back_alive",
		[100.0, Vector3(1, 2, 3), typed, ["x", null, true]], true)
	await wait_until(func(): return calls.size() == 1, "broadcast rpc")
	if calls.size() == 1:
		var params: Array = calls[0][3]
		check(calls[0][0] == host_id and calls[0][2] == "back_alive", "rpc header")
		check(params[1] == Vector3(1, 2, 3), "Vector3 keeps z (%s)" % [params[1]])
		check(params[2] == [7, 8] and params[3] == ["x", null, true], "nested arrays")

	var host_calls := []
	host.on_remote_call.connect(func(p, o, m, params): host_calls.append([p, m, params]))
	guest.remote_object_call(host_id, 50, "take_damage", [10.0], false)
	await wait_until(func(): return host_calls.size() == 1, "targeted rpc reaches the target")

	# ---- replication over UDP ----
	var spawns := []
	var locations := []
	var despawns := []
	guest.on_spawn_remote_object.connect(func(p, o, pos, rot): spawns.append([p, o, pos]))
	guest.on_remote_object_location.connect(func(p, o, pos, rot): locations.append([p, o, pos]))
	guest.on_despawn_remote_object.connect(func(p, o): despawns.append([p, o]))

	var mover := Node3D.new()
	var sync := NetworkSyncClientNode3d.new()
	sync.object_id = 160
	sync.sync_position = true
	sync.sync_rotation = true
	sync.network_client_node = host
	mover.add_child(sync)
	root.add_child(mover)
	await wait_until(func(): return spawns.size() == 1, "spawn on ready")
	check(spawns.size() == 1 and spawns[0][0] == host_id and spawns[0][1] == 160, "spawn ids")

	var started_at := Time.get_ticks_msec()
	while Time.get_ticks_msec() - started_at < 1000:
		mover.position.x += 0.05
		await physics_frame
	await wait(200)
	var rate := locations.size()
	check(rate >= 12 and rate <= 25, "about 20 updates per second while moving (%d)" % rate)
	check(rate > 0 and locations[-1][0] == host_id, "sender id comes from the server")
	check(rate > 0 and is_equal_approx(locations[-1][2].x, mover.position.x), "latest position applied")

	locations.clear()
	await wait(1100)
	check(locations.size() >= 1 and locations.size() <= 3, "slow keepalive while still (%d)" % locations.size())

	mover.queue_free()
	await wait_until(func(): return despawns == [[host_id, 160]], "despawn when the node is freed")

	# ---- voice & chat ----
	var voices := []
	guest.on_voice.connect(func(p, frame): voices.append([p, frame]))
	host.send_voice(PackedByteArray([1, 2, 3, 250]))
	await wait_until(func(): return voices.size() == 1, "voice frame")
	check(voices.size() == 1 and voices[0] == [host_id, PackedByteArray([1, 2, 3, 250])], "voice payload and sender")

	var chats := []
	host.on_chat.connect(func(p, n, t): chats.append([p, n, t]))
	guest.send_chat("hi")
	await wait_until(func(): return chats == [[guest_id, "guest", "hi"]], "chat")

	# ---- leaving and joining again in the same session ----
	var left := []
	host.on_player_leaved.connect(func(p): left.append(p.id))
	guest.leave_match()
	check(not guest.is_on_a_match(), "not on a match right after leaving")
	await wait_until(func(): return left == [guest_id], "host sees the guest leave")
	guest.join_match(room)
	await wait_until(func(): return joined.size() == 2 and guest.is_on_a_match(), "join again")

	# ---- started matches are closed ----
	var started := []
	guest.on_match_started.connect(func(map): started.append(map))
	host.start_match("pvp")
	await wait_until(func(): return started == ["pvp"], "match started")
	var late := new_client()
	await login(late, "late", tcp, udp)
	var refusals := []
	late.server_error.connect(func(code, message): refusals.append(code))
	late.join_match(room)
	await wait_until(func(): return refusals == [ERROR_MATCH_STARTED], "started match refuses joins")

	# ---- owner deletes the match ----
	var deleted := []
	guest.on_match_deleted.connect(func(): deleted.append(1))
	host.delete_match()
	await wait_until(func(): return deleted.size() == 1, "guests see the match deleted")
	check(not host.is_on_a_match() and not guest.is_on_a_match(), "nobody is on the match")

	# ---- disconnecting ----
	var gone := []
	guest.disconnected.connect(func(): gone.append(1))
	var before := Time.get_ticks_msec()
	guest.disconnect_from_server()
	check(Time.get_ticks_msec() - before < 50, "disconnect does not block")
	check(gone.size() == 1 and not guest.is_connected_to_server(), "disconnected")

	host.disconnect_from_server()
	late.disconnect_from_server()
	print("ALL PASSED" if failures == 0 else "FAILED: %d" % failures)
	quit(1 if failures else 0)
