// SPDX-License-Identifier: AGPL-3.0-only
//! The workspaces and threads this host records, which a project's thread count and its removal's refusal are counted
//! from (`workspacesOn`, `ThreadCount`): read once a projects page draws, then kept current by the rows and workspaces
//! the host pushes, so a thread that ends or a workspace deleted from the command line moves the page at once.

use serde_json::Value;
use wsp_host_client::protocol::{SessionView, WorkspaceOut};

/// What the page can say about the threads on a project.
#[derive(Debug, PartialEq)]
pub enum Count {
    Reading,
    NotRead,
    /// Threads, and how many of them run.
    Threads(usize, usize),
}

#[derive(Default)]
pub struct Standing {
    workspaces: Option<Vec<WorkspaceOut>>,
    sessions: Option<Vec<SessionView>>,
    /// The host's words when it would not list one or the other.
    refused: Option<String>,
    asked: bool,
    /// Reads sent and not yet answered; a push that comes meanwhile may be missing from their answer.
    out: usize,
    moved: bool,
}

impl Standing {
    pub fn asked(&self) -> bool {
        self.asked
    }

    pub fn reading(&self) -> bool {
        self.out > 0
    }

    /// Both lists are being asked for: what stood before is kept until they land.
    pub fn asking(&mut self) {
        self.asked = true;
        self.refused = None;
        self.out += 2;
    }

    pub fn took_workspaces(&mut self, read: Result<Vec<WorkspaceOut>, String>) {
        self.out = self.out.saturating_sub(1);
        match read {
            Ok(workspaces) => self.workspaces = Some(workspaces),
            Err(said) => self.refused = Some(said),
        }
    }

    pub fn took_sessions(&mut self, read: Result<Vec<SessionView>, String>) {
        self.out = self.out.saturating_sub(1);
        match read {
            Ok(sessions) => self.sessions = Some(sessions),
            Err(said) => self.refused = Some(said),
        }
    }

    /// Whether to read both lists again: a push came while they were out, and both have now landed.
    pub fn again(&mut self) -> bool {
        if self.out > 0 || !self.moved {
            return false;
        }
        self.moved = false;
        true
    }

