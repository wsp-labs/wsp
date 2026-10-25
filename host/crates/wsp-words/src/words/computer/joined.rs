// SPDX-License-Identifier: AGPL-3.0-only
//! The host-key and root-shell checks of a computer being added, and the lines a pause, a wake and a stop on a joined computer end with.

use super::*;

/// Where the ssh client on this computer writes the key a machine first answered with under its own default
/// config, in the words a person would type, since the file is theirs and nothing wsp owns. This is what a screen
/// can name before anything is dialled, which is where a person reads what pressing Add will do; the file that was
/// actually written is the client's own answer, and the step that writes it says which one it was.
// computer.ts:467
pub const KNOWN_HOSTS: &str = "~/.ssh/known_hosts";

/// What the step that wrote a machine's key into this computer's known_hosts says it left there: the fingerprint
/// whole, so it can be checked against the machine's own, and the file the client answered with where that is not
/// the one the plan line named. A person with UserKnownHostsFile pointed elsewhere reads the true path here rather
/// than the default above it.
// computer.ts:473
// export function hostKeyKeptNote(hostKey: string, file?: string): string {
//   return file === undefined || file.endsWith(KNOWN_HOSTS.slice(1)) ? hostKey : `${hostKey} in ${file}`;
// }
pub fn host_key_kept_note(host_key: &str, file: Option<&str>) -> String {
    match file {
        Some(file) if !file.ends_with(&KNOWN_HOSTS[1..]) => format!("{host_key} in {file}"),
        _ => host_key.to_string(),
    }
}

/// The login shells a computer's root may run for wsp to work it. sshd hands every command the host sends to
/// root's own shell with -c before wsp's own `bash -c` inside it, and on a computer wsp joins root's home is the
/// one every workspace on it writes, so a shell that reads a file under that home runs a workspace's file as that
/// computer's root, outside every namespace, on each dial.
///
/// What the list stands on and what it does not: `env -i HOME=<home> <shell> -c true` against a home whose every
/// startup file writes a marker wrote none for bash, sh, dash and ksh and wrote one for zsh. dash and ksh are off
/// the list all the same, since the list is the owner's ruling and not the measurement. And the measurement has a
/// hole this end cannot close: a bash built with SSH_SOURCE_BASHRC reads `~/.bashrc` under -c when the environment
/// carries SSH_CLIENT, which sshd sets for every session, and the bash on the computer this was measured on (3.2.57,
/// Apple's) is such a build. Which build a given box carries is not something the host can read before it dials,
/// and the shell sshd runs takes no flags from this end, so bash stays on the list by the ruling.
// computer.ts:489
// export const PLACE_ROOT_SHELLS: readonly string[] = ["bash", "sh"];
pub const PLACE_ROOT_SHELLS: [&str; 2] = ["bash", "sh"];

// The file a shell reads under the home before running a command, where wsp has read one: the measurement above.
// fish is not on the computer that measurement was made on, and its own documentation names the other. A shell in
// neither list is refused for not having been read, and the sentence says so rather than claiming a file it has
// not seen.
// computer.ts:495
// private in the TS: port it as a private helper
// const SHELL_STARTUP_FILE: Readonly<Record<string, string>> = { zsh: "~/.zshenv", fish: "config.fish" };

/// What a computer is refused with for running a shell that is not on the list: the shell, why it is a road, and
/// the one command that changes it. Said before anything of wsp's lands there.
// computer.ts:499
// export function placeRootShellRefusal(address: string, shell: string): string {
//   const file = SHELL_STARTUP_FILE[shell];
//   const why =
//     file === undefined
//       ? `${shell} is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root`
//       : `${shell} reads ${file} before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root`;
//   return `${address} runs ${shell} as its root's login shell, and sshd hands every command wsp sends to that shell: ${why}; run chsh -s /bin/bash root on it and add it again`;
// }
pub fn place_root_shell_refusal(address: &str, shell: &str) -> String {
    let startup = match shell {
        "zsh" => Some("~/.zshenv"),
        "fish" => Some("config.fish"),
        _ => None,
    };
    let why = match startup {
        None => format!("{shell} is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root"),
        Some(file) => format!("{shell} reads {file} before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root"),
    };
    format!("{address} runs {shell} as its root's login shell, and sshd hands every command wsp sends to that shell: {why}; run chsh -s /bin/bash root on it and add it again")
}

