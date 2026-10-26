# 06 Processes and lifecycle

How the host comes up, holds its state file, starts and stops children, re-opens turns after a restart, and goes down. Every `file:line` is at `a3b870a6e`. `daemon/crates/wsp-mcp/src/start.rs` and `aim.rs` already hold a Rust copy of the verb-side starter and the lock reader, with two differences noted below.

## The lock

`host.lock` beside the state file (`packages/host/src/host-lock.ts:132-134`) says which process serves a state file. Its JSON and key order are rule 2.2's: `{"pid":4242,"port":4400,"address":"127.0.0.1","startedBy":"up","startedAt":"2026-10-25T10:00:00.000Z"}`, `startedBy` one of `verb`, `up`, `service` and absent for a host served inside another process (`wsp init`, the app's own), which nothing may stop (`host-lock.ts:10-20`; `protocol/src/release.ts:18`).

6.1. A lock **holds** when `kill(pid, 0)` succeeds or fails with EPERM, and the pid's `/proc/<pid>/exe` does not name a program that is gone (`holds`, `host-lock.ts:62-100`). `programGone(path)` is `!existsSync(path without a trailing " (deleted)")` (`host-lock.ts:84`): a new file at the same path is not gone, so an update in place keeps the old host serving. With no `/proc` (any Mac) the program is never gone. There is no start-time check against pid reuse. The Rust reader in `wsp-mcp/src/aim.rs:166-175` skips the `/proc` half; the Rust host MUST do both.

6.2. `takeLock` (`host-lock.ts:259-280`) MUST, in order: refuse if a lock holds (`another wsp host (pid N) is already serving <state> on port P.` plus `wsp down stops it, or point --state at a different file.` when `startedBy` is set, else `Stop it first, or point --state at a different file.`, `host-lock.ts:125-130`); make the state folder 0700; write the JSON to `host.lock.<pid>` and hard-link it to `host.lock`, falling back to `open(O_EXCL)` on EPERM, ENOTSUP or EXDEV (`host-lock.ts:363-383`); when a stale lock stands, take it over under a marker: write `host.lock.<pid>.taking` holding `{"pid":N}`, hard-link it to `host.lock.taking.<n>` for n from 0, read a held marker's pid (a live one refuses with `another wsp host (pid N) is starting on <state> right now; it serves in a moment.`; one with no pid and younger than 10 s refuses with the same sentence less ` (pid N)` (`host-lock.ts:319`); a dead or old one means the next n, with no bound on n, `:315`), re-read the lock and, if its text changed, refuse with the winner's `is already serving` sentence, or `another wsp host just took <lock>` when the winner does not read as a lock (`:298-301`, `tookFirst` at `:343-346`), remove it, link ours, remove our marker (`host-lock.ts:291-341`); then sweep: every `host.lock.taking.*` goes, and a `host.lock.<pid>.taking` only when that pid is dead; a leftover `host.lock.<pid>` is not swept (`:272-278`).

6.3. The lock MUST be written with the **requested** port (0 for a verb start) and rewritten by rename with the bound port after the bind (`rewriteLock`, `host-lock.ts:356-360`; `cli/serve.ts:271,356`). A reader that dials a lock still saying port 0 fails its probe and waits (6.6).

6.4. `releaseLock` MUST remove the lock only when its pid is this process (`host-lock.ts:350-352`). The `host-token` file is never removed.

6.5. `WSP_STARTED_BY` MUST be deleted from the host's own environment right after the lock is taken (`cli/serve.ts:274`), so turns, the local daemon and pane shells never inherit it. `startedByEnv(env)` reads only `verb` and `service` from the environment (`host-lock.ts:29-35`), and the environment's word wins over the caller's (`started = startedByEnv(process.env) ?? opts.startedBy`, `cli/serve.ts:270`); `wsp up` passes `up` (`opts.startedBy ?? "up"`, `cli/service.ts:62`).

6.27. A lock MUST be read as the TS reader reads it (`readLock`, `isHostLock`, `host-lock.ts:45-123`): a missing, unreadable or empty file, text that is not JSON, or JSON whose `pid` or `port` is not a number or whose `startedAt` is not a string is **no lock**; extra keys are ignored, and `address` and `startedBy` are not type-checked. A pid is not checked to be a positive integer: `kill(0, 0)` and `kill(-1, 0)` succeed, so a lock naming pid 0 or -1 holds for ever, and a fractional pid or one at 2^31 or past throws a type error, which is not EPERM, so it reads gone. A port of 0 is accepted and the dial fails its probe.

6.28. A start MUST take over every lock that does not hold, through 6.2's markers, and refuse every lock that does. Measured on a throwaway host (2026-10-25): a lock naming a dead pid, an empty lock, a lock that is not JSON and a lock with no `pid` are each replaced by the new host's own, and no marker is left; a lock `{"pid":0,"port":1,"startedAt":"x"}` refuses the start:

```text
another wsp host (pid 0) is already serving <state> on port 1. Stop it first, or point --state at a different file.
```

> **After the port.** A pid of 0 or -1 should not hold, and `wsp down` sends SIGTERM to the lock's pid unchecked (`cli/service.ts:111,251`), which for a lock with `startedBy` set and pid 0 or -1 signals the caller's process group or every process it may signal. The port keeps both; the ticket is to require a pid above 0.

6.29. Only `statePathFrom` resolves a path: `resolve(<pick>)` against the cwd, with no realpath, which strips trailing slashes and `..` (`cli/state.ts:64-68`, from `cli/flags.ts:82`). `lockPathFor`, `hostLogPath`, `hostTokenPath` and `serviceTag` take the path as given, and `serviceTag` hashes its UTF-8 bytes (`service.ts:139`), so a symlinked spelling of one state file names a second unit. The unit's argv writes the script as `resolve(process.argv[1])` and `--state <resolved>`.

