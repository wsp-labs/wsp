// SPDX-License-Identifier: AGPL-3.0-only
//! Which shells a drawer shows, which is active and how they group into splits, as pure transitions. A port of
//! `apps/web/src/terminal/groups.ts`, itself adapted from pingdotgg/t3code at 57a66608 (MIT). The shells themselves
//! come from the daemon channel; this only arranges them.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

pub const DEFAULT_HEIGHT: f32 = 280.;
pub const MAX_PER_GROUP: usize = 4;

/// How a group's splits sit: side by side (horizontal) or stacked (vertical).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub terminal_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "is_horizontal")]
    pub split_direction: Direction,
}

fn is_horizontal(d: &Direction) -> bool {
    *d == Direction::Horizontal
}

/// `TerminalUiState`, with the web store's field names so a saved drawer reads the same.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ui {
    pub terminal_open: bool,
    pub terminal_height: f32,
    pub terminal_ids: Vec<String>,
    pub active_terminal_id: String,
    pub terminal_groups: Vec<Group>,
    pub active_terminal_group_id: String,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            terminal_open: false,
            terminal_height: DEFAULT_HEIGHT,
            terminal_ids: Vec::new(),
            active_terminal_id: String::new(),
            terminal_groups: Vec::new(),
            active_terminal_group_id: String::new(),
        }
    }
}

/// Where a split lands: right after the active shell, as the drawer does, or at the end, as the panel's surface does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    AfterActive,
    End,
}

fn normalize_ids(ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.iter().map(|id| id.trim()).filter(|id| !id.is_empty() && seen.insert(id.to_string())).map(str::to_owned).collect()
}

fn fallback_group_id(terminal: &str) -> String {
    format!("group-{terminal}")
}

fn unique_group_id(base: &str, used: &mut HashSet<String>) -> String {
    let mut candidate = base.to_owned();
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{base}-{n}");
        n += 1;
    }
    used.insert(candidate.clone());
    candidate
}

fn normalize_groups(groups: &[Group], ids: &[String]) -> Vec<Group> {
    if ids.is_empty() {
        return Vec::new();
    }
    let valid: HashSet<&str> = ids.iter().map(String::as_str).collect();
    let mut assigned: HashSet<String> = HashSet::new();
    let mut used = HashSet::new();
    let mut next = Vec::new();
    for group in groups {
        let kept: Vec<String> =
            normalize_ids(&group.terminal_ids).into_iter().filter(|id| valid.contains(id.as_str()) && !assigned.contains(id)).collect();
        if kept.is_empty() {
            continue;
        }
        assigned.extend(kept.iter().cloned());
        let base = if group.id.trim().is_empty() { fallback_group_id(&kept[0]) } else { group.id.trim().to_owned() };
        next.push(Group { id: unique_group_id(&base, &mut used), terminal_ids: kept, split_direction: group.split_direction });
    }
    for id in ids {
        if !assigned.contains(id) {
            next.push(Group {
                id: unique_group_id(&fallback_group_id(id), &mut used),
                terminal_ids: vec![id.clone()],
                split_direction: Direction::Horizontal,
            });
        }
    }
    next
}

impl Ui {
    pub fn normalize(&self) -> Ui {
        let ids = normalize_ids(&self.terminal_ids);
        let active =
            if ids.contains(&self.active_terminal_id) { self.active_terminal_id.clone() } else { ids.first().cloned().unwrap_or_default() };
        let groups = normalize_groups(&self.terminal_groups, &ids);
        let active_group = groups
            .iter()
            .find(|g| g.id == self.active_terminal_group_id)
            .or_else(|| groups.iter().find(|g| g.terminal_ids.contains(&active)))
            .or(groups.first())
            .map(|g| g.id.clone())
            .unwrap_or_default();
        Ui {
            terminal_open: self.terminal_open,
            terminal_height: if self.terminal_height.is_finite() && self.terminal_height > 0. {
                self.terminal_height
            } else {
                DEFAULT_HEIGHT
            },
            terminal_ids: ids,
            active_terminal_id: active,
            terminal_groups: groups,
            active_terminal_group_id: active_group,
        }
    }

