// SPDX-License-Identifier: AGPL-3.0-only
//! What every Settings page is drawn from, after `settingsContext.ts` and `settingsReads.ts`: the host's records, read
//! once as Settings opens and kept current by the events the host pushes, and the page Settings stands on.

use serde::de::DeserializeOwned;
use serde_json::Value;
use wsp_host_client::protocol::{
    AccountView, Capabilities, DeviceView, EditorChoice, HarnessCatalog, InitSetup, PendingComputer, PlaceView, Preferences, ProjectView,
    RecipeView, ReleaseView, SealedImageView,
};

/// A settings group, in the order the sidebar draws them (`groupIds.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GroupId {
    General,
    Appearance,
    Agents,
    Computers,
    Recipes,
    Projects,
    Usage,
    Activity,
    Keybindings,
    Privacy,
    Devices,
    Account,
}

impl GroupId {
    pub const ALL: [GroupId; 12] = [
        GroupId::General,
        GroupId::Appearance,
        GroupId::Agents,
        GroupId::Computers,
        GroupId::Recipes,
        GroupId::Projects,
        GroupId::Usage,
        GroupId::Activity,
        GroupId::Keybindings,
        GroupId::Privacy,
        GroupId::Devices,
        GroupId::Account,
    ];

    pub fn id(self) -> &'static str {
        match self {
            GroupId::General => "general",
            GroupId::Appearance => "appearance",
            GroupId::Agents => "agents",
            GroupId::Computers => "computers",
            GroupId::Recipes => "recipes",
            GroupId::Projects => "projects",
            GroupId::Usage => "usage",
            GroupId::Activity => "activity",
            GroupId::Keybindings => "keybindings",
            GroupId::Privacy => "privacy",
            GroupId::Devices => "devices",
            GroupId::Account => "account",
        }
    }

    pub fn parse(word: &str) -> Option<GroupId> {
        GroupId::ALL.into_iter().find(|g| g.id() == word)
    }
}

/// A page of Settings: a group, or a computer's, a project's or an agent's own page (`SettingsAt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum At {
    Group(GroupId),
    Computer(String),
    Project(String),
    Agent(String),
}

impl At {
    /// The page a fresh window opens on.
    pub const FIRST: At = At::Group(GroupId::Appearance);

    /// The one word for a page (`atId`): `appearance`, `computer:<id>`.
    pub fn id(&self) -> String {
        match self {
            At::Group(g) => g.id().to_owned(),
            At::Computer(id) => format!("computer:{id}"),
            At::Project(id) => format!("project:{id}"),
            At::Agent(id) => format!("agent:{id}"),
        }
    }

    /// A page read back off its word, or nothing for a word that names no page (`parseAt`).
    pub fn parse(word: &str) -> Option<At> {
        match word.split_once(':') {
            Some(("computer", id)) if !id.is_empty() => Some(At::Computer(id.to_owned())),
            Some(("project", id)) if !id.is_empty() => Some(At::Project(id.to_owned())),
            Some(("agent", id)) if !id.is_empty() => Some(At::Agent(id.to_owned())),
            Some(_) => None,
            None => GroupId::parse(word).map(At::Group),
        }
    }

    /// The group a page belongs to (`groupOf`).
    pub fn group(&self) -> GroupId {
        match self {
            At::Group(g) => *g,
            At::Computer(_) => GroupId::Computers,
            At::Project(_) => GroupId::Projects,
            At::Agent(_) => GroupId::Agents,
        }
    }
}

/// The host's records the pages read. `None` is a read not yet answered or refused; a refusal a page says a sentence
/// for keeps its words.
#[derive(Default)]
pub struct Data {
    pub preferences: Option<Preferences>,
    pub places: Vec<PlaceView>,
    /// The adds that have not reached Set up.
    pub pending: Vec<PendingComputer>,
    pub places_refused: Option<String>,
    /// Whether the place list has been answered either way, so an empty list is the host's word and not a wait.
    pub places_read: bool,
    pub projects: Vec<ProjectView>,
    pub projects_read: bool,
    pub projects_refused: Option<String>,
    pub harnesses: Vec<HarnessCatalog>,
    pub recipes: Option<Vec<RecipeView>>,
    pub release: Option<ReleaseView>,
    pub capabilities: Option<Capabilities>,
    pub setup: Option<InitSetup>,
    pub account: Option<AccountView>,
    pub devices: Option<Vec<DeviceView>>,
    /// Whether the host refused the devices list to this socket, which the page says in its place.
    pub devices_refused: bool,
    pub image: Option<SealedImageView>,
    pub image_reading: bool,
    /// The editors installed on the computer the host runs on.
    pub editors: Option<Vec<EditorChoice>>,
    /// Whether ~/.ssh/config reads wsp's own ssh config.
    pub ssh_include: Option<bool>,
}