## Starting a host

`wsp up` is chapter 01's start order. `up()` returns 0 and the process keeps running on its open sockets and timers; the entry sets `process.exitCode` and never calls `process.exit` (`packages/host/src/entry.ts:42-53`); the exit comes from the signal handler (6.13).

6.6. A line that needs a host and finds none (`heldOrStarted`, `host-lock.ts:240-242`, only when the aim is this computer) MUST start one exactly as `hostStarter` does (`packages/host/src/host-start.ts:68-113`):
1. refuse if a service unit file exists for this state file and nothing serves: `<state> is served by the <launchd agent|systemd user unit> <name>, which is not running; wsp up --service --state <state> starts it again`;
2. open `host.log` beside the state for append, noting its size as the mark where this child's lines begin;
3. spawn `[...wsp.args, "up", "--state", <state>, "--port", "0"]` (or the held port on a restart), detached (a new session and process group), stdin ignored, stdout and stderr the log, environment the parent's plus `WSP_STARTED_BY=verb`, no cwd; unref it and close the parent's log handle;
4. say on stderr `starting the host for <state>; its log is <host.log>, and wsp down stops it`;
5. poll the lock every 200 ms for up to 20 s (`SERVICE_WAIT_MS`), until it holds AND an HTTP `GET /` to its address answers anything within 2 s (`httpProbe`, `service.ts:537-550`), or the child exits;
6. on the child's exit, fail with the non-empty log lines written since the mark, or `the host for <state> exited with <code | signal | "no code"> before it served; its log is <log>`; at the deadline, `no host answered for <state> within 20s` (the duration through `fmtDuration`, so no space, `protocol/src/words/units.ts:375,380-388`) and the last 20 non-empty lines of the log's last 64 KiB. The deadline is checked after each probe, so the wait can run to about 22 s (`service.ts:568-573`). Only the child's `exit` is listened for, so a spawn that fails to start (ENOENT) is an unhandled `error` event in the verb's process (`host-start.ts:94-99`).

`wsp.command` (`wspCommand`, `packages/host/src/mcp-install.ts:94-101`) is the desktop's shim when the process runs behind it; `npx -y @wsp-labs/wsp@<version>` when run from an `_npx` folder; the `wsp` on PATH when it is the same file; else `process.execPath` with its exec args and the script. The Rust starter in `wsp-mcp/src/start.rs:24-77` clears the environment and hands in its own; the TS starter passes its whole environment. The Rust host MUST pass the whole environment.

6.7. Two lines that start a host at once need no coordination: the second child's `takeLock` refuses and exits, and the first line's wait finds the first child's lock (`host-start.ts:101-103`).

6.30. `host.log` MUST be appended to, never truncated or rotated, by each of its three writers: the verb starter (`openSync(path, "a")` at the umask, its folder made with `mkdirSync(dir, {recursive: true})` at the umask, `host-start.ts:74-78`), launchd (`StandardOutPath`) and systemd (`append:`). A host started by `wsp up` in a terminal writes no log. A missing log reads as size 0 and no lines (`logSize`, `logSince`, `logTail`, `service.ts:594-635`); `logSince` has no cap, and `logTail` cuts by bytes at 64 KiB and drops the first partial line only when it cut.

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

6.9. The unit's argv MUST be `[<node>, <script>, "up", ...]` or `[<shim>, "up", ...]` followed by `SERVE_FLAGS` in order (`--state`, `--port`, `--listen`, then `--advertise`, `--provider`, `--no-relay` when set), port and address written as resolved even when not typed (`cli/service.ts:119-125`; `cli/flags.ts:39-46`). The cwd is the installing process's, so its `.env` stays the one read. The environment is `serviceEnv` (`service.ts:148-161`): `WSP_HOME` when named, then, only when non-empty (whitespace counts as non-empty), `WSP_CLOUD`, `WSP_LABS`, `WSP_UPDATE_CHECK`, `WSP_ANALYTICS`, `WSP_STATE_STORE`, `SHELL`, then `providerEnvNames()` in its order `WSP_PROVIDER`, `WSP_FAKE_AS`, `WSP_FAKE_ROOT`, `WSP_FAKE_RECORDS` (`service.ts:156`; `providers.ts:116,170`), then `WSP_STARTED_BY=service` last (`cli/service.ts:194`). No value is escaped for `%` or `$` in the unit. No keys and no PATH: the host reads the login shell at its own start (6.14). Under the Rust host `WSP_STATE_STORE` goes with the JSON store (02).