    /// A drawer left as it starts is not kept, as the web store deletes it.
    pub fn is_default(&self) -> bool {
        self.normalize() == Ui::default()
    }

    pub fn active_group(&self) -> Option<&Group> {
        self.terminal_groups.iter().find(|g| g.id == self.active_terminal_group_id)
    }

    pub fn group_of(&self, terminal: &str) -> Option<&Group> {
        self.terminal_groups.iter().find(|g| g.terminal_ids.iter().any(|t| t == terminal))
    }

    /// Whether a split may add a shell to the active group now; a refused split reads "Max 4 per group".
    pub fn can_split(&self) -> bool {
        self.active_group().is_none_or(|g| g.terminal_ids.len() < MAX_PER_GROUP)
    }

    fn upsert(&self, terminal: &str, split: Option<(Direction, Place)>) -> Ui {
        let n = self.normalize();
        let split = if n.terminal_ids.is_empty() { None } else { split };
        if terminal.trim().is_empty() {
            return n;
        }
        let is_new = !n.terminal_ids.iter().any(|t| t == terminal);
        let mut ids = n.terminal_ids.clone();
        if is_new {
            ids.push(terminal.to_owned());
        }
        let mut groups = n.terminal_groups.clone();
        if let Some(ix) = groups.iter().position(|g| g.terminal_ids.iter().any(|t| t == terminal)) {
            groups[ix].terminal_ids.retain(|t| t != terminal);
            if groups[ix].terminal_ids.is_empty() {
                groups.remove(ix);
            }
        }
        let Some((direction, place)) = split else {
            let mut used: HashSet<String> = groups.iter().map(|g| g.id.clone()).collect();
            let id = unique_group_id(&fallback_group_id(terminal), &mut used);
            groups.push(Group { id: id.clone(), terminal_ids: vec![terminal.to_owned()], split_direction: Direction::Horizontal });
            return Ui {
                terminal_open: true,
                terminal_ids: ids,
                active_terminal_id: terminal.to_owned(),
                terminal_groups: groups,
                active_terminal_group_id: id,
                ..n
            }
            .normalize();
        };
        let mut dest = groups.iter().position(|g| g.id == n.active_terminal_group_id);
        if dest.is_none() {
            dest = groups.iter().position(|g| g.terminal_ids.contains(&n.active_terminal_id));
        }
        let dest = match dest {
            Some(ix) => ix,
            None => {
                let mut used: HashSet<String> = groups.iter().map(|g| g.id.clone()).collect();
                let id = unique_group_id(&fallback_group_id(&n.active_terminal_id), &mut used);
                groups.push(Group { id, terminal_ids: vec![n.active_terminal_id.clone()], split_direction: Direction::Horizontal });
                groups.len() - 1
            }
        };
        let group = &mut groups[dest];
        let holds = group.terminal_ids.iter().any(|t| t == terminal);
        if is_new && !holds && group.terminal_ids.len() >= MAX_PER_GROUP {
            return n;
        }
        if !holds {
            let anchor = group.terminal_ids.iter().position(|t| *t == n.active_terminal_id).filter(|_| place == Place::AfterActive);
            match anchor {
                Some(ix) => group.terminal_ids.insert(ix + 1, terminal.to_owned()),
                None => group.terminal_ids.push(terminal.to_owned()),
            }
        }
        group.split_direction = direction;
        let group_id = group.id.clone();
        Ui {
            terminal_open: true,
            terminal_ids: ids,
            active_terminal_id: terminal.to_owned(),
            terminal_groups: groups,
            active_terminal_group_id: group_id,
            ..n
        }
        .normalize()
    }

    pub fn split(&self, terminal: &str, direction: Direction, place: Place) -> Ui {
        self.upsert(terminal, Some((direction, place)))
    }

    pub fn new_terminal(&self, terminal: &str) -> Ui {
        self.upsert(terminal, None)
    }

