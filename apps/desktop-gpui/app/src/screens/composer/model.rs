// SPDX-License-Identifier: AGPL-3.0-only
//! What the composer knows with no window and no host: the agents and their choices as `harnesses.list` gives them,
//! the picks, which layout the box takes, the menu a token before the caret opens, the files, the queue, the stash,
//! and what holds a send. The web app's rules, read off ChatComposer.tsx, composer-logic.ts and the composer stores.

use std::ops::Range;

use serde::Deserialize;

/// The most files one send carries, and the most bytes each (packages/protocol/src/attachments.ts).
pub const FILES_MAX: usize = 5;
pub const FILE_MAX_BYTES: usize = 10 * 1024 * 1024;
pub const IMAGE_TYPES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];
/// The stash keeps this many drafts and drops the oldest past it (promptStashStore.ts).
pub const STASH_MAX: usize = 20;

/// One choice an agent offers: a model, an effort, a context window or an access mode.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub value: String,
    pub label: String,
    #[serde(default)]
    pub short: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub is_default: Option<bool>,
    /// A model's own efforts and context windows, by value, where it narrows the agent's.
    #[serde(default)]
    pub efforts: Option<Vec<String>>,
    #[serde(default)]
    pub context_windows: Option<Vec<String>>,
    #[serde(default)]
    pub default_effort: Option<String>,
    /// A model that offers Fast mode.
    #[serde(default)]
    pub fast: Option<bool>,
}

impl Choice {
    /// The word a trigger shows: the short form where the agent gives one.
    pub fn word(&self) -> &str {
        self.short.as_deref().unwrap_or(&self.label)
    }
}

/// One agent as `harnesses.list` describes it; only the fields the composer reads.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Agent {
    pub harness: String,
    pub label: String,
    pub models: Vec<Choice>,
    pub legacy_models: Vec<Choice>,
    pub efforts: Vec<Choice>,
    pub context_windows: Vec<Choice>,
    pub permission_modes: Vec<Choice>,
    pub steers: bool,
    pub steers_images: bool,
    pub images: bool,
    pub asides: bool,
    pub is_default: Option<bool>,
    pub refusal: Option<String>,
}

impl Agent {
    /// An agent read off the host's catalog entry; one that does not read is left out by the caller.
    pub fn read(catalog: &impl serde::Serialize) -> Option<Agent> {
        serde_json::to_value(catalog).ok().and_then(|v| serde_json::from_value(v).ok())
    }
}

fn default_of(list: &[Choice]) -> Option<&Choice> {
    list.iter().find(|c| c.is_default == Some(true)).or_else(|| list.first())
}

/// What the person picked in this composer; a `None` falls back to the thread's own value, then the agent's default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picks {
    pub agent: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub window: Option<String>,
    pub access: Option<String>,
    /// Fast mode: the person's pick, else whether the thread's newest turn ran fast.
    pub fast: Option<bool>,
}

/// The choices in force, resolved against the catalog: what each trigger shows and what a send carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Resolved {
    pub agent: Option<Agent>,
    pub model: Option<Choice>,
    pub effort: Option<Choice>,
    pub window: Option<Choice>,
    pub access: Option<Choice>,
    /// The efforts and context windows the model offers, which the reasoning menu lists.
    pub efforts: Vec<Choice>,
    pub windows: Vec<Choice>,
}

/// Resolves `picks` over the thread's own values (`ran`) and the catalog's defaults.
pub fn resolve(agents: &[Agent], picks: &Picks, ran: &Picks) -> Resolved {
    let agent = picks
        .agent
        .as_ref()
        .or(ran.agent.as_ref())
        .and_then(|h| agents.iter().find(|a| &a.harness == h))
        .or_else(|| agents.iter().find(|a| a.is_default == Some(true)))
        .or_else(|| agents.first())
        .cloned();
    let Some(a) = &agent else { return Resolved::default() };
    let pick = |list: &[Choice], picked: &Option<String>, ran: &Option<String>| {
        picked.as_ref().or(ran.as_ref()).and_then(|v| list.iter().find(|c| &c.value == v)).or_else(|| default_of(list)).cloned()
    };
    let models: Vec<Choice> = a.models.iter().chain(&a.legacy_models).cloned().collect();
    let model = pick(&models, &picks.model, &ran.model);
    let narrow = |all: &[Choice], own: Option<&Vec<String>>| -> Vec<Choice> {
        match own {
            Some(own) => all.iter().filter(|c| own.contains(&c.value)).cloned().collect(),
            None => all.to_vec(),
        }
    };
    let efforts = narrow(&a.efforts, model.as_ref().and_then(|m| m.efforts.as_ref()));
    let windows = match &model {
        Some(m) => narrow(&a.context_windows, Some(m.context_windows.as_ref().unwrap_or(&Vec::new()))),
        None => a.context_windows.clone(),
    };
    let model_effort = model.as_ref().and_then(|m| m.default_effort.clone());
    let effort = pick(&efforts, &picks.effort, &ran.effort.clone().or(model_effort));
    Resolved {
        model,
        effort,
        window: pick(&windows, &picks.window, &ran.window),
        access: pick(&a.permission_modes, &picks.access, &ran.access),
        agent,
        efforts,
        windows,
    }
}

