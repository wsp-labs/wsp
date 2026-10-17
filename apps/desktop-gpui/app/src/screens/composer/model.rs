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
}

/// The trigger the text before `cursor` (a byte offset) ends in, and the byte range of its token.
pub fn trigger(text: &str, cursor: usize) -> Option<(Trigger, Range<usize>)> {
    let before = text.get(..cursor)?;
    let start = before.rfind(char::is_whitespace).map(|i| i + before[i..].chars().next().map_or(1, char::len_utf8)).unwrap_or(0);
    let token = &before[start..];
    if let Some(word) = token.strip_prefix('/') {
        return (start == 0).then(|| (Trigger::Slash(word.to_owned()), start..cursor));
    }
    token.strip_prefix('@').map(|path| (Trigger::Mention(path.to_owned()), start..cursor))
}

/// One row of the command menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// The group head it sits under.
    pub group: String,
    /// What the menu shows: `/name`, or a file's basename.
    pub label: String,
    /// The muted words after it: a command's description, a file's folder.
    pub note: Option<String>,
    /// What a pick puts in the text: `/name ` for a command, the path for a file's chip.
    pub insert: String,
    pub file: bool,
}

/// A slash command the agent announced, or wsp's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub description: Option<String>,
    /// The group the menu lists it under: a plugin's prefix, or `wsp` for wsp's own.
    pub source: Option<String>,
}

/// `/btw`, which wsp adds where the agent takes a side question (apps/web/src/adapt/catalog.ts).
pub fn aside() -> Command {
    Command {
        name: "btw".to_owned(),
        description: Some("Ask a side question the thread never keeps".to_owned()),
        source: Some("wsp".to_owned()),
    }
}

/// The commands the menu offers: the announced ones, then `/btw` where the agent takes asides.
pub fn commands(announced: &[Command], agent: Option<&Agent>) -> Vec<Command> {
    let mut all: Vec<Command> = announced.iter().filter(|c| c.name != "btw").cloned().collect();
    if agent.is_some_and(|a| a.asides) {
        all.push(aside());
    }
    all
}

/// What holds a send of a slash draft (ChatComposer.tsx `slashHold`): a lone slash, a command nothing announced with
/// nothing after it, or `/btw` with no question.
pub fn slash_hold(text: &str, commands: &[Command]) -> Option<String> {
    let text = text.trim();
    let rest = text.strip_prefix('/')?;
    if rest.is_empty() {
        return Some("a slash on its own is not a command".to_owned());
    }
    let (name, after) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    if name == "btw" && after.trim().is_empty() {
        return Some("/btw takes a question after it".to_owned());
    }
    (after.trim().is_empty() && !commands.iter().any(|c| c.name == name)).then(|| format!("no command here is called /{name}"))
}

/// The rows a trigger lists: commands whose name starts with the word, or files whose path holds the query, best
/// first, at most `cap`.
pub fn menu_items(trigger: &Trigger, commands: &[Command], files: &[String], cap: usize) -> Vec<MenuItem> {
    match trigger {
        Trigger::Slash(word) => {
            let word = word.to_lowercase();
            commands
                .iter()
                .filter(|c| c.name.to_lowercase().starts_with(&word))
                .take(cap)
                .map(|c| MenuItem {
                    group: c.source.clone().unwrap_or_else(|| "Commands".to_owned()),
                    label: format!("/{}", c.name),
                    note: c.description.clone(),
                    insert: format!("/{} ", c.name),
                    file: false,
                })
                .collect()
        }
        Trigger::Mention(query) => {
            let query = query.to_lowercase();
            let mut hits: Vec<(usize, &String)> = files
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
                .take(cap)
                .map(|(_, path)| {
                    let (folder, name) = match path.trim_end_matches('/').rsplit_once('/') {
                        Some((folder, name)) => (Some(folder.to_owned()), name.to_owned()),
                        None => (None, path.clone()),
                    };
                    MenuItem { group: "Files".to_owned(), label: name, note: folder, insert: path.clone(), file: true }
                })
                .collect()
        }
    }
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
    pub at: std::time::Instant,
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
        let rows = menu_items(&Trigger::Slash("re".into()), &commands, &[], 10);
        assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["/review"]);
        assert_eq!(rows[0].insert, "/review ");
        assert_eq!(rows[0].group, "Commands");
        let files: Vec<String> = ["src/lib/main.rs", "main.rs", "docs/domain.md", "src/remain.ts"].map(String::from).to_vec();
        let rows = menu_items(&Trigger::Mention("main".into()), &[], &files, 10);
        assert_eq!(
            rows.iter().map(|r| r.insert.as_str()).collect::<Vec<_>>(),
            ["src/lib/main.rs", "main.rs", "docs/domain.md", "src/remain.ts"]
        );
        assert_eq!(rows[0].label, "main.rs");
        assert_eq!(rows[0].note.as_deref(), Some("src/lib"));
        assert_eq!(menu_items(&Trigger::Mention(String::new()), &[], &files, 2).len(), 2);
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
            stash.push(Stashed { prompt: format!("draft {i}"), files: vec![], at: std::time::Instant::now() });
        }
        assert_eq!(stash.entries.len(), STASH_MAX);
        assert_eq!(stash.entries[0].prompt, "draft 24");
        assert_eq!(stash.take(0).unwrap().prompt, "draft 24");
        assert_eq!(stash.take(99), None);
        assert_eq!(Stash::snippet(&Stashed { prompt: "  a\n\n b ".into(), files: vec![], at: std::time::Instant::now() }), "a b");
        assert_eq!(
            Stash::snippet(&Stashed { prompt: String::new(), files: vec![file("x", 1), file("y", 1)], at: std::time::Instant::now() }),
            "(2 files)"
        );
        assert_eq!(Stash::snippet(&Stashed { prompt: "x".repeat(100), files: vec![], at: std::time::Instant::now() }).chars().count(), 91);
    }

    #[test]
    fn a_slash_draft_holds_until_it_names_a_command() {
        let claude = Agent { asides: true, ..Default::default() };
        let all = commands(&[], Some(&claude));
        assert_eq!(all, [aside()]);
        assert!(commands(&[], None).is_empty());
        assert_eq!(slash_hold("/", &all).as_deref(), Some("a slash on its own is not a command"));
        assert_eq!(slash_hold("/btw", &all).as_deref(), Some("/btw takes a question after it"));
        assert_eq!(slash_hold("/btw why", &all), None);
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