    pub fn set_open(&self, open: bool) -> Ui {
        Ui { terminal_open: open, ..self.normalize() }
    }

    pub fn set_height(&self, height: f32) -> Ui {
        let n = self.normalize();
        if !height.is_finite() || height <= 0. {
            return n;
        }
        Ui { terminal_height: height, ..n }
    }

    pub fn activate(&self, terminal: &str) -> Ui {
        let n = self.normalize();
        if !n.terminal_ids.iter().any(|t| t == terminal) {
            return n;
        }
        let group = n.group_of(terminal).map(|g| g.id.clone()).unwrap_or_else(|| n.active_terminal_group_id.clone());
        Ui { active_terminal_id: terminal.to_owned(), active_terminal_group_id: group, ..n }
    }

    /// Drops `terminal`; the next shell becomes active, else the previous. Closing the last keeps only the height, so
    /// the drawer closes with it.
    pub fn close(&self, terminal: &str) -> Ui {
        let n = self.normalize();
        let Some(closed_at) = n.terminal_ids.iter().position(|t| t == terminal) else { return n };
        let remaining: Vec<String> = n.terminal_ids.iter().filter(|t| *t != terminal).cloned().collect();
        if remaining.is_empty() {
            return Ui { terminal_height: n.terminal_height, ..Ui::default() };
        }
        let active = if n.active_terminal_id == terminal {
            remaining[closed_at.min(remaining.len() - 1)].clone()
        } else {
            n.active_terminal_id.clone()
        };
        let groups: Vec<Group> = n
            .terminal_groups
            .iter()
            .filter_map(|g| {
                let ids: Vec<String> = g.terminal_ids.iter().filter(|t| *t != terminal).cloned().collect();
                (!ids.is_empty()).then(|| Group { terminal_ids: ids, ..g.clone() })
            })
            .collect();
        let active_group = groups
            .iter()
            .find(|g| g.terminal_ids.contains(&active))
            .or(groups.first())
            .map(|g| g.id.clone())
            .unwrap_or_else(|| fallback_group_id(&active));
        Ui {
            terminal_open: n.terminal_open,
            terminal_height: n.terminal_height,
            terminal_ids: remaining,
            active_terminal_id: active,
            terminal_groups: groups,
            active_terminal_group_id: active_group,
        }
        .normalize()
    }