impl Resolved {
    /// The reasoning trigger's words: the effort's and the window's short forms, else "Reasoning".
    pub fn reasoning_word(&self) -> String {
        let words: Vec<&str> = [&self.effort, &self.window].into_iter().flatten().map(Choice::word).collect();
        let words = if self.windows.len() > 1 { words } else { words.into_iter().take(1).collect() };
        if words.is_empty() {
            "Reasoning".to_owned()
        } else {
            words.join(" ")
        }
    }

    pub fn model_word(&self) -> String {
        self.model.as_ref().map(|m| m.label.clone()).or_else(|| self.agent.as_ref().map(|a| a.label.clone())).unwrap_or_default()
    }

    pub fn access_word(&self) -> String {
        self.access.as_ref().map(|a| a.word().to_owned()).unwrap_or_else(|| "Access".to_owned())
    }
}

/// The box's three arrangements (ChatComposer.tsx): `Full` on a thread with no message yet; `Line` while the draft
/// fits one line beside the pickers; `Tall` once it needs a second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Full,
    Line,
    Tall,
}

/// `compact` is a thread with a message or a running turn; `lines` how many lines the draft takes at the one-line
/// text width.
pub fn layout(compact: bool, text: &str, lines: usize) -> Layout {
    match (compact, text.contains('\n') || lines > 1) {
        (false, _) => Layout::Full,
        (true, false) => Layout::Line,
        (true, true) => Layout::Tall,
    }
}

/// The menu a token before the caret opens (composer-logic.ts `detectComposerTrigger`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// `/` and a word, only at the very start of the prompt.
    Slash(String),
    /// `@` and a path.
    Mention(String),
    /// `#` and a number or word: the pull requests and issues of the folder's repository.
    Hash(String),
    /// `$` and a name: the skills alone.
    Dollar(String),
}

/// The trigger the text before `cursor` (a byte offset) ends in, and the byte range of its token.
pub fn trigger(text: &str, cursor: usize) -> Option<(Trigger, Range<usize>)> {
    let before = text.get(..cursor)?;
    let start = before.rfind(char::is_whitespace).map(|i| i + before[i..].chars().next().map_or(1, char::len_utf8)).unwrap_or(0);
    let token = &before[start..];
    let range = start..cursor;
    let mut chars = token.chars();
    let word = chars.as_str().get(1..).unwrap_or("").to_owned();
    match chars.next()? {
        '/' => (start == 0).then_some((Trigger::Slash(word), range)),
        '@' => Some((Trigger::Mention(word), range)),
        '#' if word.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') => Some((Trigger::Hash(word), range)),
        '$' => Some((Trigger::Dollar(word), range)),
        _ => None,
    }
}

/// What a menu row is, which says what a pick puts in the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// Words: a command, or a skill the agent announced as one.
    Command,
    /// A `$name` chip.
    Skill,
    /// An `@path` chip.
    File,
    /// A pull request or an issue, as a chip carrying its quoted text.
    Reference { pull: bool },
}

/// One row of the command menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// The group head it sits under.
    pub group: String,
    /// What the menu shows: `/name`, `$name`, a file's basename, `#12 Title`.
    pub label: String,
    /// The muted words after it: a command's description, a file's folder.
    pub note: Option<String>,
    /// What a pick puts in the text: `/name ` for a command, the chip's text for the rest.
    pub insert: String,
    pub kind: ItemKind,
}

/// A slash command the agent announced, or wsp's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub description: Option<String>,
    /// The group the menu lists it under: a plugin's prefix, or `wsp` for wsp's own.
    pub source: Option<String>,
}

/// A skill `agents.read` reports for the thread's agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
}

/// An open pull request or issue of the folder's repository, as the daemon's `git.prList` lists it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct HostItem {
    pub kind: String,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub url: String,
}

impl HostItem {
    pub fn pull(&self) -> bool {
        self.kind == "pull-request"
    }

    /// The text a pick sends (composer-editor-mentions.ts `hostItemText`): its kind, number, title and address, then
    /// its body quoted.
    pub fn text(&self) -> String {
        let noun = if self.pull() { "Pull request" } else { "Issue" };
        let title = self.title.split_whitespace().collect::<Vec<_>>().join(" ");
        let body = self.body.trim();
        let body = if body.is_empty() { "(no description)" } else { body };
        let quoted =
            body.replace("\r\n", "\n").lines().map(|l| if l.is_empty() { ">".to_owned() } else { format!("> {l}") }).collect::<Vec<_>>();
        format!("{noun} #{} \"{title}\" ({}):\n{}", self.number, self.url, quoted.join("\n"))
    }
}