/// Whether the key a dial read is the one the person pinned. A person passes either the whole known_hosts word
/// (`ssh-ed25519 SHA256:...`) or the fingerprint on its own, which is what ssh-keygen prints beside the size and the
/// comment, so both sides are read down to the fingerprint where they carry one.
// computer.ts:511
// export function hostKeyMatches(pinned: string, read: string): boolean {
//   const mark = (word: string): string => word.trim().split(/\s+/).find(w => w.startsWith("SHA256:")) ?? word.trim();
//   return mark(pinned) !== "" && mark(pinned) === mark(read);
// }
pub fn host_key_matches(pinned: &str, read: &str) -> bool {
    let mark = |word: &str| -> String {
        let trimmed = js_trim(word);
        trimmed.split(is_js_whitespace).find(|w| w.starts_with("SHA256:")).unwrap_or(trimmed).to_string()
    };
    let pinned_mark = mark(pinned);
    !pinned_mark.is_empty() && pinned_mark == mark(read)
}

/// What a computer this computer's ssh client has never met is asked before the first dial: the key it answered a
/// scan with, for the person to check against the computer in front of them. What the first dial writes is the
/// identity every later dial of that computer trusts, so it is confirmed before anything is sent there.
// computer.ts:519
// export function hostKeyAsk(address: string, hostKey: string): string {
//   return `${address} answers with the key ${hostKey}; trust it and continue?`;
// }
pub fn host_key_ask(address: &str, host_key: &str) -> String {
    format!("{address} answers with the key {host_key}; trust it and continue?")
}

/// What the add is refused with for such a computer where nobody confirmed that key: the key whole, so it can be
/// read against the computer itself, and the line that pins it. Said before anything is dialled.
// computer.ts:525
// export function hostKeyUnconfirmedRefusal(address: string, hostKey: string): string {
//   return `${address} has never been reached from this computer and answers with the key ${hostKey}; check it against the computer itself, then run wsp add ${address} --host-key '${hostKey}'`;
// }
pub fn host_key_unconfirmed_refusal(address: &str, host_key: &str) -> String {
    format!("{address} has never been reached from this computer and answers with the key {host_key}; check it against the computer itself, then run wsp add {address} --host-key '{host_key}'")
}

/// The same refusal where the key could not be asked for at all: a jump host or a proxy command in the person's own
/// ssh config, which a scan cannot follow, or a computer that answered no scan. The key is theirs to read on the
/// computer and pass here.
// computer.ts:532
// export function hostKeyUnscannableRefusal(address: string, stoppedBy?: string): string {
//   const why = stoppedBy === undefined ? "answered no key to a scan" : `is reached through ${stoppedBy} in your ssh config, which a key scan cannot follow`;
//   return `${address} has never been reached from this computer and ${why}; read its host key on the computer itself and run wsp add ${address} --host-key '<type> <fingerprint>'`;
// }
pub fn host_key_unscannable_refusal(address: &str, stopped_by: Option<&str>) -> String {
    let why = match stopped_by {
        None => "answered no key to a scan".to_string(),
        Some(by) => format!("is reached through {by} in your ssh config, which a key scan cannot follow"),
    };
    format!("{address} has never been reached from this computer and {why}; read its host key on the computer itself and run wsp add {address} --host-key '<type> <fingerprint>'")
}

