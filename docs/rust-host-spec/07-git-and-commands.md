# 07 Git, gh and the other programs the host runs

The behaviour nobody wrote down lives here: the exact command lines, which exit codes mean what, which output is parsed and how, and which environment each child gets. Every `file:line` is at `a3b870a6e`. Rows marked (daemon) are argv the Rust daemon builds today; the host only sends an op and reads the JSON reply, so a port of the host needs the op and the reply, and the one binary (#2450) carries the daemon's argv as they are.

## Two roads for git

Most git is the daemon's. The host runs git itself only for reads on this computer, a few fetches, and the `copy` verb of the daemon binary as a child.

**The host's own road.** `gitHere(cwd, args, timeoutMs = 20_000, stdin?)` (`packages/runtime/src/machines/kinds.ts:96-99`) builds `git -C <cwd> <args...>` as text with `shellLine` and runs it on the local machine as `bash -c <text>`. `shellLine` leaves a word bare when it matches `^[A-Za-z0-9_@%+=:,./-]+$` and single-quotes it otherwise, `'` becoming `'\''` (`packages/protocol/src/shell-quote.ts:4-25`). The local machine (`packages/engine/src/local-backend.ts:84-106`) runs `runChild("bash", ["-c", cmd])` with cwd `<person home>/wsp-work` (`packages/host/src/cli/wiring.ts:64`) and the host's own `process.env` object, by reference, so the login shell's PATH that `adoptLoginPath` wrote into it reaches every git (06, rule 6.15). `gitHere` never throws on a non-zero exit: it answers `{exitCode, stdout, stderr}`.

**The daemon's road.** 33 `git.*` ops (`packages/protocol/src/wire/daemon.ts:684-870`: status, diff, discard, commit, snapshot, range, turn, push, pr, prRead, prView, runLog, prMerge, issueRead, prCheckout, prDiff, prReview, prReply, prResolve, prReact, repoRead, update, startOn, branchCompare, mergeIn, prList, checkpoint, restore, checkpointDrop, worktrees, branches, switchNew, fetchBranch), each carrying `cwd`, sent through `withDaemon(entry, work)` (`packages/runtime/src/account/channels.ts:158-168`): up the computer's link where the workspace's computer serves its daemon, else one socket opened with the workspace's daemon token, used and closed. `onThisComputer(work)` (`channels.ts:247-251`) sends the frame to this computer's daemon with `cwd` the person's home, so it runs the person's own signed-in `gh`. The daemon runs every git under `GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0 LC_ALL=C` and reads exit 0 and 1 as answers (`daemon/crates/wsp-daemon/src/git.rs:88,108-116`).

7.1. Host-run git MUST be built as text with `shellLine` and run by `bash -c` through the machine's exec, as today, not as an argv array: the quoting is part of the behaviour (a word with `^{commit}`, `*` or `~` is quoted, `--` and `-C` are not), and on a box or a joined computer the same text goes over ssh or the daemon's `exec` op.

7.2. `GIT_TERMINAL_PROMPT=0` MUST be set exactly where the TS host sets it, as a text prefix before `git`: in `branchGoneAtRemote` (`packages/runtime/src/projects/folders.ts:355-357`), `gitAtLead` (`machines/sub-spawn.ts:356-359`), the branch-of-record heads (`projects/branch-of-record.ts:144`), the places setup probe (`places/setup.ts:639-640`), and the clone on this computer as `env GIT_TERMINAL_PROMPT=0 git clone` (`kinds.ts:157`). It is NOT set on `gitHere` fetches (`projects/start-from.ts:161,174`; `folders.ts:423`). A fetch that prompts is a behaviour the port keeps and 14 lists as an after-port ticket.

7.3. Host git writes on one workspace MUST run one at a time through `queued(id, work)` (`folders.ts:204-212`), and a failed one MUST NOT block the next: commit, discard, restore, merge in, update, bring back and worktree removal.

## Every git the host runs itself

| Purpose | Command (after `git -C <path>` unless shown) | Road | Read | Exit codes | Bound | Where |
|---|---|---|---|---|---|---|
| repo top | `rev-parse --show-toplevel` | `gitHere` | `stdout.trim()`; empty or non-zero is none | | 20 s | `kinds.ts:102-106` |
| current branch | `symbolic-ref --quiet --short HEAD` | `gitHere` | `stdout.trim()` | non-zero or empty is detached | 20 s | `kinds.ts:108-112` |
| origin and default branch, one command | `git -C <p> remote get-url origin \|\| echo; git -C <p> symbolic-ref --short refs/remotes/origin/HEAD \|\| echo` | local exec | line 0 the remote, line 1 the head with `^origin/` cut; each side `""` when it failed | the shell's exit is not read | 20 s | `kinds.ts:83-92` |
| origin for a sync rule | `spawnSync("git", ["-C", p, "remote", "get-url", "origin"])`, no shell | the host process | `status === 0 ? stdout.trim() : ""` | | 2 s | `account/channels.ts:910-917` |
| a branch's upstream | `config --get branch.<b>.remote`, `config --get branch.<b>.merge` | `gitHere` | each `stdout.trim()`; empty, or remote `.`, is no upstream | not read | 20 s | `folders.ts:351-354` |
| is the branch gone at the remote | `GIT_TERMINAL_PROMPT=0 git -C <worktree> ls-remote --exit-code --heads <remote> <merge ref>` | local exec | exit code only | **2 alone means gone**; 0, 1, 128, 124 all mean kept | 20 s | `folders.ts:355-357` |
| does the remote hold a ref | `ls-remote --exit-code <remote> <ref>` | `gitHere` | exit code only | anything but 2 means it holds, a network failure included | 300 s | `start-from.ts:165-166` |
| changed files | `status --porcelain` | `gitHere` | count of non-blank lines; no column is parsed | non-zero throws git's own text in the refusal paths | 20 s | `folders.ts:235,324,393`; `start-from.ts:180` |
| worktrees | `worktree list --porcelain` (no `-z`) | `gitHere` | split on `\n`; `worktree <path>` starts an item; `branch refs/heads/<b>` sets the branch of the last item | non-zero answers `[]` | 20 s | `folders.ts:459-466` |
| checkpoint still there | `rev-parse --verify -q <ref>^{commit}` | `gitHere` | exit code | non-zero refuses with `FORK_NO_CHECKPOINT_LINE`, kind `conflict` | 20 s | `threads/forks.ts:209` |
| drop a branch after a refused fork start | `branch -D <branch>` | `gitHere` | errors swallowed | | 20 s | `forks.ts:219-223` |
| pull request head into a branch | `fetch --quiet --no-tags <remote> pull/<n>/head:refs/heads/<branch>` | `gitHere` | error text via `gitSaid` | | 300 s | `start-from.ts:161-162` |
| catch up | `fetch --quiet --no-tags <remote> refs/heads/<b>` (or `pull/<n>/head`), then `rev-parse FETCH_HEAD`, `rev-parse HEAD`, `merge-base --is-ancestor` both ways | `gitHere` | equal is nothing to do; worktree an ancestor is fast-forward; neither is diverged | 0 and 1 read as yes and no | 300 s, 20 s | `start-from.ts:170-179` |
| sub-thread refs | `GIT_TERMINAL_PROMPT=0 git -C <lead> ls-remote --heads origin`; `ls-remote origin <n> <n>^{}`; `rev-parse --verify --quiet <n>^{commit}`; `for-each-ref --contains <sha> --format=%(refname:lstrip=3) refs/remotes/origin` | `gitAtLead`, on the lead's machine or here | `<sha>\t<ref>` rows; match order `name`, `refs/heads/name`, `refs/tags/name^{}`, `refs/tags/name`; a 7 to 40 hex name is a commit | non-zero refuses with `subFromUnreadRefusal` | 20 s each | `machines/sub-spawn.ts:349-390`; `branch-of-record.ts:124-146` |
| a recipe's folder remote | `execFile("git", ["-C", p, "remote", "get-url", "origin"])`, `rev-list --count HEAD --not --remotes` | the host process | `stdout.trim()`; error is undefined | | 5 s | `packages/host/src/recipe-options.ts:144-159` |
| anonymous read test | `git -c credential.helper= ls-remote --quiet <url> HEAD` with `GIT_TERMINAL_PROMPT=0 GIT_ASKPASS= SSH_ASKPASS= GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1` added | the host process | `error === null` is open | | 5 s | `recipe-options.ts:149-152` |
| tracked files for a bundle | `ls-files -z` | `execFile`, maxBuffer 256 MiB | NUL split | error throws `git ls-files failed in <source>: ...` | none | `host/src/project-bundle.ts:95-103` |
| unpushed commits as a patch | `format-patch --stdout <base>..HEAD` | `runChild` | whole stdout | non-zero throws `git format-patch over N commits: ...` | 60 s | `host/src/project-seed.ts:68-76` |
| old copy rescue | `fetch --quiet --no-tags <from> +refs/heads/*:refs/rescue-incoming/<n>/heads/* +HEAD:...`, `rev-list --glob=...`, `for-each-ref`, `update-ref --stdin` | `gitHere` | | | 300 s | `folders.ts:399-447` (dropped with `moveOldCopies`, 02) |

The host runs no `git merge-tree`, no `stash` write, no `gc` and no `worktree move` (the one `merge-tree` in the repo is the developer script `scripts/pre-review.sh:260-286`).

7.4. `branchGoneAtRemote(path, branch)` (`folders.ts:351-358`) MUST answer true only when the branch has both `branch.<b>.remote` (not `.`) and `branch.<b>.merge` set AND `ls-remote --exit-code --heads` exits exactly 2. A branch never pushed has no `merge` config and never reaches the `ls-remote`, which is how "never pushed" and "deleted at the remote" (both exit 2) are told apart (trap #1607).

7.5. Every place the TS host reads an exit code as a yes or no (the table's Exit codes column) MUST read it the same way in Rust, including the cases where a failure reads as yes (`ls-remote` in `start-from.ts:165`) or is not read at all.

## The worktree lifecycle

The host makes and removes worktrees through the daemon binary's `copy` verb, run as a child (`packages/engine/src/copier.ts`):

| Verb | Arguments after the binary | Bound | Where |
|---|---|---|---|
| make | `copy worktree --from <repo top> --home <state folder> --project <project id> --branch <branch> [--module <json>]... [--checkpoint <ref>]` | 600 s | `copier.ts:113-114` |
| mount overlays again | `copy worktree-mount --from <top> --home <state folder> --path <worktree>` | 30 s | `copier.ts:117` |
| forget a project's frozen copies | `copy worktree-forget --from <top> --home <state folder> --project <id>` | 600 s | `copier.ts:121` |
| remove | `copy worktree-remove --from <top> --home <state folder> --path <path> [--force]` | 600 s | `copier.ts:125-126` |

Each `--module` is `JSON.stringify` of `{id, lockfiles, carry, never, installed?}` for every ecosystem the catalog knows (`folders.ts:17`).

7.6. The `copy` verb's answer MUST be read as the TS host reads it (`copier.ts:63-106`): exit 0 with one JSON line on stdout (`WorktreeReport {path, branch, made, carried, plain?, overlaid?, fresh, modules[], ms}` or `{path, rescued?}`, held by its zod schema; anything else is `the <what> verb answered something this host does not read: <first 200 chars>`); exit 1 is a refusal whose sentence is the last line of stderr, else of stdout; any other exit (2 usage, 124 timeout, 127 not startable, -1 a signal) is `the daemon's copy verb failed (exit N); the host log has its output` (`protocol/src/words/units.ts:216`) with the real output logged. The host refuses to run the verb when this computer's daemon is older than `DAEMON_VERSION` (`folders.ts:77-81`).

What the daemon decides when it makes a worktree (daemon, `daemon/crates/wsp-runtime/src/copy_road/branch.rs:66-188,449-513`): the branch name must pass `check-ref-format refs/heads/<b>` and not start with `-`; the path is `<root>/<project id>/<branch with every character outside [A-Za-z0-9._-] as ->`, `-2`, `-3` up to 999 on a taken name, folder made 0700; `<root>` is `<state folder>/worktrees` when the state folder and the repo share a volume, else `<repo volume's mount point>/.wsp/worktrees`; an existing branch is checked out as it stands, a missing one is made at the repo top's `HEAD` (for a fork, at the checkpoint commit's parent); a branch already checked out in some worktree answers that worktree with `fresh: false`.

The host's side, in order (`folders.ts:72-264`):

7.7. `folderFor` (`folders.ts:154-200`) MUST refuse, before anything runs: a branch and a folder together (`BRANCH_OR_CWD_LINE`, kind `usage`); a branch on a joined computer (`placeBranchLine`); a project with no repo top (`noBranchesLine`, kind `usage`). A branch that is the one checked out in the asking thread's own folder answers that folder, no worktree.

7.8. `worktreeFolder` (`folders.ts:72-90`) MUST run one at a time per `<project id>\0<branch>` (`ctx.oneFolder`), MUST answer the project folder when the report's path is the repo top, and otherwise MUST record the worktree through `worktreeRecordAt` and, when the report says `fresh`, run the setup commands (7.11).

7.9. Every worktree record MUST be made by `worktreeRecordAt(project, tree, parent?)` (`folders.ts:140-149`; trap #1607): an existing record of the same path has its worktree rewritten to exactly `{path, made: was.made || tree.made, branch?, madeFor?: was.madeFor}`, which clears `settled`, `kept`, `removeFailed` and `gone` and keeps the OLD `madeFor`; a new one is `recordFolder` (`folders.ts:28-62`: id `ws_<8 hex>`, name `takenNameAfter(project.name + "@" + branch)`, then `writeDaemonRoots`, then the sweep armed if `made`, then `workspace.created`). A thread's own tree admits a worktree record whose `madeFor` is its root thread (`account/rules.ts:65-73`), so a record made any other way locks its thread out.

7.10. A worktree record with `made: false` (one the person made with plain `git worktree add`, `folders.ts:187-195`) MUST never be removed by wsp. Removal refuses it with `wsp made no worktree for <branch>; git worktree remove takes one wsp did not make` (kind `invalid`, `workspaces.ts:832-846`).

7.11. A fresh worktree's setup MUST run on the local machine as `cd <folder> && <command>`, each module's `rebuild` line and then the project's `afterWorktree`, with 180 s each and an empty stdin so nothing waits on a prompt; exit 124 reads "was stopped after 3 minutes"; each start and failure is a `host.notice` event; the thread starts anyway, and its first line carries `worktreeSetupLine` (`folders.ts:91-126`).

7.12. Removal (`folders.ts:216-264`), under `queued`, MUST refuse in this order: no `made` record or no repo top (`notMadeWorktreeLine`); no copier (`NO_COPIER_HERE`); a turn running there and not ending (`a thread is working in that worktree; let its turn end or stop it first`, kind `conflict`); uncommitted files and not forced (`that worktree has N file(s) not committed; ...`, kind `conflict`, from `git status --porcelain`). `check: true` stops after the refusals. Then `copy worktree-remove`, then `worktreeGone`: a record no thread names is dropped; one a thread names keeps `{path, made, branch?, madeFor?, settled, gone: true}` and its threads run on in the project folder. `--force` from a thread's own token refuses with `WORKTREE_FORCE_LINE` (kind `usage`).

7.13. The sweep (`folders.ts:274-358`) MUST run every 10 minutes only while some worktree has `made && !gone`, one worktree a pass. A worktree is removed only when it is settled (its pull request merged or closed, read with `readPullRequest(entry, true)`, or its branch gone at the remote by 7.4), settled at least 6 hours ago, no turn runs there, and `git status --porcelain` shows nothing; otherwise the record says why it is kept (`a thread is running`, `N file(s) not committed`, the removal's last line), and after 7 days `kept 7 days; remove it by hand` and the sweep leaves it alone.

7.14. `writeDaemonRoots(entry, strict)` (`machines/daemon.ts:88-107`) MUST write the whole roots file from every live workspace on the machine (each one's `project.path`, its checkout and `record.landed[]`, de-duplicated, in record order), one write per machine at a time, as the shell `next='<path>.next.'$$; mkdir -p '<dir>'; printf '%s\n' '<root>'... > "$next"; mv -f "$next" '<path>'` (`daemon-roots.ts:10-17`). A daemon op whose `cwd` is outside the person's home and not in the file is refused, and worktrees live under the state folder (trap #1607, #1645, #1774). Failures warn unless `strict` (a project add).

## Merge, merge in, bring back

7.15. `wsp merge` (`projects/workspaces.ts:1033-1058`) MUST use the pull request fact the host holds, however old, and read fresh only when it never read one: a fresh read would hand the head guard whatever an agent pushed since the person looked (runtime invariant 4). It refuses a thread's own token (`spawnGuard`), reads the repo's merge settings through this computer's daemon (`git.repoRead`, held 1 hour per remote), refuses a method the repo does not allow (`the repository does not allow a <m> merge; it allows <list>`), then sends `git.prMerge {cwd: <person home>, remote, number, method, auto, headOid: head ?? fact.headOid}` through `onThisComputer`. Only the app sends `head`. The daemon runs `gh pr merge <n> -R <owner/repo> --merge|--squash|--rebase [--auto] --match-head-commit <headOid>` (daemon, `hosts/github.rs:1158-1165`).

7.16. `wsp merge in` (`workspaces.ts:1067-1105`) MUST refuse, in order: a thread merging into a workspace it does not run on; a child that is not the lead's; a lead with a turn running in any thread but the asking one (`<lead> has a turn running in thread <ids>; merge once it ends, or from inside that turn`, trap #1476); a blocked copy; a child on no branch. With no remote host on the project both sides must be local and the merge reads the child's folder. Then `git.mergeIn {cwd: checkoutOf(lead), branch, from?}` under `queued(lead)`. The conflict check is the daemon's "merge, read the unmerged paths, abort" (daemon, `git/write.rs:340-393`: `merge --no-edit --no-ff`, on failure `rev-parse -q --verify MERGE_HEAD`, `diff --name-only --diff-filter=U -z`, `merge --abort`), so the lead's checkout is left as it was. Nothing is pushed.

7.17. Bring back (`workspaces.ts:745-800`) MUST push before it opens a pull request and MUST never undo the push: `git.push {cwd, base?}`, then for a lead (not a child) `git.pr`; a `no-host-cli` refusal of the pull request is a note beside the push, any other refusal is answered beside it. The daemon's push never uses `--force`, refuses the base branch, the default branch when a base was named, and zero commits ahead (daemon, `bring_back.rs:148-165`).

## Checkpoints and rewind

The host runs no git for checkpoints. It sends (`threads/threads.ts:963-1142`; `threads/sessions.ts:1264-1396`; `folders.ts:242-248`):

| Op | When | Frame | Read |
|---|---|---|---|
| `git.checkpoint` | each turn's end, in a repo | `{cwd, thread, turn, scope: record.id}` | `ref`, `based` into `session.checkpoint` |
| `git.snapshot` | at launch and at the end | `{cwd}` | `commit`, raced against 3,000 ms; late or failed means no changes line, and the turn runs |
| `git.turn` | after the end snapshot | `{cwd, from, to}` | the files, into `session.changes` |
| `git.restore` | rewind with files, and its undo | `{cwd, checkpoint, scope}` | `files`, `before` |
| `git.checkpointDrop` | thread or workspace delete | `{cwd, scope, thread}` | nothing; a failure warns |

7.18. A folder that is not a git repo MUST take no checkpoint and no snapshot (`threads.ts:957,966,1068`). Rewind MUST refuse before anything moves, in the order `sessions.ts:1264-1396` checks (a running turn, running children, another thread's turn in the folder when files go back, no earlier rewind for an undo, the latest turn, a copied thread, no checkpoint), and MUST restore the files again from the `before` ref if the harness then refuses to cut the conversation. The daemon keeps the newest 100 checkpoint refs per thread under `refs/wsp/checkpoints/<scope>/<thread>/<turn>` (daemon, `git/checkpoint.rs:14-62`).

## Pull request reads and their caches

`readPullRequest(entry, force, whole)` (`account/pull-requests.ts:99-152`):

7.19. An unforced read MUST answer the held fact while it is younger than 3 minutes with a window open, 15 minutes with none (`PR_POLL_MS`, `PR_POLL_IDLE_MS`, `protocol/src/pull-request.ts:118-121`), or 10 seconds when what is held is not a full fact (`CHECKOUT_TTL_MS`); a kept record that is not open is never read again; one read is in flight at a time and a non-whole caller joins it (trap #1466, #1718: a verb acting on a check forces it).

7.20. The other holds MUST keep their values: the page cache 60 s, dropped by every write (`PR_PAGE_HOLD_MS`, `pull-requests.ts:61-70`); a `rate-limited` refusal answered for 60 s on that road (`:37-47`); repo merge settings 1 hour; an open fact re-read on a timer at the poll period, or 30 s when a check is pending and a slate watches checks (`:224-241`).

7.21. Anything posted as the person (a reply, a resolve, a reaction) MUST drop the page cache first and go through this computer's daemon only, never a copy's (`postAsPerson`, `pull-requests.ts:74-82`; runtime invariant 4); with no signed-in command line it refuses with `no signed-in command line for <host> is on this computer, so nothing is posted as you`.

## gh run by the host itself

| Purpose | Command | Bound | Read | Where |
|---|---|---|---|---|
| which account the broker uses | `gh auth status --active --hostname github.com --json hosts` | 15 s, 1 MiB | `.hosts["github.com"][0]`, `state === "success"` and a `login` | `host/src/broker/credentials.ts:36-82` |
| the broker's token | `gh auth token --hostname github.com` | 15 s | `stdout.trim()`; held 5 min, one read in flight, a missing login not held | `credentials.ts:85-100` |
| a repo's id | `gh api repos/<owner/name> --jq .id` | 15 s | a positive safe integer | `host/src/server-broker.ts:100-115` |
| branch of record | `GH_PROMPT_DISABLED=1 gh api ... --jq '[...] \| @tsv'` | 20 s | the trailing newline cut, split on tab; ids positive integers, shas 40 hex | `projects/branch-of-record.ts:24-142` |
| signed in here | `gh auth token --hostname github.com > /dev/null 2>&1` | 20 s | exit 0 | `machines/sub-spawn.ts:93-101` |
| clone through gh | `env GIT_TERMINAL_PROMPT=0 gh repo clone <owner/repo> <dest>` (glab alike), after `command -v gh` and `gh auth setup-git` on a copy | 300 s | | `kinds.ts:157-158`; `catalog/src/git-hosts.ts:24-43`; `project-landing.ts:137-157` |

Every other `gh` is the daemon's (daemon, `hosts/github.rs`), always with `-R <owner/repo>` from the project's recorded remote, never the copy's own config (`pull-requests.ts:15-19`); exit 4 means authentication required and "rate limit" in stderr means rate limited (`github.rs:952-954,1350-1361`).

## Other programs

| Program | Command | Bound | Notes | Where |
|---|---|---|---|---|
| `ps` | `ps -eo pid=,ppid=,pgid=,time=`; `ps -axo pid=,ppid=,pgid=`; `ps -o etime= -p <pid>` | none | a turn's CPU, a group's members, a leader's age `[[dd-]hh:]mm:ss` | `runtime/src/local-exec.ts:171,190,281` |
| `lsof` | `lsof -nP -iTCP:<port> -sTCP:LISTEN -Fpc` | 3 s | `^p(\d+)$` and `^c(.+)$`; any error is none | `host/src/ports.ts:26-35` |
| `ssh` | `[-n] -T -o ControlMaster=auto -o ControlPath=<~/.wsp/ssh/wsp-ssh-<16 hex>.sock> -o ControlPersist=60 -o BatchMode=yes -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 -p <port> [-i <key> -o IdentitiesOnly=yes] <user>@<host> bash -c <wrapper> [wsp <script>]`; as root the wrapper `exec sudo -n bash -c` (or `sudo -S -k -p ''` with the password as stdin's first line); exit 255 is ssh's own refusal | per call | no scp and no rsync: files go as `cat > <tmp>` on stdin, then `mv -f` after a byte-count check | `engine/src/ssh-backend.ts:106-375,579-593,777-789` |
| `ssh` forward | `ssh -F /dev/null ... -o ExitOnForwardFailure=yes -o ServerAliveInterval=15 -o ServerAliveCountMax=3 -o ControlPath=none -R 127.0.0.1:<box>:127.0.0.1:<door> user@host 'echo WSP_BACK_UP; exec cat'`, SIGTERM on release | | carried options from two `ssh -G` reads | `ssh-backend.ts:182-359` |
| `ssh-keygen`, `ssh-keyscan` | `ssh-keygen -q -t ed25519 -N "" -C "wsp on <hostname>" -f <home>/ssh/id_ed25519`; `ssh-keygen -F <target> -f <known_hosts>`; `ssh-keyscan -T 5 -p <port> <host>` | 5 s, 10 s | | `host/src/ssh-files.ts:52-57`; `ssh-backend.ts:470,544` |
| `tar` | `tar --no-xattrs [--no-mac-metadata] -czf <tgz> -C <stage> .` with `COPYFILE_DISABLE=1`; `tar -xzf <archive> -C <dir>` | none | | `host/src/doctor.ts:948-964`; `host/src/project-export.ts:41-51` |
| `cloudflared` | `tunnel --no-autoupdate run` with `TUNNEL_TOKEN`, or `tunnel --no-autoupdate --url http://127.0.0.1:<port>`; env only `PATH`, `HOME`, `TUNNEL_TOKEN`; restart 2 s doubling to 30 s | none | sha256-pinned download (`host/assets/cloudflared.json`) checked before it is made executable | `host/src/connector.ts:34-200` |
| `open`, `xdg-open` | `<opener> <url>` | none | darwin `open`, else `xdg-open` | `host/src/relay.ts:23-36` |
| editors | VS Code family `bin/code [-g <path>:<line>]`, Zed `cli --classic`, JetBrains `/usr/bin/open -na <App> --args`, Finder `/usr/bin/open [-R]`; Mac only | 10 s, then left running | env less `ELECTRON_RUN_AS_NODE`, `XPC_SERVICE_NAME`, `WSP_*` | `host/src/editor.ts:54-284` |
| `launchctl`, `systemctl` | 06 | 15 s; stops 100 s | | `host/src/service.ts:186-429` |
| `security` | `/usr/bin/security find-generic-password -a <user> -w -s "Claude Code-credentials[-<8 hex>]"`; Linux reads `.credentials.json` | 10 s | darwin only | `host/src/anthropic/login.ts:56-70` |
| `claude` | `claude mcp get wsp-login-refresh` to renew a login under 5 minutes left | 60 s | env less `ANTHROPIC_*`, `CLAUDE_CODE_OAUTH_*`, `CLAUDE_CODE_USE_*` | `anthropic/login.ts:11-135` |
| the login shell | `$SHELL -ilc 'env -0'` | 15 s | 06, rule 6.15 | `host/src/login-path.ts:37-107` |
| the daemon binary | `wsp-daemon --kind local --host 127.0.0.1 --port 0 --token-path <t> --root <home> --roots-path <p> --inbox <d> --work-folder <wsp-work> --manifest <m> [...]`; `wsp-daemon unsaved <folder>`; `<bin> mcp --state <s> --scoped --guest [--no-slate]`; `<bin> mcp --help` | 10 s start, 120 s, none, 5 s | 06 | `host/src/local-daemon.ts:175-257`; `places/join.ts:486-497`; `guest-tools.ts:26-37`; `mcp-install.ts:105-112` |
| the agents' command lines | `claude -p ... --input-format stream-json --output-format stream-json ...`, `codex app-server -c ...`, `cursor-agent -p ...`, `opencode run ...`, each wrapped by `agentWord` so the run learns the agent's pid | the turn's limits | 06 | `protocol/src/adapter-port.ts:29-38`; `adapter-claude/src/landmines.ts:379-420`; `adapter-codex/src/command.ts:326-331` |

7.22. A port MUST keep each command line in this chapter's tables word for word, with its bound, its environment and its reading of the output. A row the port changes is an accepted difference with a reason, never a quiet improvement.

## Environment for children

7.23. Local children MUST get the host's own environment, unchanged, as today: `LocalBackend`'s env is `process.env` itself (`wiring.ts:111-113`; `local-backend.ts:195`), so provider keys, `GH_TOKEN` and `ANTHROPIC_API_KEY` in the host's environment reach every local `git`, `gh` and `bash -c`. `WSP_*` is stripped only for slate runs (`slate-runs.ts:647`) and the editor (`editor.ts:152`). That this leaks the host's keys to every local child is listed in 14 as an after-port security ticket, not changed in the port.

7.24. A turn on this computer MUST get the host's environment with `HOME` set to the person's home, `PATH` led by `<state folder>/thread-bin`, `ZDOTDIR=<thread-bin>` and `BASH_ENV=<thread-bin>/<file>` (`wiring.ts:140-145`; `shim.ts:22-25`), then the adapter's own on top (`threads/agents.ts:632-655`; Claude's `buildEnv` strips inherited `CLAUDE_CODE_*`, `CLAUDECODE`, `FORCE_CODE_TERMINAL` and `ANTHROPIC_API_KEY` and sets its headless overrides, `adapter-claude/src/landmines.ts:12-157`). An agent's setup may not set `PATH HOME SHELL USER LOGNAME IFS TMPDIR NODE_OPTIONS NODE_PATH PYTHONPATH PYTHONSTARTUP PYTHONHOME RUBYOPT RUBYLIB PERL5OPT PERL5LIB BASH_ENV ENV ZDOTDIR PROMPT_COMMAND GIT_SSH GIT_SSH_COMMAND GIT_EXEC_PATH` or any `LD_*` or `DYLD_*` (`protocol/src/thread-defaults.ts:388-389`).

7.25. A secret MUST reach an owned computer only on a run's stdin, as NUL-ended `NAME=value` entries read by `while IFS= read -r -d '' wsp_kv; do export "$wsp_kv"; done; unset wsp_kv` (`protocol/src/shell-quote.ts:16`; `engine/src/run-env.ts:11-27`), never in argv or a file (trap #1482).

## Platform branches

| Where | macOS | Linux | Site |
|---|---|---|---|
| adopt the login shell's PATH | when PATH is exactly launchd's four folders | when started by the service or from an AppImage | `login-path.ts:32,51-57` |
| service manager | launchd | systemd, `--user` for the host | `service.ts:362-369` |
| memory reading | `sysctl -n kern.memorystatus_level` | `/proc/meminfo` | `local-backend.ts:63-69` |
| Claude's login | Keychain | `.credentials.json` | `anthropic/login.ts:64-70` |
| door listeners | wildcard and loopback twin | wildcard only | `server.ts:540-543` |
| stop when the program file is gone | no | every 5 s | `cli/serve.ts:131-156` (06) |
| config folder | `~/Library/Application Support/wsp` | `~/.config/wsp` | `runtime/src/host-id.ts:10-15` |
| editors, computer name and model | the table above, `scutil`, `ioreg`, `sysctl` | none | `editor.ts:284`; `places/this-computer.ts:58-93` |
| daemon binary | `aarch64-apple-darwin`, `x86_64-apple-darwin` | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` | `engine/src/daemon-targets.ts:24-27` |

7.26. Every macOS branch MUST be ported even though no box can run it, and each MUST carry a test that runs on macOS; the cutover's Mac run of the tapes (the map's order, step 4) is where they are proven. A Linux-only port of a branch is a review blocker.
