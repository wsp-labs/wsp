# 06 Processes and lifecycle

How the host comes up, holds its state file, starts and stops children, re-opens turns after a restart, and goes down. Every `file:line` is at `a3b870a6e`. `daemon/crates/wsp-mcp/src/start.rs` and `aim.rs` already hold a Rust copy of the verb-side starter and the lock reader, with two differences noted below.

## The lock

`host.lock` beside the state file (`packages/host/src/host-lock.ts:132-134`) says which process serves a state file. Its JSON and key order are rule 2.2's: `{"pid":4242,"port":4400,"address":"127.0.0.1","startedBy":"up","startedAt":"2026-10-25T10:00:00.000Z"}`, `startedBy` one of `verb`, `up`, `service` and absent for a host served inside another process (`wsp init`, the app's own), which nothing may stop (`host-lock.ts:10-20`; `protocol/src/release.ts:18`).

6.1. A lock **holds** when `kill(pid, 0)` succeeds or fails with EPERM, and the pid's `/proc/<pid>/exe` does not name a program that is gone (`holds`, `host-lock.ts:62-100`). `programGone(path)` is `!existsSync(path without a trailing " (deleted)")` (`host-lock.ts:84`): a new file at the same path is not gone, so an update in place keeps the old host serving. With no `/proc` (any Mac) the program is never gone. There is no start-time check against pid reuse. The Rust reader in `wsp-mcp/src/aim.rs:166-175` skips the `/proc` half; the Rust host MUST do both.

6.2. `takeLock` (`host-lock.ts:259-283`) MUST, in order: refuse if a lock holds (`another wsp host (pid N) is already serving <state> on port P.` plus `wsp down stops it, or point --state at a different file.` when `startedBy` is set, else `Stop it first, or point --state at a different file.`, `host-lock.ts:125-130`); make the state folder 0700; write the JSON to `host.lock.<pid>` and hard-link it to `host.lock`, falling back to `open(O_EXCL)` on EPERM, ENOTSUP or EXDEV (`host-lock.ts:363-383`); when a stale lock stands, take it over under a marker: write `host.lock.<pid>.taking` holding `{"pid":N}`, hard-link it to `host.lock.taking.<n>` for n from 0, read a held marker's pid (a live one refuses with `another wsp host (pid N) is starting on <state> right now; it serves in a moment.`; one with no pid younger than 10 s refuses alike; a dead or old one means the next n), re-read the lock, remove it, link ours, remove our marker (`host-lock.ts:291-341`); then sweep every leftover marker.

6.3. The lock MUST be written with the **requested** port (0 for a verb start) and rewritten by rename with the bound port after the bind (`rewriteLock`, `host-lock.ts:356-360`; `cli/serve.ts:271,356`). A reader that dials a lock still saying port 0 fails its probe and waits (6.6).

6.4. `releaseLock` MUST remove the lock only when its pid is this process (`host-lock.ts:350-352`). The `host-token` file is never removed.

6.5. `WSP_STARTED_BY` MUST be deleted from the host's own environment right after the lock is taken (`cli/serve.ts:274`), so turns, the local daemon and pane shells never inherit it. `startedByEnv(env)` reads only `verb` and `service` from the environment; `up` is the default (`host-lock.ts:29-35`).

## Starting a host

`wsp up` is chapter 01's start order. `up()` returns 0 and the process keeps running on its open sockets and timers; the entry sets `process.exitCode` and never calls `process.exit` (`packages/host/src/entry.ts:41-51`).

6.6. A line that needs a host and finds none (`heldOrStarted`, `host-lock.ts:240-242`, only when the aim is this computer) MUST start one exactly as `hostStarter` does (`packages/host/src/host-start.ts:68-113`):
1. refuse if a service unit file exists for this state file and nothing serves: `<state> is served by the <launchd agent|systemd user unit> <name>, which is not running; wsp up --service --state <state> starts it again`;
2. open `host.log` beside the state for append, noting its size as the mark where this child's lines begin;
3. spawn `[...wsp.args, "up", "--state", <state>, "--port", "0"]` (or the held port on a restart), detached (a new session and process group), stdin ignored, stdout and stderr the log, environment the parent's plus `WSP_STARTED_BY=verb`, no cwd; unref it and close the parent's log handle;
4. say on stderr `starting the host for <state>; its log is <host.log>, and wsp down stops it`;
5. poll the lock every 200 ms for up to 20 s (`SERVICE_WAIT_MS`), until it holds AND an HTTP `GET /` to its address answers anything within 2 s (`httpProbe`, `service.ts:537-550`), or the child exits;
6. on the child's exit, fail with the non-empty log lines written since the mark, or `the host for <state> exited with <code | signal | "no code"> before it served; its log is <log>`; at the deadline, `no host answered for <state> within 20 s` and the last 20 non-empty lines of the log's last 64 KiB.