/// What an install is refused with when the computer that answered the first dial holds a key other than the one
/// the person pinned: what it answered with, the file ssh wrote that into on its way in, and the line that takes it
/// out again. Nothing of wsp's has left this computer at that point.
// computer.ts:540
// export function hostKeyMismatchRefusal(o: { address: string; pinned: string; wrote?: string; target: string; file: string }): string {
//   return `${o.address} answered with ${o.wrote ?? "a key this computer could not read"}, not the ${o.pinned} you pinned; ssh wrote it into ${o.file}, and ssh-keygen -R ${o.target} -f ${o.file} takes it out`;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostKeyMismatch {
    pub address: String,
    pub pinned: String,
    pub wrote: Option<String>,
    pub target: String,
    pub file: String,
}

pub fn host_key_mismatch_refusal(o: &HostKeyMismatch) -> String {
    let wrote = o.wrote.as_deref().unwrap_or("a key this computer could not read");
    format!(
        "{} answered with {wrote}, not the {} you pinned; ssh wrote it into {}, and ssh-keygen -R {} -f {} takes it out",
        o.address, o.pinned, o.file, o.target, o.file
    )
}

/// What a first dial says about the machine it reached: the host key it answered with, for the person to compare
/// against the machine's own before they trust the road. Printed once, when the workspace is recorded.
// computer.ts:546
// export function sshHostKeyNotice(hostKey: string): string {
//   return `its host key is ${hostKey}; compare it with the machine's own before a thread runs there`;
// }
pub fn ssh_host_key_notice(host_key: &str) -> String {
    format!("its host key is {host_key}; compare it with the machine's own before a thread runs there")
}

/// What a verb is refused with when this host wired no module for the kind it names.
// computer.ts:551
// export function noKindLine(kind: string): string {
//   return `this host has no ${kind} backend wired, so it serves nothing of that kind`;
// }
pub fn no_kind_line(kind: &str) -> String {
    format!("this host has no {kind} backend wired, so it serves nothing of that kind")
}

/// What import is refused with on a kind no road lands a folder on, said before the folder is read. Every kind
/// has a road today; the sentence stands for the next kind added without one, which is what the words table's
/// null import road means.
// computer.ts:558
// export function noImportRoadLine(name: string, machine: string): string {
//   return `${name} is ${machine}, which lands no folder yet; import to a fork, or register the folder on this computer`;
// }
pub fn no_import_road_line(name: &str, machine: &str) -> String {
    format!("{name} is {machine}, which lands no folder yet; import to a fork, or register the folder on this computer")
}

/// What a machine without a node the wsp command runs on is refused with. The daemon itself is one static binary
/// and asks nothing of the machine; the wsp command that rides beside it, which every turn's agent drives the host
/// through, still runs on node. A machine wsp builds carries the floor's; a machine somebody already owns may carry
/// none, or one too old. Said before the install rather than after, so nothing lands on a machine that refuses.
// computer.ts:566
pub const NO_NODE_LINE: &str = "this machine has no Node 22, which the wsp command beside the daemon runs on; install Node 22 or newer on it and deploy the daemon again";

/// What a machine whose login does not linger is refused with. Its own systemd stops when its last session ends
/// and takes the daemon with it, so a daemon deployed there is gone the moment the host's connection closes; the
/// person turns linger on once and it holds for every login after.
// computer.ts:572
pub const NO_LINGER_LINE: &str = "this login does not linger, so its services stop when you log out and the daemon would not outlive the connection; run loginctl enable-linger on it and deploy the daemon again";

/// What a machine whose daemon would be held up by systemd, and which has none, is refused with. An unsupervised
/// daemon is a daemon that goes with the first crash and never comes back from a reboot, and nothing on such a
/// machine would put it up again. Said before anything is installed rather than as the install starts a service
/// manager that is not there, so a machine that cannot take a daemon is left exactly as wsp found it.
// computer.ts:579
pub const NO_SYSTEMD_LINE: &str = "this machine runs no systemd, which is what starts the daemon here and brings it back after a reboot; deploy it on a machine that runs systemd";