/// The commands the menu offers: the announced ones. wsp's own `/btw` asks a side question in a pane of its own, which
/// this app does not have yet, so it is not offered.
pub fn commands(announced: &[Command]) -> Vec<Command> {
    announced.iter().filter(|c| c.name != "btw").cloned().collect()
}

/// What holds a send of a slash draft (ChatComposer.tsx `slashHold`): a lone slash, or a command nothing announced
/// with nothing after it.
pub fn slash_hold(text: &str, commands: &[Command]) -> Option<String> {
    let text = text.trim();
    let rest = text.strip_prefix('/')?;
    if rest.is_empty() {
        return Some("a slash on its own is not a command".to_owned());
    }
    let (name, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    (after.trim().is_empty() && !commands.iter().any(|c| c.name == name)).then(|| format!("no command here is called /{name}"))
}

/// What the menus list from: the commands, the skills, the folder's files and the repository's items.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sources<'a> {
    pub commands: &'a [Command],
    pub skills: &'a [Skill],
    pub files: &'a [String],
    pub items: &'a [HostItem],
}

/// The rows a trigger lists, best first, at most `cap` (composerMenuItems.ts): the slash menu's commands by source
/// with the skills under Skills, the `$` menu's skills alone, the `@` menu's files, the `#` menu's pull requests and
/// issues.
pub fn menu_items(trigger: &Trigger, from: Sources, cap: usize) -> Vec<MenuItem> {
    let skill_rows = |query: &str, announced: &[Command]| -> Vec<MenuItem> {
        from.skills
            .iter()
            .filter(|s| s.name.to_lowercase().starts_with(query))
            .map(|s| {
                let as_command = announced.iter().any(|c| c.name == s.name);
                MenuItem {
                    group: "Skills".to_owned(),
                    label: format!("{}{}", if as_command { "/" } else { "$" }, s.name),
                    note: s.description.clone(),
                    insert: if as_command { format!("/{} ", s.name) } else { format!("${}", s.name) },
                    kind: if as_command { ItemKind::Command } else { ItemKind::Skill },
                }
            })
            .collect()
    };
    let rows: Vec<MenuItem> = match trigger {
        Trigger::Slash(word) => {
            let word = word.to_lowercase();
            let skills: Vec<&str> = from.skills.iter().map(|s| s.name.as_str()).collect();
            let commands =
                from.commands.iter().filter(|c| !skills.contains(&c.name.as_str()) && c.name.to_lowercase().starts_with(&word)).map(|c| {
                    MenuItem {
                        group: c.source.clone().unwrap_or_else(|| "Commands".to_owned()),
                        label: format!("/{}", c.name),
                        note: c.description.clone(),
                        insert: format!("/{} ", c.name),
                        kind: ItemKind::Command,
                    }
                });
            commands.chain(skill_rows(&word, from.commands)).collect()
        }
        Trigger::Dollar(word) => skill_rows(&word.to_lowercase(), &[]),
        Trigger::Mention(query) => {
            let query = query.to_lowercase();
            let mut hits: Vec<(usize, &String)> = from
                .files
                .iter()
                .filter_map(|path| {
                    let lower = path.to_lowercase();
                    let name = lower.rsplit('/').next().unwrap_or(&lower);
                    if name.starts_with(&query) {
                        Some((0, path))
                    } else if name.contains(&query) {
                        Some((1, path))
                    } else if lower.contains(&query) {
                        Some((2, path))
                    } else {
                        None
                    }
                })
                .collect();
            hits.sort_by_key(|(rank, _)| *rank);
            hits.into_iter()
                .map(|(_, path)| {
                    let (folder, name) = match path.trim_end_matches('/').rsplit_once('/') {
                        Some((folder, name)) => (Some(folder.to_owned()), name.to_owned()),
                        None => (None, path.clone()),
                    };
                    MenuItem { group: "Files".to_owned(), label: name, note: folder, insert: format!("@{path}"), kind: ItemKind::File }
                })
                .collect()
        }
        Trigger::Hash(query) => {
            let query = query.to_lowercase();
            let hit =
                |i: &&HostItem| query.is_empty() || i.number.to_string().starts_with(&query) || i.title.to_lowercase().contains(&query);
            let row = |i: &HostItem| MenuItem {
                group: if i.pull() { "Pull requests" } else { "Issues" }.to_owned(),
                label: format!("#{} {}", i.number, i.title),
                note: None,
                insert: i.text(),
                kind: ItemKind::Reference { pull: i.pull() },
            };
            let pulls = from.items.iter().filter(|i| i.pull()).filter(hit).map(row);
            let issues = from.items.iter().filter(|i| !i.pull()).filter(hit).map(row);
            pulls.chain(issues).collect()
        }
    };
    rows.into_iter().take(cap).collect()
}