    /// Lays a pushed `session.row`, `workspace.created` or `workspace.deleted` onto the lists; whether a count moved.
    pub fn push(&mut self, event: &Value) -> bool {
        if !self.asked {
            return false;
        }
        if self.out > 0 {
            self.moved = true;
            return false;
        }
        let kind = event.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "session.row" => {
                let (Some(sessions), Some(id)) = (self.sessions.as_mut(), event.get("id").and_then(Value::as_str)) else { return false };
                let at = sessions.iter().position(|s| s.id == id);
                // `withPushedRow`: in the place of the row under its id, or after the rest; no row takes it out.
                match (event.get("row").cloned().map(serde_json::from_value::<SessionView>), at) {
                    (Some(Ok(row)), Some(at)) => sessions[at] = row,
                    (Some(Ok(row)), None) => sessions.push(row),
                    (None, Some(at)) => {
                        sessions.remove(at);
                    }
                    (None, None) => return false,
                    (Some(Err(_)), _) => {
                        self.moved = true;
                        return false;
                    }
                }
                true
            }
            "workspace.created" => {
                let Some(workspaces) = self.workspaces.as_mut() else { return false };
                let Some(Ok(workspace)) = event.get("workspace").cloned().map(serde_json::from_value::<WorkspaceOut>) else {
                    self.moved = true;
                    return false;
                };
                match workspaces.iter_mut().find(|w| w.id == workspace.id) {
                    Some(at) => *at = workspace,
                    None => workspaces.push(workspace),
                }
                true
            }
            "workspace.deleted" => {
                let Some(id) = event.get("workspaceId").and_then(Value::as_str) else { return false };
                if let Some(workspaces) = self.workspaces.as_mut() {
                    workspaces.retain(|w| w.id != id);
                }
                if let Some(sessions) = self.sessions.as_mut() {
                    sessions.retain(|s| s.workspace_id != id);
                }
                true
            }
            _ => false,
        }
    }

    fn sessions_on<'a>(&'a self, workspace: &'a str) -> impl Iterator<Item = &'a SessionView> {
        self.sessions.iter().flatten().filter(move |s| s.workspace_id == workspace)
    }

    /// `bareFolder`: a workspace in a project's own folder that holds no thread is the folder, not a workspace on it.
    fn bare(&self, workspace: &WorkspaceOut) -> bool {
        let in_folder = workspace.kind.as_ref().is_some_and(|k| matches!(k.to_string().as_str(), "local" | "place"));
        in_folder && self.sessions_on(&workspace.id).next().is_none()
    }

    fn read(&self) -> bool {
        self.refused.is_none() && self.workspaces.is_some() && self.sessions.is_some()
    }

    /// `workspacesOn`: how many workspaces stand on a project, which its removal is refused for; none until read.
    pub fn on(&self, project: &str) -> Option<usize> {
        self.read().then(|| self.workspaces.iter().flatten().filter(|w| w.project.id == project && !self.bare(w)).count())
    }

    /// The threads on a project's workspaces, one per thread however many turns it took, running where its newest
    /// turn runs.
    pub fn threads(&self, project: &str) -> Count {
        if self.refused.is_some() {
            return Count::NotRead;
        }
        let (Some(workspaces), Some(_)) = (&self.workspaces, &self.sessions) else { return Count::Reading };
        let mut threads: Vec<(&str, f64, bool)> = Vec::new();
        for workspace in workspaces.iter().filter(|w| w.project.id == project) {
            for session in self.sessions_on(&workspace.id) {
                let key = session.thread_id.as_deref().unwrap_or(&session.id);
                let (at, running) = (session.started_at.unwrap_or(0.), session.status.to_string() == "running");
                match threads.iter_mut().find(|(k, _, _)| *k == key) {
                    Some(thread) if at >= thread.1 => *thread = (key, at, running),
                    Some(_) => {}
                    None => threads.push((key, at, running)),
                }
            }
        }
        Count::Threads(threads.len(), threads.iter().filter(|(_, _, running)| *running).count())
    }

    /// The host's words for a list it would not give.
    pub fn refused(&self) -> Option<&str> {
        self.refused.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn workspace(id: &str, kind: &str) -> Value {
        json!({ "id": id, "name": id, "machineId": "m", "phase": "running", "kind": kind, "golden": "",
            "createdAt": "2026-10-17T00:00:00Z", "project": { "id": "p", "name": "p", "path": "/p", "computer": "here" } })
    }

    fn row(id: &str, workspace: &str, thread: &str, status: &str, at: f64) -> Value {
        json!({ "id": id, "workspaceId": workspace, "threadId": thread, "harness": "claude", "status": status, "startedAt": at })
    }

    fn read(workspaces: Vec<Value>, sessions: Vec<Value>) -> Standing {
        let mut standing = Standing::default();
        standing.asking();
        standing.took_workspaces(Ok(workspaces.into_iter().map(|w| serde_json::from_value(w).unwrap()).collect()));
        standing.took_sessions(Ok(sessions.into_iter().map(|s| serde_json::from_value(s).unwrap()).collect()));
        standing
    }

    #[test]
    fn a_bare_folder_is_no_workspace_on_its_project() {
        let standing = read(
            vec![workspace("w1", "local"), workspace("w2", "local"), workspace("w3", "cloud")],
            vec![row("s1", "w2", "t1", "completed", 1.), row("s2", "w2", "t1", "running", 2.), row("s3", "w3", "t2", "completed", 1.)],
        );
        assert_eq!(standing.on("p"), Some(2), "w1 holds no thread, so it is the folder");
        assert_eq!(standing.threads("p"), Count::Threads(2, 1), "two turns of t1 are one thread, running as its newest turn runs");
        assert_eq!(standing.on("q"), Some(0));
    }

    #[test]
    fn a_thread_counts_as_its_newest_turn() {
        let standing = read(vec![workspace("w", "cloud")], vec![row("s2", "w", "t", "completed", 2.), row("s1", "w", "t", "running", 1.)]);
        assert_eq!(standing.threads("p"), Count::Threads(1, 0), "an older turn's row does not make the thread run");
    }

    #[test]
    fn pushed_rows_and_workspaces_move_the_counts() {
        let mut standing = read(vec![workspace("w", "cloud")], vec![row("s1", "w", "t1", "running", 1.)]);
        assert_eq!(standing.threads("p"), Count::Threads(1, 1));
        assert!(
            standing.push(&json!({ "type": "session.row", "workspaceId": "w", "id": "s1", "row": row("s1", "w", "t1", "completed", 1.) }))
        );
        assert_eq!(standing.threads("p"), Count::Threads(1, 0), "the thread finished");
        assert!(
            standing.push(&json!({ "type": "session.row", "workspaceId": "w", "id": "s2", "row": row("s2", "w", "t2", "running", 2.) }))
        );
        assert_eq!(standing.threads("p"), Count::Threads(2, 1), "a new thread");
        assert!(standing.push(&json!({ "type": "session.row", "workspaceId": "w", "id": "s2" })));
        assert_eq!(standing.threads("p"), Count::Threads(1, 0), "a row the host no longer holds goes");
        assert!(standing.push(&json!({ "type": "workspace.created", "workspace": workspace("v", "cloud") })));
        assert_eq!(standing.on("p"), Some(2));
        assert!(standing.push(&json!({ "type": "workspace.deleted", "workspaceId": "w" })));
        assert_eq!(standing.on("p"), Some(1), "the deleted workspace no longer holds the remove");
        assert_eq!(standing.threads("p"), Count::Threads(0, 0), "its threads went with it");
    }

    #[test]
    fn nothing_reads_zero_until_both_lists_land_and_a_refusal_says_so() {
        let mut standing = Standing::default();
        assert!(!standing.push(&json!({ "type": "workspace.deleted", "workspaceId": "w" })), "nothing asked, nothing follows");
        standing.asking();
        assert_eq!((standing.threads("p"), standing.on("p")), (Count::Reading, None));
        assert!(!standing.push(&json!({ "type": "workspace.deleted", "workspaceId": "w" })));
        standing.took_workspaces(Ok(vec![]));
        assert_eq!(standing.threads("p"), Count::Reading, "one list is not a count");
        assert!(!standing.again(), "a read is still out");
        standing.took_sessions(Err("the host is starting".into()));
        assert_eq!((standing.threads("p"), standing.on("p")), (Count::NotRead, None));
        assert!(standing.again(), "a push came while the reads were out, so they go again");
        assert!(!standing.again());
    }
}
