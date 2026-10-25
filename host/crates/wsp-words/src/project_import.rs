// SPDX-License-Identifier: AGPL-3.0-only
//! What an import asks about a project's secret files and how a row says each one. Port of the functions of packages/protocol/src/project-import.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::project_import. See spec/project_import.md.

use wsp_wire::views::project_bundle::ProjectAgent;
use wsp_wire::types::project_import::ProjectImportRequest;
use wsp_wire::views::project_bundle::ProjectPlan;
use wsp_wire::views::project_bundle::ProjectSecret;

use indexmap::IndexSet;
use serde::Serialize;

/// A rewrite removes the credentials, so it starts ticked; a file that would travel as it is never does.
// project-import.ts:10
// export function defaultConsent(secrets: readonly ProjectSecret[]): ReadonlySet<string> {
//   return new Set(secrets.filter(s => s.rewrite !== undefined).map(s => s.path));
// }
pub fn default_consent(secrets: &[ProjectSecret]) -> IndexSet<String> {
    secrets.iter().filter(|s| s.rewrite.is_some()).map(|s| s.path.clone()).collect()
}

/// The request the ticks become: offered files rewrite, the rest carry; everything unticked is cut and named by the runtime.
// project-import.ts:15
// export function consentRequest(secrets: readonly ProjectSecret[], ticked: ReadonlySet<string>): { carry: string[]; rewrite: string[] } {
//   const chosen = secrets.filter(s => ticked.has(s.path));
//   return {
//     carry: chosen.filter(s => s.rewrite === undefined).map(s => s.path),
//     rewrite: chosen.filter(s => s.rewrite !== undefined).map(s => s.path),
//   };
// }
/// The TS `{ carry, rewrite }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ConsentRequest {
    pub carry: Vec<String>,
    pub rewrite: Vec<String>,
}

pub fn consent_request(secrets: &[ProjectSecret], ticked: &IndexSet<String>) -> ConsentRequest {
    let chosen: Vec<&ProjectSecret> = secrets.iter().filter(|s| ticked.contains(&s.path)).collect();
    ConsentRequest {
        carry: chosen.iter().filter(|s| s.rewrite.is_none()).map(|s| s.path.clone()).collect(),
        rewrite: chosen.iter().filter(|s| s.rewrite.is_some()).map(|s| s.path.clone()).collect(),
    }
}

/// Only an agent with sessions the plan could read has anything to send; the runtime never reads one it cannot.
// project-import.ts:24
// export const canTravel = (a: ProjectAgent): boolean => a.error === undefined && a.sessions > 0;
pub fn can_travel(a: &ProjectAgent) -> bool {
    a.error.is_none() && a.sessions.0 > 0.0
}

/// An agent whose sessions can travel starts ticked; one with none, or whose store could not be read, does not.
// project-import.ts:27
// export function defaultAgents(agents: readonly ProjectAgent[]): ReadonlySet<string> {
//   return new Set(agents.filter(canTravel).map(a => a.agent));
// }
pub fn default_agents(agents: &[ProjectAgent]) -> IndexSet<String> {
    agents.iter().filter(|a| can_travel(a)).map(|a| a.agent.clone()).collect()
}

/// The request the agent ticks become, in the plan's order; nothing when none is ticked, which the runtime reads the same.
// project-import.ts:32
// export function agentsRequest(agents: readonly ProjectAgent[], ticked: ReadonlySet<string>): string[] | undefined {
//   const chosen = agents.filter(a => ticked.has(a.agent)).map(a => a.agent);
//   return chosen.length === 0 ? undefined : chosen;
// }
pub fn agents_request(agents: &[ProjectAgent], ticked: &IndexSet<String>) -> Option<Vec<String>> {
    let chosen: Vec<String> = agents.iter().filter(|a| ticked.contains(&a.agent)).map(|a| a.agent.clone()).collect();
    if chosen.is_empty() { None } else { Some(chosen) }
}