/// Every sentence a machine's own checks refuse with, in one place beside them. Read by the rule that keeps each
/// one's first clause short enough for a row, so a refusal added later takes that rule without anyone remembering
/// where it is checked. Nothing decides anything by searching this: which ending a throw is comes off its mark.
// computer.ts:585
// export const MACHINE_LACKS_LINES: readonly string[] = [NO_NODE_LINE, NO_LINGER_LINE, NO_SYSTEMD_LINE];
pub const MACHINE_LACKS_LINES: [&str; 3] = [NO_NODE_LINE, NO_LINGER_LINE, NO_SYSTEMD_LINE];

/// The marks the checks a machine takes before a daemon is put on it end with, put on where the throw happens
/// rather than matched against text: a check added to a place's preflight is then one shell line and one sentence,
/// and nothing keeps a second copy of which sentences mean what. Two of them, because the two endings lead
/// opposite ways. A machine that answered and refused has said what it has not got, which a row shows and which
/// stands until a person puts that thing there. A check that never reached the machine has said nothing about it
/// either way: those are the client's own words, they belong in no row, and whatever the record already knew about
/// that machine still holds. Every other failure is a deploy log and carries neither mark.
// computer.ts:594
pub(super) const MACHINE_LACKS: &str = "wspMachineLacks";

// computer.ts:595
pub(super) const MACHINE_UNANSWERED: &str = "wspMachineUnanswered";

// computer.ts:597
// export function machineLacking(said: string): Error {
//   return Object.assign(new Error(said), { [MACHINE_LACKS]: true });
// }
/// A HostError with the message as given and the mark `wspMachineLacks`, nothing else set.
pub fn machine_lacking(said: &str) -> HostError {
    let mut error = HostError::new(said);
    error.marks.push(MACHINE_LACKS.to_string());
    error
}

// computer.ts:601
// export function machineUnanswered(said: string): Error {
//   return Object.assign(new Error(said), { [MACHINE_UNANSWERED]: true });
// }
/// A HostError with the message as given and the mark `wspMachineUnanswered`, nothing else set.
pub fn machine_unanswered(said: &str) -> HostError {
    let mut error = HostError::new(said);
    error.marks.push(MACHINE_UNANSWERED.to_string());
    error
}

/// The sentence a machine refused with, or undefined for every other failure.
// computer.ts:606
// export function machineLacksLine(e: unknown): string | undefined {
//   return marked(e, MACHINE_LACKS) ? e.message : undefined;
// }
pub fn machine_lacks_line(e: &HostError) -> Option<String> {
    marked(e, MACHINE_LACKS).then(|| e.message.clone())
}

/// Whether the check never reached the machine, so nothing about what that machine has was learned.
// computer.ts:611
// export function machineNeverAnswered(e: unknown): boolean {
//   return marked(e, MACHINE_UNANSWERED);
// }
pub fn machine_never_answered(e: &HostError) -> bool {
    marked(e, MACHINE_UNANSWERED)
}

/// A refusal cut to its first clause, which is what the machine has not got. Every sentence above is written in
/// that order, what is wrong, then why it matters, then what to do, and its head is short enough for a row about
/// thirty characters wide; the whole sentence goes where there is room, since the instruction is at the end of it
/// and a row that cut from the right would take the instruction off. A refusal added later is written to the same
/// shape rather than carrying a second, shorter copy of itself.
// computer.ts:620
// export function machineLacksShort(said: string): string {
//   return said.split(",")[0]!.trim();
// }
pub fn machine_lacks_short(said: &str) -> String {
    js_trim(said.split(',').next().unwrap_or("")).to_string()
}

/// The one sentence a socket a machine's requests arrive on is refused a ticket with. A ticket authenticates the
/// next socket, and a socket this host minted no relay ticket for is one of the person's own, so a machine that
/// could mint one would hand itself the origin the relay stamps on it.
// computer.ts:627
pub const RELAY_TICKET_REFUSAL: &str = "a request relayed from a machine cannot mint a ticket into this host";