    /// The daemon's list is the truth for which shells exist; the arrangement follows it.
    pub fn reconcile(&self, next: &[String]) -> Ui {
        let n = self.normalize();
        if n.terminal_ids == next {
            return n;
        }
        let active =
            if next.contains(&n.active_terminal_id) { n.active_terminal_id.clone() } else { next.first().cloned().unwrap_or_default() };
        let groups = normalize_groups(&n.terminal_groups, next);
        let active_group = groups
            .iter()
            .find(|g| g.terminal_ids.contains(&active))
            .map(|g| g.id.clone())
            .or_else(|| groups.iter().any(|g| g.id == n.active_terminal_group_id).then(|| n.active_terminal_group_id.clone()))
            .or_else(|| groups.first().map(|g| g.id.clone()))
            .unwrap_or_default();
        Ui { terminal_ids: next.to_vec(), active_terminal_id: active, terminal_groups: groups, active_terminal_group_id: active_group, ..n }
            .normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(ids: &[&str]) -> Ui {
        let mut ui = Ui::default();
        for id in ids {
            ui = ui.new_terminal(id);
        }
        ui
    }

    fn group_ids(ui: &Ui) -> Vec<Vec<&str>> {
        ui.terminal_groups.iter().map(|g| g.terminal_ids.iter().map(String::as_str).collect()).collect()
    }

    #[test]
    fn a_new_shell_opens_its_own_group_and_the_drawer() {
        let ui = with(&["a", "b"]);
        assert!(ui.terminal_open);
        assert_eq!(group_ids(&ui), [vec!["a"], vec!["b"]]);
        assert_eq!(ui.active_terminal_id, "b");
        assert_eq!(ui.active_terminal_group_id, "group-b");
    }

    #[test]
    fn a_split_lands_right_after_the_active_shell_and_sets_the_groups_direction() {
        let ui = with(&["a"]).split("b", Direction::Horizontal, Place::AfterActive).split("c", Direction::Horizontal, Place::AfterActive);
        let ui = ui.activate("b").split("d", Direction::Vertical, Place::AfterActive);
        assert_eq!(group_ids(&ui), [vec!["a", "b", "d", "c"]]);
        assert_eq!(ui.terminal_groups[0].split_direction, Direction::Vertical);
        assert_eq!(ui.active_terminal_id, "d");
    }

    #[test]
    fn the_panels_split_lands_at_the_end() {
        let ui = with(&["a"]).split("b", Direction::Horizontal, Place::End).activate("a").split("c", Direction::Horizontal, Place::End);
        assert_eq!(group_ids(&ui), [vec!["a", "b", "c"]]);
    }

    #[test]
    fn a_group_holds_four_and_refuses_a_fifth() {
        let mut ui = with(&["a"]);
        for id in ["b", "c", "d"] {
            ui = ui.split(id, Direction::Horizontal, Place::AfterActive);
        }
        assert!(!ui.can_split());
        let refused = ui.split("e", Direction::Horizontal, Place::AfterActive);
        assert_eq!(refused, ui.normalize());
        assert!(!refused.terminal_ids.contains(&"e".to_owned()));
    }

    #[test]
    fn closing_the_active_shell_activates_the_next_else_the_previous() {
        let ui = with(&["a", "b", "c"]).activate("b");
        let ui = ui.close("b");
        assert_eq!(ui.active_terminal_id, "c");
        let ui = ui.close("c");
        assert_eq!(ui.active_terminal_id, "a");
        assert_eq!(ui.active_terminal_group_id, "group-a");
    }

    #[test]
    fn closing_the_last_shell_closes_the_drawer_and_keeps_its_height() {
        let ui = with(&["a"]).set_height(400.).close("a");
        assert!(!ui.terminal_open);
        assert_eq!(ui.terminal_height, 400.);
        assert!(ui.terminal_ids.is_empty());
    }

    #[test]
    fn normalize_drops_blank_and_twice_listed_shells_and_gives_each_stray_a_group() {
        let ui = Ui {
            terminal_ids: vec![" a ".into(), "".into(), "a".into(), "b".into()],
            terminal_groups: vec![Group {
                id: "g".into(),
                terminal_ids: vec!["a".into(), "x".into()],
                split_direction: Direction::Vertical,
            }],
            active_terminal_id: "zz".into(),
            terminal_height: f32::NAN,
            ..Ui::default()
        }
        .normalize();
        assert_eq!(ui.terminal_ids, ["a", "b"]);
        assert_eq!(group_ids(&ui), [vec!["a"], vec!["b"]]);
        assert_eq!(ui.active_terminal_id, "a");
        assert_eq!(ui.active_terminal_group_id, "g");
        assert_eq!(ui.terminal_height, DEFAULT_HEIGHT);
    }

    #[test]
    fn the_daemons_list_reconciles_the_arrangement() {
        let ui = with(&["a"]).split("b", Direction::Horizontal, Place::AfterActive);
        let ui = ui.reconcile(&["b".into(), "c".into()]);
        assert_eq!(group_ids(&ui), [vec!["b"], vec!["c"]]);
        assert_eq!(ui.active_terminal_id, "b");
    }

    #[test]
    fn a_saved_drawer_reads_back_with_the_web_stores_names() {
        let ui = with(&["a"]).split("b", Direction::Vertical, Place::AfterActive);
        let json = serde_json::to_value(&ui).unwrap();
        assert_eq!(json["activeTerminalId"], "b");
        assert_eq!(json["terminalGroups"][0]["splitDirection"], "vertical");
        let back: Ui = serde_json::from_value(json).unwrap();
        assert_eq!(back, ui);
    }
}