`wsp.command` (`wspCommand`, `packages/host/src/mcp-install.ts:94-101`) is the desktop's shim when the process runs behind it; `npx -y @wsp-labs/wsp@<version>` when run from an `_npx` folder; the `wsp` on PATH when it is the same file; else `process.execPath` with its exec args and the script. The Rust starter in `wsp-mcp/src/start.rs:24-77` clears the environment and hands in its own; the TS starter passes its whole environment. The Rust host MUST pass the whole environment.

6.7. Two lines that start a host at once need no coordination: the second child's `takeLock` refuses and exits, and the first line's wait finds the first child's lock (`host-start.ts:101-103`).

## The service

One service per state file, named by the first 8 hex characters of the sha256 of the state path as given (`serviceTag`, `packages/host/src/service.ts:138-140`). macOS: a launchd agent `com.wsp.host.<tag>` at `~/Library/LaunchAgents/com.wsp.host.<tag>.plist`. Linux: a systemd user unit `wsp-host-<tag>.service` at `~/.config/systemd/user/`; a joined computer's agent is the system unit `wsp-place-<tag>.service` in `/etc/systemd/system/`. Any other platform refuses with `wsp writes no service on <platform>; it writes a launchd agent on darwin and a systemd unit on linux. Run wsp up in a terminal that stays open instead.` (`service.ts:363-374`). Files are written with mode 0600 when new.

The plist, exactly (`service.ts:197-220`); `xml()` escapes `&`, `<` and `>` only; one `<string>` per argv word and one key line per environment entry, in order; the file ends with a newline:

```
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.wsp.host.<tag></string>
  <key>ProgramArguments</key>
  <array>
    <string><argv word></string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key><NAME></key><string><value></string>
  </dict>
  <key>WorkingDirectory</key><string><cwd></string>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string><state folder>/host.log</string>
  <key>StandardErrorPath</key><string><state folder>/host.log</string>
</dict>
</plist>
```

The systemd unit, exactly (`service.ts:282-305`); every `ExecStart` word and every `Environment=` pair goes through `shellQuote`, `WorkingDirectory` is bare (a quoted one makes systemd refuse the unit), and the file ends with a newline:

```
[Unit]
Description=wsp host serving <statePath>

[Service]
Type=simple
ExecStart='<argv word>' '<argv word>'
WorkingDirectory=<cwd>
Environment='<NAME>=<value>'
Restart=always
RestartSec=5
KillMode=process
StandardOutput=append:<state folder>/host.log
StandardError=append:<state folder>/host.log

[Install]
WantedBy=default.target
```

6.8. `KillMode=process` MUST be in the host's unit (`service.ts:296-298`): the default kills the unit's whole cgroup, turns included, and every turn leads its own process group precisely so the next host can re-open it. The place unit keeps the default and uses `WantedBy=multi-user.target`.

6.9. The unit's argv MUST be `[<node>, <script>, "up", ...]` or `[<shim>, "up", ...]` followed by `SERVE_FLAGS` in order (`--state`, `--port`, `--listen`, then `--advertise`, `--provider`, `--no-relay` when set), port and address written as resolved even when not typed (`cli/service.ts:119-125`; `cli/flags.ts:39-46`). The cwd is the installing process's, so its `.env` stays the one read. The environment is `serviceEnv` (`service.ts:148-161`): `WSP_HOME` when named, then, only when non-empty, `WSP_CLOUD`, `WSP_LABS`, `WSP_UPDATE_CHECK`, `WSP_ANALYTICS`, `WSP_STATE_STORE`, `SHELL`, `WSP_PROVIDER`, then `WSP_STARTED_BY=service` last. No keys and no PATH: the host reads the login shell at its own start (6.14). Under the Rust host `WSP_STATE_STORE` goes with the JSON store (02).