/// The one sentence a workspace on a machine wsp does not run refuses a verb that machine cannot take with. This
/// computer and a machine reached over ssh are not machines wsp forks, pauses or snapshots, so the verbs that move a
/// fork have no meaning on either; `machine` is the kind's own word for what it is and `action` is the verb as the
/// person typed it. The capability behind each is false, so the road that reads the capability says this.
// computer.ts:633
// export function undrivenRefusal(name: string, machine: string, action: string): string {
//   return `${name} is ${machine}, which wsp does not run; it cannot ${action}`;
// }
pub fn undriven_refusal(name: &str, machine: &str, action: &str) -> String {
    format!("{name} is {machine}, which wsp does not run; it cannot {action}")
}

/// The one sentence a forget on a workspace wsp does not run the machine of is refused with. Such a machine is
/// never gone, so the sentence about a machine still standing at a provider says two impossible things on it: there
/// is no provider to pause it at, and the road that takes the record away is the delete, which asks a provider for
/// nothing either.
// computer.ts:641
// export function forgetUndrivenRefusal(name: string, machine: string): string {
//   return `${name} runs on ${machine}, which is not at a provider; run wsp delete ${name}`;
// }
pub fn forget_undriven_refusal(name: &str, machine: &str) -> String {
    format!("{name} runs on {machine}, which is not at a provider; run wsp delete {name}")
}

/// The one sentence a workspace on a machine wsp does run refuses a verb with when the provider under it has no
/// road for that verb: the machine is wsp's to move, so the refusal names the provider's limit rather than telling a
/// person their fork is their own computer. Each verb reads its own capability, so what is missing is that verb's
/// road and nothing else about the machine. The machine stays the subject the action was written for, since every
/// action is the machine's own verb phrase, and where the road is missing comes after it.
// computer.ts:650
// export function providerCannotRefusal(name: string, machine: string, action: string): string {
//   return `${name} is ${machine}, and it cannot ${action} on the provider it runs on`;
// }
pub fn provider_cannot_refusal(name: &str, machine: &str, action: &str) -> String {
    format!("{name} is {machine}, and it cannot {action} on the provider it runs on")
}

/// The row's line when a pause or a wake ran its deadline out, once and once more after the retry: which move, how
/// long it was given in all, and what the provider reads about the machine after it, or that the provider could not
/// be read. The person's road is to try again; the runtime never leaves the row at Pausing or Waking.
// computer.ts:657
// export function moveTimedOutLine(move: "pause" | "wake", elapsedMs: number, reads: MachineState | undefined): string {
//   const provider = reads === undefined ? "could not be read about the machine" : `reads the machine ${reads}`;
//   return `${move} did not complete in ${fmtDuration(elapsedMs)}; the provider did not answer and ${provider}; try again`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Pause,
    Wake,
}

/// `reads` is a MachineState's wire word.
pub fn move_timed_out_line(r#move: Move, elapsed_ms: f64, reads: Option<&str>) -> String {
    let word = match r#move {
        Move::Pause => "pause",
        Move::Wake => "wake",
    };
    let provider = match reads {
        None => "could not be read about the machine".to_string(),
        Some(state) => format!("reads the machine {state}"),
    };
    format!("{word} did not complete in {}; the provider did not answer and {provider}; try again", fmt_duration(elapsed_ms, None))
}

/// How many asks a window of asking holds at a cadence, which is the count the row counts against and the last ask
/// a wake makes. An ask starting exactly as the window runs out is not made, so a window of one cadence holds one.
/// The window and the cadence are the backend's to declare.
// computer.ts:665
// export const wakeAsksIn = (forMs: number, everyMs: number): number => Math.ceil(forMs / everyMs);
pub fn wake_asks_in(for_ms: f64, every_ms: f64) -> f64 {
    (for_ms / every_ms).ceil()
}

/// The row's line the moment a resume runs its cap out with nothing back. Not a refusal and not a failed resume: the
/// provider's mutating calls hang at the HTTP level while the operation goes through (probed 2026-09-10, a pause that
/// answered nothing in 30 s had the machine paused 40 s later), so the machine's own state is what settles it and the
/// sentence says that is what is happening next.
// computer.ts:671
pub const RESUME_UNANSWERED: &str = "the provider has not answered the resume request; reading the machine";