/// What a person answered under the plan: yes to its defaults, or a keep or cut per secret-shaped row.
// project-import.ts:38
// export interface ImportAnswers {
//   yes?: boolean | undefined;
//   keep: readonly string[];
//   cut: readonly string[];
// }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportAnswers {
    pub yes: Option<bool>,
    pub keep: Vec<String>,
    pub cut: Vec<String>,
}

/// Whether anything may leave this computer: a yes, or an answer to a row; nothing given is the dry run.
// project-import.ts:45
// export function importConsented(a: ImportAnswers): boolean {
//   return a.yes === true || a.keep.length > 0 || a.cut.length > 0;
// }
pub fn import_consented(a: &ImportAnswers) -> bool {
    a.yes == Some(true) || !a.keep.is_empty() || !a.cut.is_empty()
}

// What project.import is asked for: the folder here as given (events echo it), where it lands on the machine,
// which secret-shaped rows travel as they are or rewritten, whose sessions travel, and whether to replace.
// project-import.ts:51
// in wsp_wire::types::project_import: ProjectImportRequest

/// The one request every client sends from a plan and its ticks; the folder lands at the path it has here, which
/// is where init's road puts a project on the machine it seals.
// project-import.ts:62
// export function importRequest(plan: ProjectPlan, source: string, ticked: ReadonlySet<string>, agents: ReadonlySet<string>, replace?: boolean): ProjectImportRequest {
//   const travelling = agentsRequest(plan.agents, agents);
//   return { source, dest: plan.source, ...consentRequest(plan.secrets, ticked), ...(travelling !== undefined ? { agents: travelling } : {}), ...(replace === true ? { replace: true } : {}) };
// }
pub fn import_request(plan: &ProjectPlan, source: &str, ticked: &IndexSet<String>, agents: &IndexSet<String>, replace: Option<bool>) -> ProjectImportRequest {
    let travelling = agents_request(&plan.agents, agents);
    let consent = consent_request(&plan.secrets, ticked);
    ProjectImportRequest {
        source: source.to_string(),
        dest: plan.source.clone(),
        carry: consent.carry,
        rewrite: consent.rewrite,
        agents: travelling,
        replace: if replace == Some(true) { Some(Default::default()) } else { None },
    }
}

/// The row's words for its tick, short for the row's end and full for its title: left out, travels as is, or rewritten
/// without its keys, the full form naming what the remote then reads and which config keys are left out.
// project-import.ts:69
// export function secretOffer(s: ProjectSecret, ticked: boolean): { short: string; full: string } {
//   if (!ticked) return { short: "left out", full: "left out" };
//   if (s.rewrite === undefined) return { short: "travels as is", full: "travels as is" };
//   const short = "rewritten without keys";
//   const remote = s.rewrite.urls.length > 0 ? `; the remote reads ${s.rewrite.urls.join(", ")}` : "";
//   const dropped = s.rewrite.drop.length > 0 ? `; ${s.rewrite.drop.join(", ")} left out` : "";
//   return { short, full: `${short}${remote}${dropped}` };
// }
/// The TS `{ short, full }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SecretOffer {
    pub short: String,
    pub full: String,
}

pub fn secret_offer(s: &ProjectSecret, ticked: bool) -> SecretOffer {
    if !ticked {
        return SecretOffer { short: "left out".into(), full: "left out".into() };
    }
    let Some(rewrite) = &s.rewrite else {
        return SecretOffer { short: "travels as is".into(), full: "travels as is".into() };
    };
    let short = "rewritten without keys";
    let remote = if rewrite.urls.is_empty() { String::new() } else { format!("; the remote reads {}", rewrite.urls.join(", ")) };
    let dropped = if rewrite.drop.is_empty() { String::new() } else { format!("; {} left out", rewrite.drop.join(", ")) };
    SecretOffer { short: short.into(), full: format!("{short}{remote}{dropped}") }
}