6.10. The manager commands MUST be: launchd load `launchctl enable gui/<uid>/<label>` then `launchctl bootstrap gui/<uid> <plist>`; unload `launchctl bootout gui/<uid>/<label>`; held `launchctl print gui/<uid>/<label>` exit 0, pid from `^\tpid = (\d+)$`, absent on exit 113 or "could not find service". systemd load `systemctl --user daemon-reload`, `enable`, `restart` (restart, so an old process is replaced); unload `disable --now` then `daemon-reload`; held `show <name> --property=ActiveState,UnitFileState,MainPID`, held when ActiveState is active, activating, deactivating, reloading or refreshing, or UnitFileState is enabled, enabled-runtime, linked or linked-runtime; pid from `MainPID=(\d+)` (`service.ts:186-358`). Each runs with a 15 s bound; a failure's code is the numeric error code, else 127 (a missing binary or a timeout); `runAll` stops at the first non-zero (`service.ts:411-447`).

6.11. `wsp up --service` (`cli/service.ts:165-218`) MUST refuse when a host already serves (`wsp up --service: a wsp host (pid N) is already serving <state> on port P. Stop it first (Ctrl-C in its terminal, or kill N), then run wsp up --service again.`); refuse when the provider's key is only in this shell and in neither `.env`; write the file and load it, deleting the file only if this call created it and the load failed; then wait 20 s for it to serve as 6.6 step 5 does.

