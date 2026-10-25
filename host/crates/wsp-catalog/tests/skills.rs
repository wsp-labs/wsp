// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/skills.test.ts.
use regex::Regex;
use serde_json::json;
use wsp_catalog::catalog::{catalog_agents, catalog_entry, AgentEntry};
use wsp_catalog::context::SKILL_NAME;
use wsp_catalog::mcp::{CODEX_TOML, OPENCODE_JSON};
use wsp_catalog::skills::{
    is_system_skill, own_skill_folder, skills_dir_of, Lands, SkillRoot, SHARED_SKILLS, WSP_SKILL_NAME, XDG_SHARED_SKILLS,
};

mod the_folders_each_agent_loads_skills_from {
    use super::*;

    #[test]
    fn every_agent_names_its_own_folder_under_the_home_first_and_project_folders_relati() {
        let own_folder = Regex::new(r"^~/\S+/skills$").unwrap();
        for a in catalog_agents() {
            let dir = skills_dir_of(&a.skill_roots);
            assert!(own_folder.is_match(&dir), "{}: {dir}", a.id);
            for r in &a.skill_roots.user {
                assert!(r.dir.starts_with("~/"), "{} {}", a.id, r.dir);
            }
            for r in &a.skill_roots.project {
                assert!(!(r.dir.starts_with('/') || r.dir.starts_with('~')), "{} {}", a.id, r.dir);
            }
        }
        // No agent's own folder is a shared one: ~/.agents/skills and the XDG ~/.config/agents/skills are each read by
        // several agents and carry no one of them, so a skill installed for one agent never lands where others read it.
        let own: Vec<String> = catalog_agents().iter().map(|a| skills_dir_of(&a.skill_roots)).collect();
        for shared in [SHARED_SKILLS, XDG_SHARED_SKILLS] {
            assert!(!own.iter().any(|d| d == shared), "{shared}");
        }
        assert_eq!(XDG_SHARED_SKILLS, "~/.config/agents/skills");
    }
}

mod a_server_its_file_switches_off {
    use super::*;

    #[test]
    fn reads_opencode_s_enabled_false_and_codex_s_enabled_false_as_off_and_every_other() {
        let text = json!({"mcp": {"a": {"type": "local", "command": ["x"], "enabled": false}, "b": {"type": "remote", "url": "https://b.example"}}}).to_string();
        let opencode: Vec<(String, Option<bool>)> = OPENCODE_JSON.read(&text, "/h").into_iter().map(|s| (s.name, s.disabled)).collect();
        assert_eq!(opencode, vec![("a".to_string(), Some(true)), ("b".to_string(), None)]);
        let codex: Vec<(String, Option<bool>)> = CODEX_TOML
            .read("[mcp_servers.a]\ncommand = \"x\"\nenabled = false\n\n[mcp_servers.b]\ncommand = \"y\"\nenabled = true\n", "/h")
            .into_iter()
            .map(|s| (s.name, s.disabled))
            .collect();
        assert_eq!(codex, vec![("a".to_string(), Some(true)), ("b".to_string(), None)]);
    }
}

mod the_skills_wsp_writes_itself {
    use super::*;

    #[test]
    fn are_the_one_on_the_person_s_computer_and_the_one_on_a_machine_wsp_made_and_no_ot() {
        assert!(is_system_skill(WSP_SKILL_NAME));
        assert!(is_system_skill(SKILL_NAME));
        assert!(!is_system_skill("wsp-review"));
        assert!(!is_system_skill("pdf"));
    }
}

mod where_an_install_puts_a_skill_for_each_agent {
    use super::*;

    fn agent(id: &str) -> &'static AgentEntry {
        catalog_entry(id).unwrap().as_agent().unwrap()
    }

    fn root(dir: &str, lands: Lands) -> Option<SkillRoot> {
        Some(SkillRoot { dir: dir.to_string(), lands })
    }

    #[test]
    fn gives_an_agent_that_reads_the_shared_folder_nothing_of_its_own_and_the_rest_thei() {
        assert_eq!(own_skill_folder(&agent("claude").skill_roots, false), root("~/.claude/skills", Lands::Link));
        assert_eq!(own_skill_folder(&agent("gemini").skill_roots, false), root("~/.gemini/skills", Lands::Copy));
        for id in ["codex", "opencode", "pi"] {
            assert_eq!(own_skill_folder(&agent(id).skill_roots, false), None, "{id}");
        }
        assert_eq!(own_skill_folder(&agent("claude").skill_roots, true), root(".claude/skills", Lands::Link));
        assert_eq!(own_skill_folder(&agent("pi").skill_roots, true), None);
        assert_eq!(own_skill_folder(&agent("hermes").skill_roots, true), None);
    }
}