impl Data {
    pub fn preferences(&self) -> Option<&Preferences> {
        self.preferences.as_ref()
    }

    /// Projects in the person's order (`inProjectOrder`): the ordered ids first, the rest by when they were added.
    pub fn projects_in_order(&self) -> Vec<&ProjectView> {
        let order: Vec<String> = self.preferences.as_ref().map(|p| p.project_order.clone()).unwrap_or_default();
        let mut listed: Vec<&ProjectView> = self.projects.iter().collect();
        listed.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        listed.sort_by_key(|p| order.iter().position(|id| *id == p.id).unwrap_or(usize::MAX));
        listed
    }
}

/// One answer from the host, carried to the screen's thread.
pub enum Read {
    Preferences(Result<Preferences, String>),
    Places(Result<(Vec<PlaceView>, Vec<PendingComputer>), String>),
    Projects(Result<Vec<ProjectView>, String>),
    Harnesses(Result<Vec<HarnessCatalog>, String>),
    Recipes(Result<Vec<RecipeView>, String>),
    Release(Result<ReleaseView, String>),
    Capabilities(Result<Capabilities, String>),
    Setup(Result<InitSetup, String>),
    Account(Result<AccountView, String>),
    /// The list, or the refusal and whether it was the ticket socket's.
    Devices(Result<Vec<DeviceView>, (String, bool)>),
    Image(Result<SealedImageView, String>),
    Editors(Result<Vec<EditorChoice>, String>),
    SshInclude(Result<bool, String>),
}

impl Data {
    /// Lays one answer onto the records; a refusal of a read a page has no sentence for leaves what was there.
    pub fn take(&mut self, read: Read) {
        match read {
            Read::Preferences(Ok(p)) => self.preferences = Some(p),
            Read::Places(Ok((places, pending))) => {
                self.places_read = true;
                self.places = places;
                self.pending = pending;
                self.places_refused = None;
            }
            Read::Places(Err(said)) => {
                self.places_read = true;
                self.places.clear();
                self.places_refused = Some(said);
            }
            Read::Projects(Ok(projects)) => {
                self.projects_read = true;
                self.projects = projects;
                self.projects_refused = None;
            }
            Read::Projects(Err(said)) => {
                self.projects_read = true;
                self.projects.clear();
                self.projects_refused = Some(said);
            }
            Read::Harnesses(Ok(h)) => self.harnesses = h,
            Read::Recipes(Ok(r)) => self.recipes = Some(r),
            Read::Release(Ok(r)) => self.release = Some(r),
            Read::Capabilities(Ok(c)) => self.capabilities = Some(c),
            Read::Setup(Ok(s)) => self.setup = Some(s),
            Read::Setup(Err(_)) => self.setup = None,
            Read::Account(Ok(a)) => self.account = Some(a),
            Read::Account(Err(_)) => self.account = None,
            Read::Devices(Ok(d)) => {
                self.devices = Some(d);
                self.devices_refused = false;
            }
            Read::Devices(Err((_, ticket))) => {
                self.devices = None;
                self.devices_refused = ticket;
            }
            Read::Image(result) => {
                self.image_reading = false;
                if let Ok(view) = result {
                    self.image = Some(view);
                }
            }
            Read::Editors(Ok(e)) => self.editors = Some(e),
            Read::SshInclude(Ok(s)) => self.ssh_include = Some(s),
            Read::Preferences(Err(_))
            | Read::Harnesses(Err(_))
            | Read::Recipes(Err(_))
            | Read::Release(Err(_))
            | Read::Capabilities(Err(_))
            | Read::Editors(Err(_))
            | Read::SshInclude(Err(_)) => {}
        }
    }