6.12. `wsp down` (`cli/service.ts:220-274`) MUST start no host, take the pid off the lock and never off a process search, and: stop the service where one is installed (held by 6.10's reading; an unsure reading changes nothing and says so); stop a host the command line started (`startedBy` `verb` or `up`) with SIGTERM to the lock's pid and poll the lock 20 s; refuse a host with no `startedBy` (`...was started by hand. Stop it with Ctrl-C in its terminal, or kill N.`). A stopped service says `<words> <unit> stopped; nothing serves <state> now, and its running turns keep going until the next host adopts them`.

## Signals and exit codes

6.13. Once serving, `wsp up` (and a host served by `wsp init`) MUST handle SIGINT, SIGTERM and SIGHUP alike: the first runs the host's close and then exits 0, or 1 with `host close failed: <message>` if the close threw; a second signal during the close exits at once with 130, 143 or 129 (`stopOnSignals`, `packages/host/src/cli/serve.ts:108-156`; `exitCodeOf`, `init.ts:696-698`). On Linux only, an unref'd 5 s timer asks `programGone(process.execPath)`, the node or app binary and not the wsp script, and when it is gone says `<execPath>, which this host runs, is gone; it stops so the next start runs what the wsp command names now` and stops as SIGTERM would (host invariant 8; `test/stop-signals.test.ts`). On macOS nothing watches.

6.14. Once serving, and only then, an uncaught exception or unhandled rejection MUST be logged as `uncaught exception, kept serving: <sentence>, <first stack frame>` (or `unhandled rejection, kept serving: ...`) and the host MUST keep running (`stayOnUncaught`, `cli/serve.ts:164-178`). A verb keeps the default, so its exit code says it failed. In Rust this is a panic in a handler task caught at the task boundary and logged; a panic in the state owner is not survivable and is 03's concern.

6.15. Once per process, before a runtime is built, the host MUST read the login shell's environment when `needsLoginPath(env)`: on Linux when `WSP_STARTED_BY=service` or `APPIMAGE` is set, on any platform when PATH is exactly launchd's four folders `/usr/bin /bin /usr/sbin /sbin` in any order (`packages/host/src/login-path.ts:32-107`). The read is `$SHELL -ilc 'env -0'` with the process environment, its own process group, stdin ignored, 15 s, SIGTERM then SIGKILL after 1 s (`collect/src/live-host.ts:177-251`); it sets `PATH` and lends the catalog's sign-in variables the launch lacks, keeping `CLAUDE_CONFIG_DIR` and `CODEX_HOME` aside. A shell that fails changes nothing and logs one line. A verb-started host on Linux does not read it.

The only other process handlers in `packages/*/src`: the warning filter that drops node's sqlite warning (`entry.ts:17-23`), the watch loops' SIGINT, SIGTERM and SIGHUP flags (`watch.ts:23-91`), and `wsp init`'s build-phase handlers (`init.ts:1244-1352`). No `exit`, `beforeExit`, `SIGUSR*`, `SIGPIPE` or `SIGCHLD` handler exists; `process.exit` is called only by the restart road and `wsp init`.

## Shutdown

The close order, outermost first (`cli/serve.ts:388-398`; `server.ts:774-796`; `runtime/src/serve.ts:1966-1979`; `runtime/src/runtime.ts:1225-1277`; `cli/wiring.ts:166-176`):

1. the recipe watcher, usage, the relay (heartbeat cleared, cloudflared SIGTERMed and awaited, `connector.pid` removed);
2. the ssh forwards back to the door, the broker, the reap timer, the release watch, the guest door;
3. the websocket server: every client and place socket closed with 4001 `stopping`, terminated after 1 s;
4. the HTTP servers, all connections closed;
5. `rt.close()`: mark closing; wait for copies moving and the worktree sweep; close idle, alerts and slates (every running slate command group and slate MCP server SIGKILLed); wait for daemon syncs, kept writes and agent asks; end kept agents (each `close({now:true})`: EOF on the stdin channel, SIGTERM to the run's group, SIGKILL after 5 s) but wait for them at most 2 s (`KEPT_CLOSE_WAIT_MS`, `types/internal.ts:655`); drop the readers of running turns; close pipes; close the local wiring (6.22); mark closed; cancel the timers; flush every transcript and session document;
6. analytics; the lock removed last.

6.16. A host stop MUST NOT signal or wait on a running turn: each leads its own group, writes its own log in `runs/`, and is re-opened by the next host (`cli/wiring.ts:167-170`). Kept idle agents are ended; slate runs are killed; the local daemon is stopped; cloudflared is stopped. If the host is killed outright, nothing reaps the local daemon (the next start removes only its token folder) and the next start stops a recorded connector (`local-daemon.ts:47-61`; `connector.ts:262-280`).

## Turns on this computer

A turn is an `ExecStreamFactory` call (`protocol/src/adapter-port.ts:300-327`). On this computer it is `localExecStream` (`runtime/src/local-exec.ts:437-791`); on a cloud machine or box `machineExecStream` (`runtime/src/machine-exec.ts:300-830`); on a joined computer the kind's own road.

6.17. A local launch MUST be exactly (`local-exec.ts:702-755`):
1. `base = <state folder>/runs/<12 lowercase hex from 6 random bytes>`; `runs/` 0700; `mkdir <base>.d` 0700, the claim that says the run lives on this computer;
2. `<base>.sh`, 0600, created exclusive: with no input channel `( <command>\n)\necho $? > '<base>.exit'\n`; with one, `( <command>\n) < <(sh -c 'echo $$ >> <base>.pump; exec tail -n +1 -f <base>.in' | { sh -c 'echo $PPID >> <base>.pump'; while IFS= read -r l; do [ "$l" = '__WSP_EOF_<last 12 of base>__' ] && break; printf '%s\n' "$l"; done; } &)\necho $? > '<base>.exit'\n`; process substitution because node on a Mac never reads the end of a named fifo (`local-exec.ts:711`);
3. `<base>.in` seeded with the launch's lines (or `.held` when `inputAfter` holds them back, `.in` then empty), and `<base>.log` empty, both 0600;
4. `spawn("bash", ["<base>.sh"])` with cwd `<home>/wsp-work`, environment the turn's (07, rule 7.24) plus `WSP_AGENT_PID_FILE=<base>.agent`, stdin ignored, stdout and stderr both the log, detached; unref; write `<base>.pid` as `<pid>\n` at once;
5. save `run = base` and the pid on the session row while it runs (`threads/turns.ts:650`; `threads/transcripts.ts:617-647`).

6.18. The reader MUST poll every 100 ms (1 s between turns of a kept process): read `.pid`, apply the cut rule, read `.exit` **before** the log so an exit code means a complete log, read at most 262,144 bytes of the log from the offset, and split lines, cutting one longer than 16 MiB (`local-exec.ts:451-700`; `machine-exec.ts:268-298`). A turn settles when `.exit` holds a number and the log is drained (the code is that number: 143 for TERM, 137 for KILL); when the leader is dead for two polls with no exit file (`null`); or when a limit cuts it (`null`, with `turnCutLine`). A local stream never reports a signal.

6.19. Limits: the wall limit counts from the row's `startedAt`, so a re-open does not restart it; this computer and joined computers have none, a cloud account 6 h unless set, up to 24 h (`account/channels.ts:89-93`; `protocol/src/place-state.ts:69,213-232`). The idle limit is 10 minutes of quiet (`TURN_IDLE_MS`), touched by log bytes, by a message written into the turn, and while a permission prompt is open; once quiet for half the limit it reads `ps -eo pid=,ppid=,pgid=,time=` and counts the tree's CPU, and the clock moves when the work since the last move is at least `max(1, 5 ticks/s × seconds)` with a tick of 10 ms (`local-exec.ts:131-176`; `machine-exec.ts:105-155`). macOS `ps` prints hundredths, Linux whole seconds (`protocol/src/ps-time.ts`).

6.20. A restarted host MUST re-open each running turn by its stored handle and read its log from `from ?? 0`, byte 0 for a process's first turn (runtime invariant 1; `test/local-exec.test.ts`), and MUST write no row twice: `turnWritten()` counts what was written and the replay skips it (invariant 2). `attach` (`local-exec.ts:757-773`) refuses a handle not shaped `<runs>/<12 hex>`, answers `gone` when `<run>.d` is missing (only this computer's own answer ends a run), reaps and answers `gone` for an empty `.in` (a host that died before writing the prompt), and otherwise reads `.in` (less the EOF marker) as the messages already handed over. A row whose machine does not answer stays running and is asked again after 60 s (`boot.ts:548-562`). After re-opening, `sweepRuns` reaps every claim no running row holds (`threads/turns.ts:748-772`; `local-exec.ts:775-791`).

6.21. Stopping a turn: `endRun(stream, 5000)` MUST send SIGTERM to the run's whole group, wait 5 s, then SIGKILL the group (`adapter-port.ts:362-376`; `local-exec.ts:88-94,621-625`); `endAfterResult` first waits 10 s (`RUN_EXIT_MS`) for the agent to exit after EOF. Every ending, a clean exit included, then runs `reapRun(base, lastRead, 2000)` (`local-exec.ts:299-333`): if the recorded pid still leads this run's group (the group exists; its leader is dead, or its age from `ps -o etime= -p <pid>` is within 5 s of the claim folder's mtime, or `ps` failed), SIGTERM the group, poll every 200 ms for 2 s, SIGKILL the group anyway, read what the log gained, and delete the run's files (`sh log pid agent exit in held late fifo tail pump` and the `.d` folder). So children an agent left in its group always die. Between turns of a kept process only the leftovers die: group members whose parent is outside the group, TERM then KILL after 2 s (`local-exec.ts:214-226,463-469`).

A cloud machine or box runs the same shape through its exec (`machine-exec.ts:738-830`): `/tmp/wsp-run/<12 hex>`, `oom_score_adj` 500 first, `setsid bash <base>.sh > <base>.log 2>&1 &`, polls every 1.5 s (200 ms when hurried, 10 s at rest), reap by `kill -TERM -- -$P`, ten checks 200 ms apart, `kill -KILL -- -$P`; attach answers `gone` only on the machine's own `WSP_GONE` and never on silence.

## The local daemon

6.22. The daemon for this computer MUST start lazily on the first ask and be held; a failed start is forgotten so the next ask tries again, warning once per sentence `the daemon for this computer did not start, so its terminal, files and processes have nothing to dial: <line>` (`cli/wiring.ts:117-129`; `start-once.ts:20-43`). The spawn (`local-daemon.ts:175-257`): `<assets>/daemon/<triple>/wsp-daemon --kind local --host 127.0.0.1 --port 0 --token-path <t> --root <person home> --roots-path <state>/roots --inbox <state>/inbox --work-folder <home>/wsp-work --manifest <own>/manifest.json [--readings-dir <state>/readings] [--docker-notes <state>/docker-notes]`, stdin ignored, not detached, no environment option (it inherits the host's), no cwd. The token is 24 random bytes as 48 hex characters plus a newline, mode 0600, in `mkdtemp(<tmp>/wsp-local-daemon-<host pid>-)`; folders of dead pids are swept first. The port is read from the line `wsp-daemon listening on <host>:<port>` within 10 s; then one link reads `daemon.hello`'s version within 10 s. A failed start stops the child (SIGTERM, SIGKILL after 5 s) and removes the folder. There is no automatic replacement when it is behind: the copy road and the doctor refuse (`folders.ts:77-81`), and `workspaces.restartDaemon` restarts it on request. Close is SIGTERM, SIGKILL after 5 s, await, remove the folder.

## Spawn helpers

The TS host has one helper per road, and they disagree on what a timeout reports. Each MUST be ported with its own behaviour:

| Helper | Spawn | Bound | On the bound | Exit code reported | Output | Where |
|---|---|---|---|---|---|---|
| `runChild` | detached; `cwd`, `env` as given (replaces) | none unless given | SIGKILL the group | 124; unstartable 127 with the error appended to stderr; killed by a signal -1 | two utf8 strings, no cap; pipes drained 200 ms quiet or 1 s after exit | `engine/src/child-exec.ts:63-140` |
| `LocalMachine.exec` | `runChild("bash", ["-c", cmd])`, cwd `wsp-work` | per call | as `runChild` | as `runChild` | | `engine/src/local-backend.ts:101-125` |
| `runHere` | `/bin/bash -c`, cwd home, `HOME` set, detached | 60 s | SIGKILL the group | the close code, or **1** when null (a timeout reads 1); start error 127 | | `host/src/target-road.ts:24-50` |
| `spawnRun` | detached, stdin ignored, stderr dropped | 120 s | SIGTERM the group, settle failed after 1 s | none: stdout only on exit 0, else nothing | 64 MiB cap | `collect/src/live-host.ts:177-246` |
| `systemRunner` | `execFile` | 15 s | SIGTERM the single child | numeric code, else **127** | stdout and stderr joined | `host/src/service.ts:421-429` |
| slate runs | `bash -c <cmd> bash <args...>`, detached | `min(max(timeout ?? 60, 1), 600)` s | SIGKILL the group | 124, `timed out after <n> s` | 2 MiB shared, then the last 64 KiB characters kept | `runtime/src/slate-runs.ts:296-359,985-987` |
| `boundedCommand` | shell text on a machine: `set -m; <cmd> & p=$!; ( sleep N; kill -KILL -- -$p ) ...; wait $p; ...; [ $r -ne 137 ]` | N s | SIGKILL the group | non-zero when killed | | `engine/src/machine-context.ts:194-199` (macOS has no `timeout`) |

6.23. A port MUST keep each helper's own timeout code: `runChild` 124, `runHere` 1, `systemRunner` 127, `spawnRun` none. Callers read these codes (`exit 124 = "was stopped after 3 minutes"`, `folders.ts:115-125`). Unifying them is an after-port ticket.

6.24. Who leads a process group (spawned detached, so signalled as a group): every turn, slate run, slate `then`, slate MCP server, `runChild` child, `runHere`, `spawnRun`, `codex app-server` login read, editor program, and the verb-started host. Not detached: the local daemon, the guest tool server, cloudflared, the ssh forward, `wsp mcp`'s binary. Every liveness check `kill(pid, 0)` reads EPERM as alive (`host-lock.ts:66-71`; `local-exec.ts:98-102,258-265`; `connector.ts:231-235`; `sqlite-store.ts:209`).

## Restart and update

6.25. The host never re-execs itself. `host.restart` (serve.ts:1899-1915) refuses any socket but the own road, answers `ok` first, then runs the road the lock's `startedBy` picks (`packages/host/src/restart.ts:55-86`): `service` closes and exits 0 (1 if the close threw) for the manager to start it again; `verb` closes, then starts a successor exactly as 6.6 does on the held port and exits 0 once it serves, 1 if it does not; `up` refuses with `Ctrl-C the terminal running wsp up and run it again.`; a host with no `startedBy` says `This host cannot restart itself.` A second ask while closing is ignored; a failed restart leaves the host serving.

6.26. Nothing restarts on an update except the Linux program-gone stop (6.13). The release watch (`release.ts:126-236`) only reads GitHub: 30 s after start, then every 6 h, never more often than every 10 minutes, cached in `release.json`, showing `installed` (read fresh from `package.json`) beside `running`.

What a running turn sees across a restart: nothing. It was never signalled, its stdout is its log file, its stdin is the file-fed pump, its group and session are its own; its `WSP_HOST_URL` names the same port (the successor takes the held port, a service names `--port`), and its thread token stays valid, so its tools reconnect after a few seconds of refused connections. Waiting clients saw 4001 `stopping` and wait for the host to come back (`verbs/client.ts:202,399`).