6.10. The manager commands MUST be: launchd load `launchctl enable gui/<uid>/<label>` then `launchctl bootstrap gui/<uid> <plist>`; unload `launchctl bootout gui/<uid>/<label>`; held `launchctl print gui/<uid>/<label>` exit 0, pid from `^\tpid = (\d+)$`, absent on exit 113 or "could not find service". systemd load `systemctl --user daemon-reload`, `enable`, `restart` (restart, so an old process is replaced); unload `disable --now` then `daemon-reload`; held `show <name> --property=ActiveState,UnitFileState,MainPID`, held when ActiveState is active, activating, deactivating, reloading or refreshing, or UnitFileState is enabled, enabled-runtime, linked or linked-runtime; pid from `MainPID=(\d+)` (`service.ts:186-358`). Each runs with a 15 s bound (`MANAGER_WAIT_MS`), except a joined computer's leave, 100 s (`STOP_WAIT_MS`, `service.ts:417`; `place-report.ts:350`); a failure's code is the numeric error code, else 127 (a missing binary, a timeout, or output past `execFile`'s default 1 MiB `maxBuffer`, whose code is a string); `runAll` stops at the first non-zero (`service.ts:411-447`).

6.11. `wsp up --service` (`cli/service.ts:165-218`) MUST refuse when a host already serves (`wsp up --service: a wsp host (pid N) is already serving <state> on port P. Stop it first (Ctrl-C in its terminal, or kill N), then run wsp up --service again.`); refuse when the provider's key is only in this shell and in neither `.env`; write the file and load it, deleting the file only if this call created it and the load failed; then wait 20 s for it to serve as 6.6 step 5 does.

6.12. `wsp down` (`cli/service.ts:220-274`) MUST start no host and take the pid off the lock, never off a process search. In order:
1. with no service manager for the platform, print the `wsp writes no service on ...` sentence of the service paragraph above (`service.ts:370-374`) and exit 1, before reading the lock (so a verb-started host on such a platform cannot be stopped by `wsp down`);
2. ask the manager whether the unit holds, with the 15 s bound, even when no unit file exists;
3. when that reading is unsure and the host was not started by a line, change nothing and exit 1 with `wsp down: <runFailureLine>, so wsp cannot tell whether the <words> <name> is still loaded. ` plus `Its unit file is still <path>; nothing was changed.` or `Nothing was changed.` (`runFailureLine` is `<argv> exited <code> and said: <text>` or `... and said nothing`, `service.ts:454-458`);
4. when the unit holds, unload it and remove its file (`service.ts:504-505`); a failed unload exits 1 with `wsp down: <runFailureLine>`; then, unless step 5 applies, wait 20 s for the lock to go: a host left behind gives `the <words> <name> is gone, but the host it started (pid N) is still serving <state>.` and exit 1, otherwise `<words> <name> stopped; nothing serves <state> now, and its running turns keep going until the next host adopts them`, exit 0, which a unit file the manager no longer held reaches too;
5. when no unit was held and no unit file stands, or the lock's `startedBy` is `verb` or `up` whatever the manager held (`lineStarted`, `cli/service.ts:233,246`), and the lock's `startedBy` is set (any value, `service` included, so a service host whose unit is gone is stopped here), send SIGTERM to the lock's pid (`cli/service.ts:111`) and poll the lock for 20 s with no SIGKILL after: `stopped the host <road> started (pid N); nothing serves <state> now`, exit 0, or `wsp down: the host <road> started (pid N) is still serving <state>.`, exit 1, `<road>` being `wsp up`, `a verb` or `the service`;
6. a lock with no `startedBy`: `wsp down: no <words> for <state>; the host serving it (pid N) was started by hand. Stop it with Ctrl-C in its terminal, or kill N.`, exit 1; no unit and no host: `wsp down: no <words> for <state>, and no host is serving it.`, exit 1.

A SIGTERM that fails (EPERM for another login's pid) throws out of the verb, which prints the message and exits 1 (`entry.ts:47-52`).

## Signals and exit codes

6.13. Once serving, `wsp up` (and a host served by `wsp init`) MUST handle SIGINT, SIGTERM and SIGHUP alike: the first runs the host's close and then exits 0, or 1 with `host close failed: <message>` if the close threw; a second signal during the close exits at once with 130, 143 or 129 (`stopOnSignals`, `packages/host/src/cli/serve.ts:108-156`; `exitCodeOf`, `init.ts:696-698`). On Linux only, an unref'd 5 s timer asks `programGone(process.execPath)`, the node or app binary and not the wsp script, and when it is gone says `<execPath>, which this host runs, is gone; it stops so the next start runs what the wsp command names now` and stops as SIGTERM would (host invariant 8; `test/stop-signals.test.ts`). On macOS nothing watches.

6.14. Once serving, and only then, an uncaught exception or unhandled rejection MUST be logged as `uncaught exception, kept serving: <sentence>, <first stack frame>` (or `unhandled rejection, kept serving: ...`) and the host MUST keep running (`stayOnUncaught`, `cli/serve.ts:164-178`). A verb keeps the default, so its exit code says it failed. In Rust this is a panic in a handler task caught at the task boundary and logged; a panic in the state owner is not survivable and is 03's concern.

6.15. Once per process, before a runtime is built, the host MUST read the login shell's environment when `needsLoginPath(env)`: on Linux when `WSP_STARTED_BY=service` or `APPIMAGE` is set, on any platform when PATH is exactly launchd's four folders `/usr/bin /bin /usr/sbin /sbin` in any order (`packages/host/src/login-path.ts:32-107`). The read is `$SHELL -ilc 'env -0'` with the process environment, its own process group, stdin ignored, 15 s, SIGTERM then SIGKILL after 1 s (`collect/src/live-host.ts:177-251`); it sets `PATH` and lends the catalog's sign-in variables the launch lacks, keeping `CLAUDE_CONFIG_DIR` and `CODEX_HOME` aside. A shell that fails changes nothing and logs one line. A verb-started host on Linux does not read it.

6.31. Signals MUST behave as measured on a throwaway host (`env -i`, `--port 0`, 2026-10-25): one SIGTERM, one SIGHUP or one SIGINT to a serving host runs the close, exits 0 and leaves no `host.lock`; a SIGTERM before the host serves (0.15 s after the spawn) meets node's default handler and exits 143 with no close, because the handlers are installed only after `up()` returns (`cli/commands.ts:145-146`; `cli/init.ts:258,322`); a SIGKILL leaves `host.lock` and `host-token`, and the next start takes the lock over (6.28). The close has no deadline: a close that never settles ends only on a second signal, which exits with that signal's code. A close that throws skips `releaseLock` (no `finally`, `cli/serve.ts:390-397`), so the process exits 1 with `host close failed: <message>` and leaves a stale lock. Two signals sent back to back reach node as one, so the second-signal path has no measured line here; `packages/host/test/stop-signals.test.ts` holds it.

```text
one SIGTERM, serving          exit 0    host.lock gone
one SIGHUP, serving           exit 0
one SIGINT, serving           exit 0    host.lock gone
SIGTERM before serving        exit 143  no lock written
SIGKILL, serving              host.lock and host-token left
```

The only other process handlers in `packages/*/src`: the warning filter that drops node's sqlite warning (`entry.ts:17-23`), the watch loops' SIGINT, SIGTERM and SIGHUP flags (`watch.ts:23-91`), `wsp init`'s build-phase handlers (`init.ts:1244-1352`), its `exit` hook (`atExit`, `cli/init.ts:47`) and its side-by-side SIGINT (`init-beside.ts:136`). No `beforeExit`, `SIGUSR*`, `SIGPIPE` or `SIGCHLD` handler exists. `process.exit` is called by `stopOnSignals` (the serving host's normal exit, `cli/serve.ts:136,141,144`), the restart road (`cli/serve.ts:289`; `restart.ts`) and `wsp init` (`cli/init.ts:46`; `init.ts:1242,1248,1284,1444`).

## Shutdown

The close order, outermost first (`cli/serve.ts:388-398`; `server.ts:774-796`; `runtime/src/serve.ts:1966-1979`; `runtime/src/runtime.ts:1225-1277`; `cli/wiring.ts:166-176`):

1. the recipe watcher, usage, the relay (heartbeat cleared, cloudflared SIGTERMed and awaited, `connector.pid` removed);
2. the ssh forwards back to the door (`opts.back.close()`, which lets go of held logins, `place-back.ts:377`), the broker, the reap timer, the release watch, the callback relay (`startCallbackRelay`'s close: its detaches, every guest, every link; `server.ts:781`);
3. the websocket server: every client and place socket closed with 4001 `stopping`, terminated after 1 s;
4. the HTTP servers, all connections closed;
5. `rt.close()` (`runtime.ts:1225-1283`), in this order: mark closing; clear the roots recheck; run each `heldAtClose` writer, which records one `session.context` per running turn (`threads/turns.ts:117-123`); wait for copies moving; stop the worktree sweep and wait for one in flight; stop the disk watch; close idle, alerts and slates (every running slate command group and slate MCP server SIGKILLed, 6.40); close the agents reader; wait for daemon syncs, kept writes and agent asks (each `allSettled`); end kept agents (each `reapKept(id, {now: true})`: EOF on the stdin channel, SIGTERM to the run's group, SIGKILL after 5 s) but wait for them at most 2 s (`KEPT_CLOSE_WAIT_MS`, `types/internal.ts:655`); drop the readers of machine turns; close pipes; close the local wiring (6.22); close the place door; mark closed; stop the heartbeat; cancel the grace, snooze and resume timers and each workspace's pull request poll; close the gone watch; wait for a status tick in flight; take this host's `heldBy` off each builder it owns and write it; flush every transcript, then wait for the transcript queue and the index flushes (02, rule 2.33);
6. the stand-in guests (`closeStandInGuests`, `server.ts:794`); analytics; the lock removed last.

6.16. A host stop MUST NOT signal or wait on a running turn (a kept agent between turns is not a running turn, and is ended): each leads its own group, writes its own log in `runs/`, and is re-opened by the next host (`cli/wiring.ts:167-170`). Kept idle agents are ended; slate runs are killed; the local daemon is stopped; cloudflared is stopped. If the host is killed outright, nothing reaps the local daemon (the next start removes only its token folder) and the next start stops a recorded connector (`local-daemon.ts:47-61`; `connector.ts:262-280`).

6.32. What survives a host stop MUST be what survives the TS host's: the turns' process groups, every file in `runs/` and the turns' pump processes; `host-token` (rewritten at the next start); `door-port`; `host.log`; the service unit file, until `wsp down` removes it; `readings/`, `inbox/` and `roots`. A clean stop removes `host.lock`, the local daemon's token folder and `connector.pid`; a SIGKILL leaves all three, and the local daemon and cloudflared run on (6.38, 6.41). The next host, at its start, re-opens the turns, sweeps the claims no running row holds, revokes turn-scoped devices whose thread is not running (`machines/boot.ts:597-599`), removes dead hosts' token folders, and stops a recorded connector.

6.39. Kept agents at close MUST get only what `rt.close()` gives them: EOF and SIGTERM to the group, then at most 2 s of waiting, after which the signal handler exits the process at once (`cli/serve.ts:141`), so the 5 s SIGKILL is never reached for one that outlives the wait; the next host's sweep ends what is left (`runtime.ts:1248`).

6.40. Slate runs at close MUST be killed and not recorded (`slate-runs.ts:975-981`; `slates.ts:1107-1110`): every run is stopped with no reason, which on this computer is SIGKILL to its group; reshapes are killed and timers cleared; the slate MCP servers and the ledger close. No record is written, so each stays `running` in the store until the next start rewrites it as failed with `the host restarted while it ran` (02, item 13 of the start).

6.41. Cloudflared MUST stop as the TS connector stops it (`connector.ts:210-226`): mark stopping, clear the restart timer, resolve the waiting hostname with the last one found; with no child, remove `connector.pid` and return; otherwise SIGTERM that one pid and await its `exit` with no timeout and no SIGKILL. Its restart backoff is 2 s doubling to 30 s (`MAX_RESTART_MS`). `stopRecordedConnector` (`connector.ts:262-282`; called with `--no-relay` at `cli/serve.ts:386`, at unlink and before a tunnel is asked for, `relay-link.ts:551,676`) removes `connector.pid` first; returns without a signal when the file is not JSON with an integer `pid` above 0 and a string `startedAt`, or when `startedAt` is before the computer's boot (`Date.now() - os.uptime() * 1000`); otherwise sends SIGTERM, reads any kill error (EPERM included) as gone, and polls every 50 ms for up to 5 s, with no SIGKILL.

> **After the port.** A cloudflared that ignores SIGTERM holds the host's close for ever. The port keeps it.

## Turns on this computer

A turn is an `ExecStreamFactory` call (`protocol/src/adapter-port.ts:300-327`). On this computer it is `localExecStream` (`runtime/src/local-exec.ts:437-791`); on a cloud machine or box `machineExecStream` (`runtime/src/machine-exec.ts:300-830`); on a joined computer the kind's own road.

6.17. A local launch MUST be exactly (`local-exec.ts:702-755`):
1. `base = <state folder>/runs/<12 lowercase hex from 6 random bytes>`; `runs/` 0700; `mkdir <base>.d` 0700, the claim that says the run lives on this computer;
2. `<base>.sh`, 0600, created exclusive: with no input channel `( <command>\n)\necho $? > '<base>.exit'\n`; with one, `( <command>\n) < <(sh -c 'echo $$ >> '\''<base>.pump'\''; exec tail -n +1 -f '\''<base>.in'\''' | { sh -c 'echo $PPID >> '\''<base>.pump'\'''; while IFS= read -r l; do [ "$l" = '__WSP_EOF_<last 12 of base>__' ] && break; printf '%s\n' "$l"; done; } &)\necho $? > '<base>.exit'\n`, every path and the marker through `shellQuote` (always `'...'`, an inner `'` as `'\''`, `protocol/src/shell-quote.ts:4-6`), the pump's paths quoted twice because they sit inside a quoted `sh -c` line (`local-exec.ts:714-718`); process substitution because node on a Mac never reads the end of a named fifo (`local-exec.ts:711`);
3. `<base>.in` seeded with the launch's lines (or `.held` when `inputAfter` holds them back, `.in` then empty), and `<base>.log` empty, both 0600;
4. `spawn("bash", ["<base>.sh"])` with cwd `<home>/wsp-work`, environment the turn's (07, rule 7.24) plus `WSP_AGENT_PID_FILE=<base>.agent`, stdin ignored, stdout and stderr both the log, detached; unref; write `<base>.pid` as `<pid>\n` at once;
5. save `run = base` on the stored session row while it runs, and the pid on the in-memory entry only: it is never written down (`threads/turns.ts:650`; `threads/transcripts.ts:617-647`; `protocol/src/views/session.ts:243-247`).

6.18. The reader MUST poll every 100 ms (1 s between turns of a kept process): read `.pid`, apply the cut rule, read `.exit` **before** the log so an exit code means a complete log, read at most 262,144 bytes of the log from the offset, and split lines, cutting one longer than 16 MiB (`local-exec.ts:451-700`; `machine-exec.ts:268-298`). A turn settles when `.exit` holds a number and the log is drained (the code is `parseInt` of it); when the leader has read dead on two polls with no exit file, counted over the turn's life and never reset (`downs`, `local-exec.ts:502,601`), a missing, empty or non-numeric `.pid` reading dead (`null`); or when a limit cuts it (`null`, with `turnCutLine`). A local stream never reports a signal. A signal to the run's whole group (every stop, 6.21) kills the script's `bash` too, so `.exit` is never written and the turn settles `null` by the dead-leader rule; a code of 143 or 137 appears only when the agent alone dies of that signal and the script lives to write it (measured: a TERM to a detached `bash x.sh` group leaves no exit file, 2026-10-25).

6.19. Limits: the wall limit counts from the row's `startedAt`, so a re-open does not restart it; this computer and joined computers have none where the door lists their row (otherwise the 6 h default, 6.37), a cloud account 6 h unless set, up to 24 h (`account/channels.ts:89-93`; `protocol/src/place-state.ts:69,213-232`). The idle limit is 10 minutes of quiet (`TURN_IDLE_MS`), touched by log bytes, by a message written into the turn, and while a permission prompt is open; once quiet for half the limit it reads `ps -eo pid=,ppid=,pgid=,time=` and counts the tree's CPU, and the clock moves when the work since the last move is at least `max(1, 5 ticks/s × seconds)` with a tick of 10 ms (`local-exec.ts:131-176`; `machine-exec.ts:105-155`). macOS `ps` prints hundredths, Linux whole seconds (`protocol/src/ps-time.ts`).

6.20. A restarted host MUST re-open each running turn by its stored handle and read its log from `from ?? 0`, byte 0 for a process's first turn (runtime invariant 1; `test/local-exec.test.ts`), and MUST write no row twice: `turnWritten()` counts what was written and the replay skips it (invariant 2). `attach` (`local-exec.ts:757-773`) refuses a handle not shaped `<runs>/<12 hex>`, answers `gone` when `<run>.d` is missing (only this computer's own answer ends a run), reaps and answers `gone` for an empty `.in` (a host that died before writing the prompt), and otherwise reads `.in` (less the EOF marker) as the messages already handed over. A row whose machine does not answer stays running and is asked again after 60 s (`machines/boot.ts:548-562`). After re-opening, `sweepRuns` reaps every claim no running row holds (`threads/turns.ts:748-766`; `local-exec.ts:775-791`); it finds claims by their `.d` folder alone, and keeps a run only when the stored handle matches the path exactly.

> **After the port.** A handle `attach` refuses (not `<runs>/<12 lowercase hex>`, a trailing slash included) throws, and `reattach` turns any throw into `unreached` (`threads/turns.ts:822-825`), so the row stays running and is asked again every 60 s for ever; the comment at `turns.ts:771-773` says `cannot`. The port keeps the loop.

6.21. Stopping a turn: `endRun(stream, grace)` MUST send SIGTERM to the run's whole group, wait the grace, then SIGKILL the group (`adapter-port.ts:362-376`; `local-exec.ts:88-94,621-625`); the adapters pass `deps.interruptGraceMs ?? INTERRUPT_GRACE_MS`, 5,000 ms. `endAfterResult` first waits 10 s (`RUN_EXIT_MS`) for the agent to exit after EOF. Every ending, a clean exit included, then runs `reapRun(base, lastRead, RUN_STOP_MS = 2000)` (`local-exec.ts:299-333`): if the recorded pid still leads this run's group (the group exists; its leader is dead, or its age from `ps -o etime= -p <pid>` is within 5 s of the claim folder's mtime, or `ps` failed, or the claim folder cannot be read, `:312`), SIGTERM the group and poll every 200 ms for up to 2 s, stopping as soon as the group is gone; then SIGKILL the group whatever the poll saw; read what the log gained; and delete the run's files in this order: `sh log pid agent exit in held late fifo tail pump`, then the `.d` folder. So children an agent left in its group always die. Between turns of a kept process only the leftovers die: group members whose parent is outside the group, TERM then KILL after 2 s (`local-exec.ts:214-226,463-469`).

6.33. A local reader's poll MUST run its steps in this order (`local-exec.ts:563-604`): a killed or failed launch settles `null`; a permission wait touches the activity clock; with the leader known, not resting, a finite idle limit and quiet for at least half of it, start one `ps -eo pid=,ppid=,pgid=,time=` read, not awaited and with no timeout (one per tick once a turn has been quiet 5 minutes); work out the cut, unless resting, wall before idle (`machine-exec.ts:106`), and when cut, caught up and no exit seen, settle `null` with the cut error without reading again; read `.exit` and trim it; read one chunk of the log (at most 262,144 bytes; the offset moves by the bytes read). A non-empty chunk touches the activity clock and feeds the lines, sets caught up when it was shorter than a chunk and notes whether `.exit` was non-empty, then yields with `setImmediate` and loops with no sleep. An empty chunk with a non-empty `.exit` settles with its code; else, caught up, a cut settles `null` with the error; else the dead-leader rule; else sleep 100 ms (1 s at rest). Every line is queued before `exited` resolves, and a cut turn's error comes after its last line.

6.34. Broken run files MUST read as the TS reader reads them (`local-exec.ts:393-399,488-491,537,585,597,757-773`; `machine-exec.ts:268-290`):

| Input | Reading |
|---|---|
| `.exit` empty or blank | still running |
| `.exit` `14` with no newline, or `14abc` | 14 (`parseInt`) |
| `.exit` not a number | `NaN` as the code |
| `.pid` missing, empty, not a number or 0 or below | no leader: a kill sets `killed` but signals nothing, the reap only deletes files, and the group runs on; a host that dies between the spawn and the `.pid` write leaves exactly this |
| the log's last line with no newline at exit | delivered as a line; a trailing newline makes no empty line; blank lines are `""`; `\r` is kept |
| a line of exactly 16 MiB plus its newline | one line; only strictly longer is cut, at 16 MiB moved back up to 3 bytes to a UTF-8 character start, each piece its own line |
| `.in` holding only the EOF marker | kept (its size is above 0), with no messages handed over |
| `.in` empty and a `.held` beside it | the held seed is appended first, so the run is kept |
| `.in` empty and no `.held` (an `input` of `[]` included) | reaped, `gone` |
| a stored `from` past the log's size | reads nothing until the log grows past it; no clamp, no error |

> **After the port.** A non-numeric `.exit` settles with `NaN`, and a run with no readable `.pid` is never signalled, so its group outlives the host's stop. The port keeps both.

6.35. The `.late` files and `.pump` MUST be what the TS reader makes of them (`local-exec.ts:331,355-366,660-672`; `adapter-claude/src/adapter.ts:1279`; `adapter-codex/src/adapter.ts:888`): `writeAfter` parks a line by writing `.late.key` (6 random bytes as hex) and `.late` (the line and `\n`), both 0600; once the `after` promise resolves, the line is appended to `.in` when `.d` exists, `.exit` does not and the key differs from `.late.went`, which is then written with the key, 0600; a rejected `after` deletes both. `reapRun` does not delete `.late.key` or `.late.went` and the sweep finds claims by `.d` alone, so `.late.went` stays in `runs/` after every turn that released a late line, and `.late.key` stays when a host dies between the park and the release. The machine road removes `<base>.*` whole (`machine-exec.ts:338`). `.pump` holds the pids of the pump's two processes, which `pumpOf` leaves out of the leftovers; both are orphans in the run's group, survive a host restart, and die with the group's SIGKILL.

6.36. Signal errors MUST be swallowed where the TS host swallows them: `signalGroup`, `signalPid` and `groupExists` ignore ESRCH and EPERM alike (`local-exec.ts:88-94,228-234,258-265`), and `groupExists` reads EPERM as existing; the slate `killGroup`s ignore every error. Between turns of a kept process, `endLeftovers` signals each leftover by pid, not by group: SIGTERM, then SIGKILL after `RUN_STOP_MS` from an unref'd timer to those still alive (`local-exec.ts:214-226,463-469`).

6.37. A local turn's wall limit MUST default to `TURN_WALL_MS` = 6 h, replaced by the computer's `turnLimitAt` when the wiring has one; `runtime/src/account/channels.ts:87-93` gives `Infinity` for a computer whose row reads no limit (`null`) and nothing when the door lists no row, which leaves the 6 h default. The idle clock restarts at a re-open (`turnActivity(now())`, `local-exec.ts:456`), while the wall counts from the row's `startedAt`.

A cloud machine or box runs the same shape through its exec (`machine-exec.ts:738-830`): `/tmp/wsp-run/<12 hex>`, `oom_score_adj` 500 first, `setsid bash <base>.sh > <base>.log 2>&1 &`, polls every 1.5 s (200 ms when hurried, 10 s at rest), reap by `kill -TERM -- -$P`, ten checks 200 ms apart, `kill -KILL -- -$P`; attach answers `gone` only on the machine's own `WSP_GONE` and never on silence.

6.43. A machine reader MUST keep its own constants beside 6.19's: 200 ms polls for 3 s after a write (`HURRIED_POLL_MS`, `HURRIED_FOR_MS`) and for 30 s from the launch (`OPENING_HURRY_MS`), 20 s per inline exec (`INLINE_EXEC_MS`), and a 60 s dark window (`LINK_RETRY_WINDOW_MS`) before a cut. Its attach reads back at most 262,144 bytes of `.in`, whole lines only, and has no empty-`.in` rule; only the machine's own `WSP_GONE` answers `gone` (`machine-exec.ts:196-211,738-830`).

## The local daemon

6.22. The daemon for this computer MUST start lazily on the first ask and be held; a failed start is forgotten so the next ask tries again, warning once per sentence `the daemon for this computer did not start, so its terminal, files and processes have nothing to dial: <line>` (`cli/wiring.ts:117-129`; `start-once.ts:20-43`). The spawn (`local-daemon.ts:175-257`): `<assets>/daemon/<triple>/wsp-daemon --kind local --host 127.0.0.1 --port 0 --token-path <t> --root <person home> --roots-path <state>/roots --inbox <state>/inbox --work-folder <home>/wsp-work --manifest <own>/manifest.json [--readings-dir <state>/readings] [--docker-notes <state>/docker-notes]`, stdin ignored, not detached, no environment option (it inherits the host's), no cwd. The token is 24 random bytes as 48 hex characters plus a newline, mode 0600, in `mkdtemp(<tmp>/wsp-local-daemon-<host pid>-)`; folders of dead pids (EPERM reads alive) are swept first; the inbox folder is made 0700 before the spawn. The port is read from the line `wsp-daemon listening on <host>:<port>` within 10 s; then one link reads `daemon.hello`'s version within 10 s. A failed start stops the child (SIGTERM, SIGKILL after 5 s) and removes the folder. There is no automatic replacement when it is behind: the copy road and the doctor refuse (`runtime/src/projects/folders.ts:77-81`; `host/src/doctor.ts:1497`), and `workspaces.restartDaemon` restarts it on request (`runtime/src/projects/workspaces.ts:683`). Close is SIGTERM, SIGKILL after 5 s, await, remove the folder.

6.38. The local daemon MUST keep the TS lifecycle's gaps (`local-daemon.ts`; `cli/wiring.ts:114-129`; `start-once.ts:20-43`): nothing watches the child after its start, so a daemon that dies stays held and every dial gets its dead road until `workspaces.restartDaemon`; `close()` closes the samples link, signals only a child that has not exited (SIGTERM, SIGKILL after 5 s, then awaits it), and removes the token folder in every case; a close during a start waits for that start, then closes what it made; a dial after the close began throws `this host is closing; this computer's daemon is not there to dial`. Stderr goes to the host's log by line, and the last 20 chunks give the failure sentence, capped at 300 characters (`SAID_MAX`, `doctor.ts:992`); the warning repeats only when its sentence changes, and a success resets it. After a host SIGKILL the daemon runs on; the next host removes only its token folder, not the process.

> **After the port.** A daemon that died is never restarted by itself, and one a killed host left runs for ever beside the next host's. The port keeps both.

## Spawn helpers

The TS host has one helper per road, and they disagree on what a timeout reports. Each MUST be ported with its own behaviour:

| Helper | Spawn | Bound | On the bound | Exit code reported | Output | Where |
|---|---|---|---|---|---|---|
| `runChild` | detached; `cwd`, `env` as given (replaces); stdin an open pipe unless `stdin` is given; `onLine` | none unless given | SIGKILL the group | 124, forced even when the process exited in the race; unstartable 127 with the error appended to stderr; killed by a signal -1 | two utf8 strings, no cap; the last partial line flushed; pipes drained 200 ms quiet or 1 s after exit | `engine/src/child-exec.ts:63-140` |
| `LocalMachine.exec` | `runChild("bash", ["-c", cmd])`, cwd `wsp-work`; `run` takes `onLine` and passes no stdin | per call (`deadlineMs`) | as `runChild` | as `runChild` | | `engine/src/local-backend.ts:84-86,104-110` |
| `runHere` | `/bin/bash -c`, cwd home, `HOME` set, detached | 60 s (`WRITE_MS`) | SIGKILL the group | the close code, or **1** when null (a timeout reads 1); start error 127 with the error as stderr | no cap; answers on `close`, so a grandchild holding the pipes holds it to the bound | `host/src/target-road.ts:23-48` |
| `spawnRun` | detached, stdin ignored, stderr dropped | 120 s | SIGTERM the group, settle failed after 1 s (`CLOSE_GRACE_MS`); an abort signal SIGKILLs the tree | none: stdout only on exit 0, else nothing | 64 MiB cap (`MAX_OUTPUT`), past which the run fails | `collect/src/live-host.ts:146-252` |
| `systemRunner` | `execFile`, default 1 MiB `maxBuffer` | 15 s (100 s for a leave) | SIGTERM the single child | numeric code, else **127** (a timeout or a full buffer included) | stdout and stderr joined | `host/src/service.ts:411-429` |
| slate runs | `bash -c <cmd> bash <args...>`, detached | `min(max(timeout ?? 60, 1), 600)` s | SIGKILL the group | 124, `timed out after <n> s` | 2 MiB shared, then the last 64 KiB characters kept | `runtime/src/slate-runs.ts:296-359,985-987` |
| `boundedCommand` | shell text on a machine: `set -m; <cmd> & p=$!; ( sleep N; kill -KILL -- -$p ) ...; wait $p; ...; [ $r -ne 137 ]` | N s | SIGKILL the group | non-zero only when the command's status is 137; any other status, a failure included, exits 0 | | `engine/src/machine-context.ts:194-199` (macOS has no `timeout`) |

6.23. A port MUST keep each helper's own timeout code: `runChild` 124, `runHere` 1, `systemRunner` 127, `spawnRun` none. Callers read these codes (`exit 124 = "was stopped after 3 minutes"`, `folders.ts:115-125`). Unifying them is an after-port ticket.

6.24. Who leads a process group (spawned detached, so signalled as a group): every turn, slate run, slate `then`, slate MCP server, `runChild` child, `runHere`, `spawnRun`, `codex app-server` login read, editor program, and the verb-started host. Not detached: the local daemon, the guest tool server, cloudflared, the ssh forward, `wsp mcp`'s binary. Every liveness check `kill(pid, 0)` reads EPERM as alive (`pidAlive`, `host-lock.ts:64-75`; `local-exec.ts:98-102,258-265`; `connector.ts:231-235`; `sqlite-store.ts:209`); the exported `ownPid` (`host-lock.ts:79`), which reads EPERM as another login's process, is used by no check in `packages/host/src` (only the Electron app's `host-lifecycle.ts:129` calls it). So `groupExists` reads true on EPERM and the reap's 2 s wait runs out in full.

## Restart and update

6.25. The host never re-execs itself. `host.restart` (`runtime/src/serve.ts:1899-1914`) refuses a stamped socket (a ticket, a thread's token, a paired device; `ownRoad = stamped === undefined`, `serve.ts:546`) with `kind: "ticket"` and `a socket let in on a ticket cannot restart this host; restart it on the computer it runs on`. A host with no restart road answers `This host cannot restart itself.` and a host started by `wsp up` answers `Ctrl-C the terminal running wsp up and run it again.`, both as failures with no `kind` (tape below). Otherwise it answers `{id, ok: true}` first, then runs the road the lock's `startedBy` picks (`packages/host/src/restart.ts:55-86`): `service` closes and exits 0, or logs `the host did not close cleanly: <message>` and exits 1, for the manager to start it again; `verb` closes (exit 1 with no successor if the close threw), then starts a successor exactly as 6.6 does on the held port and exits 0 once it serves, or logs `the host that was to replace this one did not serve: <message>` and exits 1. A second ask also gets `ok` and is ignored. Only a restart that rejects before closing (`the host is still starting`, `cli/serve.ts:297`) leaves the host serving and logs `host.restart failed: <message>`; once a road has closed the host, a failure exits.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"host.restart"}}
{"out":{"id":2,"ok":false,"error":"Ctrl-C the terminal running wsp up and run it again."}}
```

6.26. Nothing restarts on an update except the Linux program-gone stop (6.13). The release watch (`release.ts:126-236`) only reads GitHub: 30 s after start, then every 6 h, never more often than every 10 minutes, cached in `release.json`, showing `installed` (read fresh from `package.json`) beside `running`.

6.42. The release watch MUST keep its limits (`release.ts:20-23,126-236`): the 10-minute floor counts from the last try and a person's press skips it; About opening also checks; the request sends `If-None-Match` with the kept etag and takes a 304 only when a release is kept; the body is capped at 1 MiB; the request's timeout is 130 s (`RELEASE_TIMEOUT_MS`, three 30 s connects, 30 s and 10 s); `installed` is shown only when it differs from `running`, and is read again at each check, keeping the last value when the read fails.

What a running turn sees across a restart: nothing. It was never signalled, its stdout is its log file, its stdin is the file-fed pump, its group and session are its own; its `WSP_HOST_URL` names the same port (the successor takes the held port, a service names `--port`), and its thread token stays valid, so its tools reconnect after a few seconds of refused connections. Waiting clients saw 4001 `stopping` and wait for the host to come back (`verbs/client.ts:202,399`).

## What has a tape here, and why not more

One tape: `host.restart` on a host `wsp up` started (6.25). The rest of this chapter cannot be a tape, because a tape is the frames of one socket on one fresh host with `PATH=/usr/bin:/bin` and nothing else: a turn needs an agent on that PATH and the checker's host has none; the local daemon needs its binary beside the host, which the checker's host does not have; a lock, a signal, a service unit and a restart happen outside the socket, before it opens or after it closes. Those rules are pinned three ways instead: the measured lines in 6.28 and 6.31, from a throwaway host (`env -i`, its own `--state`, `--port 0`, 2026-10-25); the TS tests a port carries over (`packages/host/test/stop-signals.test.ts`, `packages/runtime/test/local-exec.test.ts`, `packages/host/test/state-store.test.ts`); and the seeded replay of 03.