/// The line for an ask the host is making on its own: which ask this is, of the ones it will make. Two readings of
/// the one fact, since two surfaces show it: the long one is the Machine tab's, which has a line to spend on prose,
/// and the short one is the sidebar row's, whose meta slot holds a couple of words beside the spend and the nap
/// countdown and cuts from the right. The count is passed rather than worked out here so a runtime given a
/// shorter cadence says the number it will keep to.
// computer.ts:678
// export function wakeAskingAgainLine(ask: number, of: number, style: "long" | "short" = "long"): string {
//   return style === "short" ? `asking ${ask}/${of}` : `waking, asking again (${ask} of ${of})`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Long,
    Short,
}

/// `style` defaults to Long.
pub fn wake_asking_again_line(ask: f64, of: f64, style: Option<LineStyle>) -> String {
    match style.unwrap_or(LineStyle::Long) {
        LineStyle::Short => format!("asking {}/{}", number_string(ask), number_string(of)),
        LineStyle::Long => format!("waking, asking again ({} of {})", number_string(ask), number_string(of)),
    }
}

/// The row's line, and what the wake ends with, when the person stopped the host asking from the row.
// computer.ts:683
pub const WAKE_STOPPED: &str = "waking stopped; the machine is still paused";

/// The row's line, and what the wake ends with, when the person stopped the host waiting for a machine the provider
/// runs to take commands: it stays up, and the next wake waits for it again.
// computer.ts:687
pub const WAKE_STOPPED_UP: &str = "waking stopped; the machine is up and has not taken a command yet";

/// What a wake ends with when the provider runs the machine and it still refuses commands at its backend's deadline,
/// as a Boat box does while its disk streams in behind a running reading: the provider's own words, and that a later
/// wake waits for it again.
// computer.ts:692
// export function noCommandsYetLine(name: string, said: string): string {
//   return `${name} is up and takes no commands yet (${said}); wake it again in a minute or two`;
// }
pub fn no_commands_yet_line(name: &str, said: &str) -> String {
    format!("{name} is up and takes no commands yet ({said}); wake it again in a minute or two")
}

/// The row's line once the host's asking ran out and the machine is still paused there, which is the fault probed on
/// 2026-09-10: machines paused for days never resume, while a fresh pause resumes in seconds. Three facts in the order
/// a person needs them: what the provider did, that their work is where they left it, and the one road to a machine
/// now. The disk clause is about the machine as it stands, not about the rebuild, which kills the old machine and
/// lands the nap-time vault instead; the rebuild's own dialog says that.
// computer.ts:701
// export function wakeGaveUpLine(asks: number, overMs: number): string {
//   return `the provider answered none of ${plural(asks, "resume request")} over ${fmtDuration(overMs)}; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image`;
// }
pub fn wake_gave_up_line(asks: f64, over_ms: f64) -> String {
    format!(
        "the provider answered none of {} over {}; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image",
        plural(asks, "resume request"),
        fmt_duration(over_ms, None)
    )
}

/// What a wake ends with when the machine came back and failed every check its backend allows: the faults in order,
/// then that the workspace still stands on that machine, since its disk is the only copy of the agents' sessions and
/// of the work nobody pushed, and no wake trades it for a fresh fork of the image.
// computer.ts:708
// export function wakeFailedLine(machineId: string, faults: string): string {
//   return `the wake of ${machineId} did not finish: ${faults}; the workspace keeps this machine and its disk`;
// }
pub fn wake_failed_line(machine_id: &str, faults: &str) -> String {
    format!("the wake of {machine_id} did not finish: {faults}; the workspace keeps this machine and its disk")
}