    /// Lays one pushed event onto the records, as the web app's store does; whether anything a page draws moved.
    pub fn push(&mut self, event: &Value) -> Push {
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        let field = |name: &str| event.get(name).cloned();
        match kind {
            "preferences.changed" => {
                let Some(p) = field("preferences").and_then(read::<Preferences>) else { return Push::Nothing };
                self.preferences = Some(p);
                Push::Drawn
            }
            "release.changed" => {
                let Some(r) = field("release").and_then(read::<ReleaseView>) else { return Push::Nothing };
                self.release = Some(r);
                Push::Drawn
            }
            "place.joined" | "place.changed" => {
                let Some(place) = field("place").and_then(read::<PlaceView>) else { return Push::Nothing };
                match self.places.iter_mut().find(|p| p.id == place.id) {
                    Some(at) => *at = place,
                    None => self.places.push(place),
                }
                Push::Drawn
            }
            "place.removed" => {
                let Some(id) = event.get("placeId").and_then(Value::as_str) else { return Push::Nothing };
                self.places.retain(|p| p.id != id);
                Push::Drawn
            }
            "place.present" | "place.absent" => {
                let Some(id) = event.get("placeId").and_then(Value::as_str) else { return Push::Nothing };
                let present = kind == "place.present";
                for place in self.places.iter_mut().filter(|p| p.id == id) {
                    place.present = Some(present);
                }
                Push::Drawn
            }
            "place.pending" => {
                let Some(id) = event.get("id").and_then(Value::as_str) else { return Push::Nothing };
                self.pending.retain(|p| p.id != id);
                if let Some(pending) = field("pending").and_then(read::<PendingComputer>) {
                    self.pending.push(pending);
                }
                Push::Drawn
            }
            "project.added" => {
                let Some(project) = field("project").and_then(read::<ProjectView>) else { return Push::Nothing };
                self.projects.retain(|p| p.id != project.id);
                self.projects.push(project);
                self.projects.sort_by(|a, b| a.created_at.cmp(&b.created_at));
                Push::Drawn
            }
            "project.removed" => {
                let Some(id) = event.get("projectId").and_then(Value::as_str) else { return Push::Nothing };
                self.projects.retain(|p| p.id != id);
                Push::Drawn
            }
            "recipes.changed" => Push::Reread(Reread::Recipes),
            "agents.changed" => Push::Reread(Reread::Harnesses),
            "init.job" => {
                if let (Some(setup), Some(job)) = (self.setup.as_mut(), field("job").and_then(read)) {
                    setup.job = job;
                }
                let done = event.pointer("/job/phase").and_then(Value::as_str) == Some("done");
                if done {
                    self.image_reading = true;
                    Push::Reread(Reread::Image)
                } else {
                    Push::Drawn
                }
            }
            _ => Push::Nothing,
        }
    }
}

/// What a pushed event asks of the screen.
#[derive(Debug, PartialEq)]
pub enum Push {
    Nothing,
    Drawn,
    Reread(Reread),
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Reread {
    Recipes,
    Harnesses,
    Image,
    Devices,
    Places,
}

fn read<T: DeserializeOwned>(value: Value) -> Option<T> {
    serde_json::from_value(value).ok()
}

/// A request built from its JSON params, as the host reads them.
pub fn req<T: DeserializeOwned>(params: Value) -> T {
    serde_json::from_value(params).expect("a request's params match its recorded schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_page_reads_back_off_its_word() {
        for at in
            [At::FIRST, At::Group(GroupId::Keybindings), At::Computer("here".into()), At::Project("p1".into()), At::Agent("claude".into())]
        {
            assert_eq!(At::parse(&at.id()), Some(at));
        }
        assert_eq!(At::parse("computer:"), None);
        assert_eq!(At::parse("nowhere"), None);
        assert_eq!(At::Computer("x".into()).group(), GroupId::Computers);
    }

    #[test]
    fn a_pushed_removal_takes_the_row_off() {
        let mut data = Data::default();
        let place = json!({ "id": "box", "kind": "computer", "name": "box", "default": false });
        assert_eq!(data.push(&json!({ "type": "place.joined", "place": place })), Push::Drawn);
        assert_eq!(data.places.len(), 1);
        assert_eq!(data.push(&json!({ "type": "place.absent", "placeId": "box" })), Push::Drawn);
        assert_eq!(data.places[0].present, Some(false));
        assert_eq!(data.push(&json!({ "type": "place.removed", "placeId": "box" })), Push::Drawn);
        assert!(data.places.is_empty());
        assert_eq!(data.push(&json!({ "type": "workspace.status" })), Push::Nothing);
    }
}