/// One sent prompt the box can recall, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recall {
    pub id: String,
    pub prompt: String,
}

/// The thread's prompts for Up and Down (composerPromptHistory.ts): consecutive repeats collapse into the newest and
/// a send with no words is skipped.
pub fn recalls(prompts: impl IntoIterator<Item = (String, String)>) -> Vec<Recall> {
    let mut out: Vec<Recall> = Vec::new();
    for (id, prompt) in prompts {
        let prompt = prompt.trim().to_owned();
        if prompt.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.prompt == prompt => *last = Recall { id, prompt },
            _ => out.push(Recall { id, prompt }),
        }
    }
    out
}

/// Where a recall stands: the entry's id and the text put in the box, which once edited ends browsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recalled {
    pub id: String,
    pub text: String,
}

/// One step of Up (`back`) or Down, or none when the key belongs to the caret: back starts only from an empty box or an
/// unedited recall and stops at the oldest; forward past the newest empties the box and ends browsing.
pub fn recall_step(entries: &[Recall], at: Option<&Recalled>, current: &str, back: bool) -> Option<(Option<Recalled>, String)> {
    let active = at
        .filter(|a| a.text == current)
        .and_then(|a| entries.iter().position(|e| e.id == a.id).or_else(|| entries.iter().rposition(|e| e.prompt == a.text)));
    let to = |e: &Recall| Some((Some(Recalled { id: e.id.clone(), text: e.prompt.clone() }), e.prompt.clone()));
    if back {
        if active.is_none() && !current.is_empty() {
            return None;
        }
        let ix = match active {
            Some(0) => return None,
            Some(ix) => ix - 1,
            None => entries.len().checked_sub(1)?,
        };
        return to(&entries[ix]);
    }
    let ix = active?;
    match entries.get(ix + 1) {
        Some(e) => to(e),
        None => Some((None, String::new())),
    }
}

/// pastedText.ts: a paste this long goes as a file, not into the text.
pub const PASTE_FILE_LINES: usize = 200;
pub const PASTE_FILE_BYTES: usize = 32 * 1024;

/// A long paste as the file it becomes, named after the first free `pasted-text-N.txt`, else none.
pub fn pasted_file(text: &str, taken: &[&str]) -> Option<File> {
    let breaks = text.matches('\n').count();
    let lines = if text.ends_with('\n') { breaks } else { breaks + 1 };
    if lines < PASTE_FILE_LINES && text.len() < PASTE_FILE_BYTES {
        return None;
    }
    let name = (1..).map(|n| format!("pasted-text-{n}.txt")).find(|n| !taken.contains(&n.as_str()))?;
    Some(File { name, media_type: "text/plain".to_owned(), bytes: std::sync::Arc::new(text.as_bytes().to_vec()) })
}

/// What waits around the composer besides its queue (ComposerDrawer.tsx `DrawerRows`), each none while it has
/// nothing to say.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Drawer {
    /// The question the turn waits on.
    pub question: Option<String>,
    /// The usage limit's title and line.
    pub limit: Option<(String, String)>,
    /// What holds the turn back on its computer.
    pub slot: Option<String>,
    /// The step the turn is on, and how many of how many are done.
    pub tasks: Option<(String, String)>,
}

/// A file waiting to go with the next send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct File {
    pub name: String,
    pub media_type: String,
    pub bytes: std::sync::Arc<Vec<u8>>,
}

impl File {
    pub fn is_image(&self) -> bool {
        IMAGE_TYPES.contains(&self.media_type.as_str())
    }

    pub fn size_word(&self) -> String {
        size_word(self.bytes.len())
    }
}

/// The media type a file name says, as the web app reads it off the picked file.
pub fn media_type(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("txt" | "md" | "log") => "text/plain",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// Why a file of `len` bytes is not taken, in the words of attachments.ts (`filesRefusal`, `noImagesLine`), else
/// none. `no_images` names the agent when it reads no images.
pub fn refuse(name: &str, media_type: &str, len: usize, held: usize, no_images: Option<&str>) -> Option<String> {
    let image = IMAGE_TYPES.contains(&media_type);
    if held >= FILES_MAX {
        return Some(format!("only {FILES_MAX} files fit one message; this one carries {}", held + 1));
    }
    if len == 0 {
        return Some(format!("{name} is empty"));
    }
    if len > FILE_MAX_BYTES {
        let kind = if image { "an image" } else { "a file" };
        return Some(format!("{name} is {}, over the 10 MB {kind} may be", size_word(len)));
    }
    match no_images {
        Some(agent) if image => {
            Some(format!("{agent} takes no image with a message; describe it in words, or open the thread on an agent that reads images"))
        }
        _ => None,
    }
}

/// `12 KB`, as a file tile's second line reads.
pub fn size_word(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{} KB", n.div_ceil(1024))
    } else {
        format!("{:.1} MB", n as f64 / (1024. * 1024.))
    }
}