/// What the needs-you road says outside the app when a machine came up while the person was looking elsewhere.
// computer.ts:713
// export const workspaceAwakeLine = (name: string): string => `${name} is awake`;
pub fn workspace_awake_line(name: &str) -> String {
    format!("{name} is awake")
}

/// What a stop of a thread on a computer the person joined says when what the thread started there did not all end:
/// the processes still running, by pid as the Processes pane lists them, or none where that computer did not answer.
// computer.ts:717
// export function threadLeftLine(name: string, pids: readonly number[]): string {
//   if (pids.length === 0) return `${name} did not say that what this thread started there has ended; look in Processes and end what still runs there`;
//   const one = pids.length === 1;
//   return `${one ? "a process" : `${pids.length} processes`} this thread started on ${name} still ${one ? "runs" : "run"} after Stop (${pids.join(", ")}); end ${one ? "it" : "them"} in Processes, or restart ${name}`;
// }
pub fn thread_left_line(name: &str, pids: &[i64]) -> String {
    if pids.is_empty() {
        return format!(
            "{name} did not say that what this thread started there has ended; look in Processes and end what still runs there"
        );
    }
    let one = pids.len() == 1;
    let list = pids.iter().map(|pid| pid.to_string()).collect::<Vec<_>>().join(", ");
    let count = if one { "a process".to_string() } else { format!("{} processes", pids.len()) };
    let (verb, object) = if one { ("runs", "it") } else { ("run", "them") };
    format!("{count} this thread started on {name} still {verb} after Stop ({list}); end {object} in Processes, or restart {name}")
}

/// What a stop of a running turn on a computer the person joined says when that computer is away or has not answered:
/// the thread reads stopped here, and the end of what it runs there is owed to that computer's next link.
// computer.ts:725
// export const threadEndAwayLine = (name: string): string => `${name} is not answering, so the turn ends there, with what this thread started, once ${name} connects again`;
pub fn thread_end_away_line(name: &str) -> String {
    format!("{name} is not answering, so the turn ends there, with what this thread started, once {name} connects again")
}

/// The same where that computer's link still stands but it did not answer a ping in time: it may only be slow, and the
/// end runs the moment it answers.
// computer.ts:729
// export const threadEndLateLine = (name: string, seconds: number): string => `${name} did not answer within ${seconds} s, so the turn ends there, with what this thread started, once ${name} answers`;
pub fn thread_end_late_line(name: &str, seconds: f64) -> String {
    format!(
        "{name} did not answer within {} s, so the turn ends there, with what this thread started, once {name} answers",
        number_string(seconds)
    )
}

/// What a send says while it waits on the end a stop owed a computer that was away, in the app and on the command line.
// computer.ts:732
// export const sendWaitsForLine = (name: string): string => `waiting for ${name} to connect; this message runs once the stopped turn has ended there`;
pub fn send_waits_for_line(name: &str) -> String {
    format!("waiting for {name} to connect; this message runs once the stopped turn has ended there")
}

/// What a send that waited on such an end says once its turn started.
// computer.ts:735
// export const sendWaitedForLine = (name: string): string => `${name} connected and the stopped turn ended there; this turn started`;
pub fn send_waited_for_line(name: &str) -> String {
    format!("{name} connected and the stopped turn ended there; this turn started")
}

/// What a Stop or a delete of a thread says of a message it gave up while that message waited on such an end, and what
/// the send itself is answered.
// computer.ts:739
// export const sendGivenUpLine = (name: string): string => `the message waiting for ${name} to connect was given up and never ran`;
pub fn send_given_up_line(name: &str) -> String {
    format!("the message waiting for {name} to connect was given up and never ran")
}

/// What a delete of a thread on a computer the person joined says when what the thread started there did not all end:
/// the delete goes on, and that computer's next link ends it.
// computer.ts:743
// export const threadEndOwedLine = (name: string): string => `what this thread started on ${name} did not all end; wsp ends it when ${name} next connects`;
pub fn thread_end_owed_line(name: &str) -> String {
    format!("what this thread started on {name} did not all end; wsp ends it when {name} next connects")
}