/// A message typed while a turn runs, waiting for it to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    /// Which row a steer's answer is about, as the queue moves under it.
    pub id: u64,
    pub prompt: String,
    pub files: Vec<File>,
}

/// The drawer row's words for the queue: "1 message waiting", "N messages waiting", or "Sending now" while a steer is
/// out.
pub fn queue_word(waiting: usize, steering: bool) -> String {
    match (steering, waiting) {
        (true, _) => "Sending now".to_owned(),
        (false, 1) => "1 message waiting".to_owned(),
        (false, n) => format!("{n} messages waiting"),
    }
}

/// The first line of a message that is not blank, as the drawer and the stash show it.
pub fn first_line(prompt: &str) -> &str {
    prompt.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

/// A draft put aside with Cmd or Ctrl and S.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stashed {
    pub prompt: String,
    pub files: Vec<File>,
    pub at: std::time::SystemTime,
}

/// How long ago, as timestampFormat.ts's `formatRelativeTimeLabel` writes it.
pub fn ago(elapsed: std::time::Duration) -> String {
    let minutes = elapsed.as_secs() / 60;
    match (minutes, minutes / 60) {
        (0, _) => "just now".to_owned(),
        (m, 0) => format!("{m}m ago"),
        (_, h) if h < 24 => format!("{h}h ago"),
        (_, h) => format!("{}d ago", h / 24),
    }
}

/// The stash, newest first and bounded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stash {
    pub entries: Vec<Stashed>,
}

impl Stash {
    pub fn push(&mut self, entry: Stashed) {
        self.entries.insert(0, entry);
        self.entries.truncate(STASH_MAX);
    }

    pub fn take(&mut self, ix: usize) -> Option<Stashed> {
        (ix < self.entries.len()).then(|| self.entries.remove(ix))
    }

    /// The row's words: the first 90 characters with whitespace folded, else what the files are.
    pub fn snippet(entry: &Stashed) -> String {
        let folded = entry.prompt.split_whitespace().collect::<Vec<_>>().join(" ");
        if folded.is_empty() {
            return match entry.files.len() {
                0 => "(empty)".to_owned(),
                1 => "(1 file)".to_owned(),
                n => format!("({n} files)"),
            };
        }
        if folded.chars().count() > 90 {
            format!("{}…", folded.chars().take(90).collect::<String>())
        } else {
            folded
        }
    }
}

/// What holds a send, in the order the web app checks (ChatComposer.tsx `composerSendBlock`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hold {
    Connecting,
    HostDown,
    NotFound,
    Loading,
    NoAgents,
    /// A send is out and its turn has not started yet.
    InFlight,
}

impl Hold {
    pub fn words(&self) -> &'static str {
        match self {
            Hold::Connecting => "Connecting to wsp",
            Hold::HostDown => "wsp is not running",
            Hold::NotFound => "Task not found",
            Hold::Loading => "Loading transcript",
            Hold::NoAgents => "the agents here have not answered yet",
            Hold::InFlight => "Turn in flight",
        }
    }

    /// A hold that also shuts the editor; a turn in flight leaves it open.
    pub fn shuts(&self) -> bool {
        !matches!(self, Hold::InFlight)
    }
}

/// What Enter does with the draft (ChatComposer.tsx `submit`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Submit {
    /// Nothing to send, or the send is held.
    Nothing,
    /// A new turn.
    Start,
    /// Into the running turn, where the agent steers and can carry the files.
    Steer,
    /// Onto the queue until the turn ends.
    Queue,
}

pub fn submit(empty: bool, hold: Option<&Hold>, running: bool, steer_preferred: bool, agent: Option<&Agent>, files: &[File]) -> Submit {
    if empty || hold.is_some_and(|h| *h != Hold::InFlight) {
        return Submit::Nothing;
    }
    if !running && hold.is_none() {
        return Submit::Start;
    }
    let steers = agent.is_some_and(|a| a.steers && (files.iter().all(|f| f.is_image() && a.steers_images) || files.is_empty()));
    if running && steer_preferred && steers {
        Submit::Steer
    } else {
        Submit::Queue
    }
}

/// The placeholder: the slash hint only where the agent announced a command and the box is wide enough.
pub fn placeholder(has_commands: bool, editor_width: f32) -> &'static str {
    if has_commands && editor_width >= 240. {
        "Ask anything, or / for commands"
    } else {
        "Ask anything"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(value: &str, default: bool) -> Choice {
        Choice { value: value.into(), label: value.to_uppercase(), is_default: default.then_some(true), ..Default::default() }
    }

    fn agents() -> Vec<Agent> {
        vec![
            Agent { harness: "codex".into(), label: "Codex".into(), models: vec![choice("gpt", true)], ..Default::default() },
            Agent {
                harness: "claude".into(),
                label: "Claude".into(),
                models: vec![choice("opus", false), choice("sonnet", true)],
                efforts: vec![Choice { short: Some("High".into()), ..choice("high", true) }, choice("low", false)],
                permission_modes: vec![choice("default", true), choice("plan", false)],
                is_default: Some(true),
                steers: true,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn a_pick_beats_the_threads_value_which_beats_the_default() {
        let agents = agents();
        let none = Picks::default();
        let narrowed = Agent {
            harness: "n".into(),
            models: vec![Choice { efforts: Some(vec!["low".into()]), default_effort: Some("low".into()), ..choice("m", true) }],
            efforts: vec![choice("high", true), choice("low", false)],
            context_windows: vec![choice("1m", true)],
            ..Default::default()
        };
        let r = resolve(&[narrowed], &none, &none);
        assert_eq!(r.efforts.len(), 1, "the model narrows the efforts");
        assert!(r.windows.is_empty(), "a model that names no window offers none");
        assert_eq!(r.effort.unwrap().value, "low", "the model's own default effort");
        let r = resolve(&agents, &none, &none);
        assert_eq!(r.agent.as_ref().unwrap().harness, "claude", "the catalog's default agent");
        assert_eq!(r.model.as_ref().unwrap().value, "sonnet");
        assert_eq!(r.reasoning_word(), "High");
        let ran = Picks { model: Some("opus".into()), ..Default::default() };
        assert_eq!(resolve(&agents, &none, &ran).model.unwrap().value, "opus");
        let picked = Picks { model: Some("sonnet".into()), ..Default::default() };
        assert_eq!(resolve(&agents, &picked, &ran).model.unwrap().value, "sonnet");
        let other = Picks { agent: Some("codex".into()), ..Default::default() };
        let r = resolve(&agents, &other, &none);
        assert_eq!(r.model_word(), "GPT");
        assert_eq!(r.reasoning_word(), "Reasoning");
        assert_eq!(r.access_word(), "Access");
        assert_eq!(resolve(&[], &none, &none), Resolved::default());
    }

    #[test]
    fn the_layout_follows_the_thread_and_the_draft() {
        assert_eq!(layout(false, "a\nb", 2), Layout::Full);
        assert_eq!(layout(true, "one line", 1), Layout::Line);
        assert_eq!(layout(true, "a\nb", 1), Layout::Tall);
        assert_eq!(layout(true, "long", 2), Layout::Tall);
    }

    #[test]
    fn a_slash_opens_only_at_the_start_and_an_at_anywhere() {
        assert_eq!(trigger("/rev", 4), Some((Trigger::Slash("rev".into()), 0..4)));
        assert_eq!(trigger("say /rev", 8), None);
        assert_eq!(trigger("look at @src/ma", 15), Some((Trigger::Mention("src/ma".into()), 8..15)));
        assert_eq!(trigger("look at @src/ma now", 19), None);
        assert_eq!(trigger("@", 1), Some((Trigger::Mention(String::new()), 0..1)));
        assert_eq!(trigger("é @x", 5), Some((Trigger::Mention("x".into()), 3..5)));
        assert_eq!(trigger("plain", 5), None);
    }

    #[test]
    fn the_menu_lists_commands_by_prefix_and_files_best_first() {
        let commands = vec![
            Command { name: "review".into(), description: Some("Review".into()), source: None },
            Command { name: "compact".into(), description: None, source: None },
        ];
        let rows = menu_items(&Trigger::Slash("re".into()), Sources { commands: &commands, ..Default::default() }, 10);
        assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["/review"]);
        assert_eq!(rows[0].insert, "/review ");
        assert_eq!(rows[0].group, "Commands");
        let files: Vec<String> = ["src/lib/main.rs", "main.rs", "docs/domain.md", "src/remain.ts"].map(String::from).to_vec();
        let from = Sources { files: &files, ..Default::default() };
        let rows = menu_items(&Trigger::Mention("main".into()), from, 10);
        assert_eq!(
            rows.iter().map(|r| r.insert.as_str()).collect::<Vec<_>>(),
            ["@src/lib/main.rs", "@main.rs", "@docs/domain.md", "@src/remain.ts"]
        );
        assert_eq!(rows[0].label, "main.rs");
        assert_eq!(rows[0].note.as_deref(), Some("src/lib"));
        assert_eq!(menu_items(&Trigger::Mention(String::new()), from, 2).len(), 2);
    }

    #[test]
    fn skills_list_under_slash_and_alone_under_dollar() {
        let skills = vec![Skill { name: "tidy-imports".into(), description: Some("Sort imports".into()) }];
        let announced = vec![Command { name: "review".into(), description: None, source: None }];
        let from = Sources { commands: &announced, skills: &skills, ..Default::default() };
        let rows = menu_items(&Trigger::Slash(String::new()), from, 10);
        assert_eq!(
            rows.iter().map(|r| (r.group.as_str(), r.label.as_str())).collect::<Vec<_>>(),
            [("Commands", "/review"), ("Skills", "$tidy-imports")]
        );
        assert_eq!(rows[1].kind, ItemKind::Skill);
        assert_eq!(rows[1].insert, "$tidy-imports");
        let rows = menu_items(&Trigger::Dollar("ti".into()), from, 10);
        assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["$tidy-imports"]);
        assert!(menu_items(&Trigger::Dollar("x".into()), from, 10).is_empty());
        assert_eq!(trigger("say $ti", 7), Some((Trigger::Dollar("ti".into()), 4..7)));
    }

    #[test]
    fn the_hash_menu_lists_pull_requests_then_issues_and_inserts_the_quoted_item() {
        let items = vec![
            HostItem {
                kind: "issue".into(),
                number: 7,
                title: "Jumpy  scroll".into(),
                body: "It jumps.\nOften.".into(),
                url: "https://x/7".into(),
            },
            HostItem {
                kind: "pull-request".into(),
                number: 12,
                title: "Fix scroll".into(),
                body: String::new(),
                url: "https://x/12".into(),
            },
        ];
        let from = Sources { items: &items, ..Default::default() };
        let rows = menu_items(&Trigger::Hash(String::new()), from, 10);
        assert_eq!(
            rows.iter().map(|r| (r.group.as_str(), r.label.as_str())).collect::<Vec<_>>(),
            [("Pull requests", "#12 Fix scroll"), ("Issues", "#7 Jumpy  scroll")]
        );
        assert_eq!(rows[1].insert, "Issue #7 \"Jumpy scroll\" (https://x/7):\n> It jumps.\n> Often.");
        assert_eq!(rows[0].insert, "Pull request #12 \"Fix scroll\" (https://x/12):\n> (no description)");
        assert_eq!(menu_items(&Trigger::Hash("7".into()), from, 10).len(), 1);
        assert_eq!(trigger("see #1", 6), Some((Trigger::Hash("1".into()), 4..6)));
        assert_eq!(trigger("see #a.b", 8), None);
    }

    #[test]
    fn up_walks_back_through_sent_prompts_and_down_forward() {
        let entries = recalls([("a", "first"), ("b", " second "), ("c", "second"), ("d", "")].map(|(i, p)| (i.to_owned(), p.to_owned())));
        assert_eq!(entries.iter().map(|e| e.prompt.as_str()).collect::<Vec<_>>(), ["first", "second"]);
        assert_eq!(entries[1].id, "c", "a repeat collapses into the newest");
        let (at, text) = recall_step(&entries, None, "", true).unwrap();
        assert_eq!(text, "second");
        let (at2, text) = recall_step(&entries, at.as_ref(), "second", true).unwrap();
        assert_eq!(text, "first");
        assert_eq!(recall_step(&entries, at2.as_ref(), "first", true), None, "the oldest stops the walk");
        assert_eq!(recall_step(&entries, None, "a draft", true), None, "a draft keeps Up for its caret");
        assert_eq!(recall_step(&entries, at2.as_ref(), "first, edited", true), None, "an edited recall ends browsing");
        let (_, text) = recall_step(&entries, at2.as_ref(), "first", false).unwrap();
        assert_eq!(text, "second");
        assert_eq!(recall_step(&entries, at.as_ref(), "second", false), Some((None, String::new())), "past the newest empties the box");
        assert_eq!(recall_step(&entries, None, "", false), None);
    }

    #[test]
    fn a_long_paste_becomes_the_first_free_text_file() {
        assert!(pasted_file("short", &[]).is_none());
        let long = "line\n".repeat(PASTE_FILE_LINES);
        assert_eq!(pasted_file(&long, &[]).unwrap().name, "pasted-text-1.txt");
        assert_eq!(pasted_file(&long, &["pasted-text-1.txt"]).unwrap().name, "pasted-text-2.txt");
        assert!(pasted_file(&"x".repeat(PASTE_FILE_BYTES), &[]).is_some());
        assert!(pasted_file(&"x\n".repeat(PASTE_FILE_LINES - 1), &[]).is_none(), "a closing newline starts no line");
    }

    fn file(name: &str, len: usize) -> File {
        File { name: name.into(), media_type: media_type(name).into(), bytes: std::sync::Arc::new(vec![0; len]) }
    }

    #[test]
    fn files_past_the_caps_are_refused() {
        let check = |f: File, held: usize, no_images: Option<&str>| refuse(&f.name, &f.media_type, f.bytes.len(), held, no_images);
        assert_eq!(check(file("a.png", 10), 0, None), None);
        assert_eq!(check(file("a.png", 10), FILES_MAX, None).as_deref(), Some("only 5 files fit one message; this one carries 6"));
        assert_eq!(check(file("a.txt", FILE_MAX_BYTES + 1), 0, None).as_deref(), Some("a.txt is 10.0 MB, over the 10 MB a file may be"));
        assert_eq!(check(file("a.txt", 0), 0, None).as_deref(), Some("a.txt is empty"));
        assert!(check(file("a.png", 10), 0, Some("Cursor")).unwrap().starts_with("Cursor takes no image"));
        assert_eq!(check(file("a.txt", 10), 0, Some("Cursor")), None);
        assert_eq!(file("a.txt", 12 * 1024).size_word(), "12 KB");
    }

    #[test]
    fn enter_starts_queues_or_steers() {
        let agents = agents();
        let claude = agents.iter().find(|a| a.harness == "claude");
        assert_eq!(submit(true, None, false, false, claude, &[]), Submit::Nothing);
        assert_eq!(submit(false, Some(&Hold::Loading), false, false, claude, &[]), Submit::Nothing);
        assert_eq!(submit(false, None, false, false, claude, &[]), Submit::Start);
        assert_eq!(submit(false, None, true, false, claude, &[]), Submit::Queue);
        assert_eq!(submit(false, Some(&Hold::InFlight), false, false, claude, &[]), Submit::Queue);
        assert_eq!(submit(false, None, true, true, claude, &[]), Submit::Steer);
        assert_eq!(submit(false, None, true, true, claude, &[file("a.txt", 1)]), Submit::Queue, "a steer carries no file");
        assert_eq!(submit(false, None, true, true, agents.first(), &[]), Submit::Queue, "codex does not steer here");
    }

    #[test]
    fn the_stash_keeps_the_newest_twenty() {
        let mut stash = Stash::default();
        for i in 0..25 {
            stash.push(Stashed { prompt: format!("draft {i}"), files: vec![], at: std::time::SystemTime::now() });
        }
        assert_eq!(stash.entries.len(), STASH_MAX);
        assert_eq!(stash.entries[0].prompt, "draft 24");
        assert_eq!(stash.take(0).unwrap().prompt, "draft 24");
        assert_eq!(stash.take(99), None);
        assert_eq!(Stash::snippet(&Stashed { prompt: "  a\n\n b ".into(), files: vec![], at: std::time::SystemTime::now() }), "a b");
        assert_eq!(
            Stash::snippet(&Stashed { prompt: String::new(), files: vec![file("x", 1), file("y", 1)], at: std::time::SystemTime::now() }),
            "(2 files)"
        );
        assert_eq!(
            Stash::snippet(&Stashed { prompt: "x".repeat(100), files: vec![], at: std::time::SystemTime::now() }).chars().count(),
            91
        );
    }

    #[test]
    fn a_slash_draft_holds_until_it_names_a_command() {
        let review = Command { name: "review".into(), description: None, source: None };
        let btw = Command { name: "btw".into(), description: None, source: Some("wsp".into()) };
        let all = commands(&[review.clone(), btw]);
        assert_eq!(all, [review], "no aside pane yet, so /btw is not offered");
        assert_eq!(slash_hold("/", &all).as_deref(), Some("a slash on its own is not a command"));
        assert_eq!(slash_hold("/review", &all), None);
        assert_eq!(slash_hold("/btw", &all).as_deref(), Some("no command here is called /btw"));
        assert_eq!(slash_hold("/nope", &all).as_deref(), Some("no command here is called /nope"));
        assert_eq!(slash_hold("/nope and words", &all), None);
        assert_eq!(slash_hold("plain", &all), None);
    }

    #[test]
    fn a_legacy_model_the_thread_ran_resolves() {
        let agent = Agent {
            harness: "claude".into(),
            models: vec![choice("opus", true)],
            legacy_models: vec![choice("sonnet-5", false)],
            ..Default::default()
        };
        let ran = Picks { model: Some("sonnet-5".into()), ..Default::default() };
        assert_eq!(resolve(std::slice::from_ref(&agent), &Picks::default(), &ran).model.unwrap().value, "sonnet-5");
        assert_eq!(resolve(&[agent], &Picks::default(), &Picks::default()).model.unwrap().value, "opus");
    }

    #[test]
    fn words_match_the_web_app() {
        assert_eq!(queue_word(1, false), "1 message waiting");
        assert_eq!(queue_word(3, false), "3 messages waiting");
        assert_eq!(queue_word(1, true), "Sending now");
        assert_eq!(first_line("\n  \n second\nthird"), "second");
        assert_eq!(placeholder(true, 600.), "Ask anything, or / for commands");
        assert_eq!(placeholder(true, 200.), "Ask anything");
        assert_eq!(placeholder(false, 600.), "Ask anything");
        let s = std::time::Duration::from_secs;
        assert_eq!([ago(s(59)), ago(s(60)), ago(s(7200)), ago(s(200_000))], ["just now", "1m ago", "2h ago", "2d ago"]);
    }
}
