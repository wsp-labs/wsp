// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/skills.test.ts at a3b870a6e: the skills on a computer, read off a real home.
mod common;

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::symlink;
use std::rc::Rc;

use common::agent_home::agent_home;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tempfile::TempDir;
use wsp_catalog::catalog::catalog_entry;
use wsp_collect::detect::plugins::{detect_plugins, AgentPlugins};
use wsp_collect::detect::skills::{detect_skills, skill_frontmatter, skill_md_frontmatter, skill_roots, SkillFront};
use wsp_collect::host::{Host, HostExec, RunOptions};
use wsp_collect::live_host::node_host;
use wsp_wire::types::agents_report::{AgentsProject, SkillRow};

#[derive(Clone, Copy)]
enum Mode {
    Record,
    Cut,
    Fail,
    Blind,
}

struct Wrap {
    inner: Rc<dyn HostExec>,
    runs: Rc<RefCell<Vec<Vec<String>>>>,
    mode: Mode,
}

impl HostExec for Wrap {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        self.inner.which(bin)
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        Box::pin(async move {
            match self.mode {
                Mode::Record => {
                    let mut line = vec![cmd.to_string()];
                    line.extend(args.iter().cloned());
                    self.runs.borrow_mut().push(line);
                    self.inner.run(cmd, args, opts).await
                }
                Mode::Cut => {
                    let out = self.inner.run(cmd, args, opts).await?;
                    let cut = out.strip_suffix("\u{1e}END\n").map(str::to_string);
                    Some(cut.unwrap_or(out))
                }
                Mode::Fail => None,
                Mode::Blind => {
                    if args.get(1).is_some_and(|a| a.contains("linked()")) {
                        None
                    } else {
                        self.inner.run(cmd, args, opts).await
                    }
                }
            }
        })
    }
}

struct Fixture {
    host: Host,
    home: String,
    project: String,
    runs: Rc<RefCell<Vec<Vec<String>>>>,
    _root: TempDir,
}

fn fixture() -> Fixture {
    let root = tempfile::Builder::new().prefix("wsp-skills-").tempdir().unwrap();
    let agent = agent_home(root.path().to_str().unwrap());
    let live = node_host().unwrap();
    let runs = Rc::new(RefCell::new(Vec::new()));
    let host =
        Host { home: agent.home.clone(), exec: Rc::new(Wrap { inner: live.exec.clone(), runs: runs.clone(), mode: Mode::Record }), ..live };
    Fixture { host, home: agent.home, project: agent.project, runs, _root: root }
}

fn with_mode(host: &Host, mode: Mode) -> Host {
    Host { exec: Rc::new(Wrap { inner: host.exec.clone(), runs: Rc::new(RefCell::new(Vec::new())), mode }), ..host.clone() }
}

/// Claude Code's plugins alone, whose module reads files and runs nothing.
async fn plugins_of(host: &Host) -> Vec<AgentPlugins> {
    let claude = catalog_entry("claude").unwrap().as_agent().unwrap();
    detect_plugins(host, Some(&[claude]), &[]).await.agents
}

fn find<'a>(skills: &'a [SkillRow], name: &str) -> &'a SkillRow {
    skills.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no skill {name}"))
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn project(id: &str, name: &str, path: &str) -> AgentsProject {
    AgentsProject { id: id.to_string(), name: name.to_string(), path: path.to_string() }
}

#[tokio::test]
async fn lists_every_skill_once_by_name_with_every_folder_it_lives_in() {
    let f = fixture();
    let plugins = plugins_of(&f.host).await;
    f.runs.borrow_mut().clear();
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &plugins).await).await;
    assert_eq!(read.refused, Vec::<String>::new());
    // One command for every folder, however many skills they hold.
    assert_eq!(f.runs.borrow().len(), 1);
    let named: Vec<Value> = read.skills.iter().map(|s| json!([s.name, s.scope])).collect();
    assert_eq!(
        named,
        vec![
            json!(["frontend:frontend-design", "plugin"]),
            json!(["pdf", "user"]),
            json!(["plan", "user"]),
            json!(["review", "user"]),
            json!(["sql", "user"]),
        ]
    );
    let pdf = find(&read.skills, "pdf");
    assert_eq!(pdf.description.as_deref(), Some("Read and fill PDF forms"));
    assert_eq!(
        json_of(&pdf.paths),
        json!([
            { "path": "~/.claude/skills/pdf", "agent": "claude", "linkTo": "~/.agents/skills/pdf" },
            { "path": "~/.codex/skills/pdf", "agent": "codex" },
            { "path": "~/.agents/skills/pdf" },
            { "path": "~/.pi/agent/skills/pdf", "agent": "pi", "linkTo": "~/.agents/skills/pdf" },
        ])
    );
    let sql_agents: Vec<Option<&str>> = find(&read.skills, "sql").paths.iter().map(|p| p.agent.as_deref()).collect();
    assert_eq!(sql_agents, vec![Some("gemini"), Some("opencode")]);
    assert_eq!(find(&read.skills, "review").description.as_deref(), Some("Review a diff"));
    assert_eq!(
        json_of(&find(&read.skills, "plan").paths),
        json!([{ "path": "~/.hermes/skills/software-development/plan", "agent": "hermes" }])
    );
    let plugin = find(&read.skills, "frontend:frontend-design");
    assert_eq!(plugin.plugin.as_deref(), Some("frontend@official"));
    assert_eq!(
        json_of(&plugin.paths),
        json!([{ "path": "~/.claude/plugins/cache/official/frontend/abc123/skills/frontend-design", "agent": "claude" }])
    );
}

#[tokio::test]
async fn reads_a_plugins_skills_off_while_settings_switch_its_plugin_off() {
    let f = fixture();
    fs::write(format!("{}/.claude/settings.json", f.home), json!({ "enabledPlugins": { "frontend@official": false } }).to_string())
        .unwrap();
    let plugins = plugins_of(&f.host).await;
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &plugins).await).await;
    let plugin = read.skills.iter().find(|s| s.plugin.as_deref() == Some("frontend@official")).unwrap();
    assert_eq!(plugin.name, "frontend:frontend-design");
    assert!(!plugin.paths.is_empty());
    assert!(json_of(&plugin.paths).as_array().unwrap().iter().all(|p| p["off"] == json!(true)));
}

#[tokio::test]
async fn reads_the_agents_own_skill_folders_and_claude_codes_plugin_index_under_the_folder_each_store_names() {
    let f = fixture();
    let home = &f.home;
    let (claude, codex) = (format!("{home}/store-claude"), format!("{home}/store-codex"));
    fs::rename(format!("{home}/.claude"), &claude).unwrap();
    fs::rename(format!("{home}/.codex"), &codex).unwrap();
    // The index names folders under the old home, as an index carried over does; the store's own copy is what Claude reads.
    let index = format!("{claude}/plugins/installed_plugins.json");
    let old = fs::read_to_string(&index).unwrap();
    fs::write(&index, old.replace(&format!("{home}/.claude/"), &format!("{claude}/"))).unwrap();
    let stores: IndexMap<String, String> = IndexMap::from([("claude".to_string(), claude.clone()), ("codex".to_string(), codex.clone())]);
    let at = Host { stores: Some(stores), ..f.host.clone() };
    let plugins = plugins_of(&at).await;
    let read = detect_skills(&at, &skill_roots(&at, None, &[], &plugins).await).await;
    assert_eq!(
        find(&read.skills, "frontend:frontend-design").paths[0].path,
        "~/store-claude/plugins/cache/official/frontend/abc123/skills/frontend-design"
    );
    let pdf_paths: Vec<&str> = find(&read.skills, "pdf").paths.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(pdf_paths, vec!["~/store-claude/skills/pdf", "~/store-codex/skills/pdf", "~/.agents/skills/pdf", "~/.pi/agent/skills/pdf"]);
}

#[tokio::test]
async fn leaves_out_dot_folders_folders_with_no_skill_md_a_skills_own_inner_examples() {
    let f = fixture();
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &[]).await).await;
    let names: Vec<&str> = read.skills.iter().map(|s| s.name.as_str()).collect();
    for never in ["hidden", "cached", "notes", "inner", "project-only"] {
        assert!(!names.contains(&never), "{never} in {names:?}");
    }
    assert!(names.contains(&"pdf"));
}

#[tokio::test]
async fn reads_a_folder_whose_skill_md_was_renamed_skill_md_off() {
    let f = fixture();
    let home = &f.home;
    let shared = format!("{home}/.agents/skills/memo");
    fs::create_dir_all(&shared).unwrap();
    fs::write(format!("{shared}/SKILL.md"), "---\nname: memo\ndescription: Keep notes\n---\nbody\n").unwrap();
    symlink(&shared, format!("{home}/.claude/skills/memo")).unwrap();
    fs::rename(format!("{shared}/SKILL.md"), format!("{shared}/SKILL.md.off")).unwrap();
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &[]).await).await;
    let memo = find(&read.skills, "memo");
    assert_eq!(memo.description.as_deref(), Some("Keep notes"));
    assert_eq!(
        json_of(&memo.paths),
        json!([
            { "path": "~/.claude/skills/memo", "agent": "claude", "linkTo": "~/.agents/skills/memo", "off": true },
            { "path": "~/.agents/skills/memo", "off": true },
        ])
    );
    // A folder holding both is on: an agent loads the SKILL.md it finds.
    fs::write(format!("{shared}/SKILL.md"), "---\nname: memo\n---\n").unwrap();
    let again = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &[]).await).await;
    assert!(find(&again.skills, "memo").paths.iter().all(|p| p.off.is_none()));
}

#[tokio::test]
async fn reads_a_skill_md_saved_with_crlf_line_ends_as_the_node_reader_reads_it() {
    let f = fixture();
    let text = "---\r\nname: memo\r\ndescription: Keep notes\r\n---\r\nbody\r\n";
    fs::create_dir_all(format!("{}/.agents/skills/memo", f.home)).unwrap();
    fs::write(format!("{}/.agents/skills/memo/SKILL.md", f.home), text).unwrap();
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &[]).await).await;
    let memo = find(&read.skills, "memo");
    assert_eq!(memo.description.as_deref(), Some("Keep notes"));
    assert_eq!(memo.description, skill_md_frontmatter(text).description);
}

#[tokio::test]
async fn keys_a_skill_by_its_folders_name() {
    let f = fixture();
    fs::create_dir_all(format!("{}/.agents/skills/cool-tool", f.home)).unwrap();
    fs::write(format!("{}/.agents/skills/cool-tool/SKILL.md", f.home), "---\nname: pdf\ndescription: Not the real one\n---\n").unwrap();
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &[], &[]).await).await;
    assert_eq!(json_of(&find(&read.skills, "cool-tool").paths), json!([{ "path": "~/.agents/skills/cool-tool" }]));
    assert!(!find(&read.skills, "pdf").paths.iter().any(|p| p.path == "~/.agents/skills/cool-tool"));
}

#[tokio::test]
async fn a_workspace_adds_its_projects_own_folders_as_project_skills() {
    let f = fixture();
    let projects = [project("pr_app", "app", &f.project)];
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &projects, &[]).await).await;
    let app = json!({ "id": "pr_app", "name": "app", "path": "~/code/app" });
    let rows: Vec<Value> =
        read.skills.iter().filter(|s| json_of(&s.scope) == json!("project")).map(|s| json!([s.name, s.paths, s.project])).collect();
    assert_eq!(
        rows,
        vec![
            json!(["deploy", [{ "path": "~/code/app/.claude/skills/deploy", "agent": "claude" }], app]),
            json!(["lint", [{ "path": "~/code/app/.agents/skills/lint", "agent": "codex" }], app]),
        ]
    );
}

#[tokio::test]
async fn a_computers_projects_each_add_their_folders() {
    let f = fixture();
    let other = format!("{}/code/www", f.home);
    fs::create_dir_all(format!("{other}/.claude/skills/deploy")).unwrap();
    fs::write(format!("{other}/.claude/skills/deploy/SKILL.md"), "---\nname: deploy\ndescription: Ship www\n---\n").unwrap();
    let projects = [project("pr_app", "app", &f.project), project("pr_www", "www", &other)];
    let read = detect_skills(&f.host, &skill_roots(&f.host, None, &projects, &[]).await).await;
    let deploys: Vec<Value> = read
        .skills
        .iter()
        .filter(|s| s.name == "deploy")
        .map(|s| {
            json!([s.project.as_ref().map(|p| p.name.clone()), s.description, s.paths.iter().map(|p| p.path.clone()).collect::<Vec<_>>()])
        })
        .collect();
    assert_eq!(
        deploys,
        vec![
            json!(["app", "Deploy the app", ["~/code/app/.claude/skills/deploy"]]),
            json!(["www", "Ship www", ["~/code/www/.claude/skills/deploy"]]),
        ]
    );
    assert!(read.skills.iter().filter(|s| json_of(&s.scope) != json!("project")).all(|s| s.project.is_none()));
}

#[tokio::test]
async fn an_answer_cut_short_is_a_refusal_and_a_read_that_failed_is_one() {
    let f = fixture();
    let cut = with_mode(&f.host, Mode::Cut);
    let roots = skill_roots(&f.host, None, &[], &[]).await;
    assert_eq!(detect_skills(&cut, &roots).await.refused, vec!["skills: the answer was cut short, so the list is not whole"]);
    let failed = with_mode(&f.host, Mode::Fail);
    let read = detect_skills(&failed, &roots).await;
    assert_eq!(json_of(&read.skills), json!([]));
    assert_eq!(read.refused, vec!["skills: the folders could not be read"]);
}

#[tokio::test]
async fn a_project_whose_links_could_not_be_told_is_not_read() {
    let f = fixture();
    let blind = with_mode(&f.host, Mode::Blind);
    let projects = [project("pr_app", "app", &f.project)];
    let read = detect_skills(&blind, &skill_roots(&blind, None, &projects, &[]).await).await;
    assert!(!read.skills.iter().any(|s| json_of(&s.scope) == json!("project")));
    assert!(
        read.refused.contains(
            &"skills: ~/code/app/.claude/skills was not read, since whether it links out of the repo could not be told".to_string()
        ),
        "{:?}",
        read.refused
    );
    assert!(read.refused.iter().all(|l| l.starts_with("skills: ~/code/app/") && l.ends_with("could not be told")), "{:?}", read.refused);
}

#[tokio::test]
async fn a_project_whose_folder_is_not_on_the_computer_reads_no_skills_there() {
    let f = fixture();
    for path in [format!("{}/code/gone", f.home), format!("{}/code/gone/deeper", f.home)] {
        let projects = [project("pr_gone", "gone", &path)];
        let read = detect_skills(&f.host, &skill_roots(&f.host, None, &projects, &[]).await).await;
        assert!(!read.skills.iter().any(|s| json_of(&s.scope) == json!("project")), "{path}");
        assert_eq!(read.refused, Vec::<String>::new(), "{path}");
    }
}

fn front(name: Option<&str>, description: Option<&str>, names: Option<u32>) -> SkillFront {
    SkillFront { name: name.map(str::to_string), description: description.map(str::to_string), names }
}

#[test]
fn reads_a_frontmatters_plain_quoted_folded_and_continued_values() {
    assert_eq!(skill_frontmatter("name: \"a b\"\ndescription: >-\n  one\n  two\nother: x"), front(Some("a b"), Some("one two"), None));
    assert_eq!(
        skill_frontmatter("name: c\ndescription: starts here\n  and goes on"),
        front(Some("c"), Some("starts here and goes on"), None)
    );
    assert_eq!(skill_frontmatter("title: none"), SkillFront::default());
    assert_eq!(skill_frontmatter("name: pdf\nname: memo"), front(Some("pdf"), None, Some(2)));
}

#[test]
fn reads_the_name_key_however_yaml_spells_it_and_nothing_past_the_closing() {
    let name = |text: &str| skill_md_frontmatter(text).name;
    let pdf = Some("pdf".to_string());
    assert_eq!(name("---\nname: pdf\n---\n"), pdf);
    assert_eq!(name("---\nname: \"pdf\"\n---\n"), pdf);
    assert_eq!(name("---\nname: 'pdf'\n---\n"), pdf);
    assert_eq!(name("---\ndescription: none\n---\n"), None);
    assert_eq!(name("---\ndescription: none\n---\nname: pdf\n"), None);
    assert_eq!(name("---\nname : pdf\n---\n"), pdf);
    assert_eq!(name("---\n\"name\": pdf\n---\n"), pdf);
    assert_eq!(name("---\n'name': pdf\n---\n"), pdf);
    assert_eq!(name("---\nname: pdf # the pdf skill\n---\n"), pdf);
    assert_eq!(name("---\nname: \"pdf\" # quoted\n---\n"), pdf);
    assert_eq!(name("---\nname: a#b\n---\n"), Some("a#b".to_string()));
    assert_eq!(
        skill_md_frontmatter("---\nname: 'it''s'\ndescription: \"say \\\"hi\\\"\"\n---\n"),
        front(Some("it's"), Some("say \"hi\""), None)
    );
    assert_eq!(name("---\nnamed: pdf\n  name: memo\n---\n"), None);
    assert_eq!(skill_md_frontmatter("---\nname: pdf\n\"name\": memo\n---\n"), front(Some("pdf"), None, Some(2)));
    assert_eq!(skill_md_frontmatter("---\nname: pdf\nname : memo\n---\n"), front(Some("pdf"), None, Some(2)));
    assert_eq!(skill_md_frontmatter("---\nname: pdf\n\"name\" : memo\n---\n"), front(Some("pdf"), None, Some(2)));
    assert_eq!(skill_md_frontmatter("---\nname: pdf\n'name' : memo\n---\n"), front(Some("pdf"), None, Some(2)));
    assert_eq!(skill_md_frontmatter("---\nname: # c\n---\n"), SkillFront::default());
}

// Port of packages/collect/test/skills-script.test.ts: the one shell script that reads every skill folder prints what the
// per-file reader did. Each case runs once per shell, in a loop inside the test.
mod the_skills_script {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use common::agent_home::write_stub;
    use wsp_collect::detect::skills::{SkillRootAt, SKILL_HEAD_BYTES};
    use wsp_wire::types::agents_report::SkillScope;

    /// The reader as it stood with one find, one head and one awk per SKILL.md, less a line's trailing CR: the new one must print
    /// every record of this one that names a skill folder under its root, and nothing else.
    fn per_file_script() -> String {
        [
            r#"for r in "$@"; do"#.to_string(),
            r#"  [ -d "$r" ] || continue"#.to_string(),
            r#"  find -L "$r" -mindepth 2 -maxdepth 3 \( -name SKILL.md -o -name SKILL.md.off \) -type f 2>/dev/null | while IFS= read -r f; do"#.to_string(),
            "    d=${f%/*}".to_string(),
            r#"    o=; case $f in *.off) [ -f "$d/SKILL.md" ] && continue; o=1;; esac"#.to_string(),
            r#"    l=; [ -L "$d" ] && l=$(readlink "$d")"#.to_string(),
            r#"    printf '\036%s\037%s\037%s\037%s\037' "$r" "$d" "$l" "$o""#.to_string(),
            format!(r#"    head -c {SKILL_HEAD_BYTES} "$f" | awk '{{sub(/\r$/, "")}} NR==1 && $0 != "---" {{exit}} NR>1 && $0 == "---" {{exit}} NR>1 {{print}}'"#),
            "  done".to_string(),
            "done".to_string(),
            r#"printf '\036END\n'"#.to_string(),
        ]
        .join("\n")
    }

    fn temp() -> TempDir {
        tempfile::Builder::new().prefix("wsp-skills-script-").tempdir().unwrap()
    }

    fn skill(dir: &str, text: &str, file: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(format!("{dir}/{file}"), text).unwrap();
    }

    fn front(name: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: The {name} skill, é and 字\n---\n{body}")
    }

    fn root_at(dir: String) -> SkillRootAt {
        SkillRootAt { dir, scope: SkillScope::User, agent: None, project: None, skipped: None, plugin: None }
    }

    /// A home with `n` plain skills over several roots, plus every odd shape the reader has to print the same way.
    fn tree(base: &str, n: usize) -> Vec<SkillRootAt> {
        let shared = format!("{base}/shared skills");
        let own = format!("{base}/own");
        let cats = format!("{base}/cats");
        for d in [&shared, &own, &cats] {
            fs::create_dir_all(d).unwrap();
        }
        for i in 0..n {
            let root = match i % 3 {
                0 => shared.clone(),
                1 => own.clone(),
                _ => format!("{cats}/group {}", i % 5),
            };
            skill(&format!("{root}/skill {i}"), &front(&format!("skill-{i}"), &"body\n".repeat(i % 7)), "SKILL.md");
            if i % 10 == 0 {
                symlink(format!("{shared}/skill {}", i - (i % 3)), format!("{own}/linked {i}")).unwrap();
            }
            if i % 12 == 0 {
                symlink(format!("../shared skills/skill {}", i - (i % 3)), format!("{own}/rel {i}")).unwrap();
            }
            if i % 25 == 0 {
                skill(&format!("{own}/skill {i}"), &front(&format!("skill-{i}"), ""), "SKILL.md.off");
            }
            if i % 25 == 5 {
                skill(&format!("{shared}/off {i}"), &front(&format!("off-{i}"), ""), "SKILL.md.off");
            }
        }
        skill(&format!("{own}/open"), "---\nname: open\ndescription: never closed\nmore\n", "SKILL.md");
        skill(&format!("{own}/crlf"), "---\r\nname: crlf\r\ndescription: windows\r\n---\r\n", "SKILL.md");
        skill(&format!("{own}/crlf inside"), "---\nname: crlf-inside\r\ndescription: half\r\n---\n", "SKILL.md");
        skill(&format!("{own}/empty"), "", "SKILL.md");
        skill(&format!("{own}/no front"), "# Title\nname: nope\n", "SKILL.md");
        skill(&format!("{own}/no newline"), "---\nname: tail", "SKILL.md");
        skill(&format!("{own}/long open"), &format!("---\nname: long\n{}", "description: a long line of words\n".repeat(300)), "SKILL.md");
        skill(&format!("{own}/one line"), &format!("---\n{}\n---\n", "x".repeat(SKILL_HEAD_BYTES * 2)), "SKILL.md");
        for k in -8i64..=3 {
            let pad = (SKILL_HEAD_BYTES as i64 - 14 + k) as usize;
            skill(&format!("{own}/edge {k}"), &format!("---\nname: {}\n---\nafter\n", "e".repeat(pad)), "SKILL.md");
            let tail = if k % 2 == 0 { "" } else { "e" };
            skill(&format!("{own}/wide {k}"), &format!("---\nname: {}{tail}\n---\n", "é".repeat(pad / 2)), "SKILL.md");
        }
        skill(&format!("{own}/.hidden/inner"), &front("hidden", ""), "SKILL.md");
        skill(&format!("{own}/outer"), &front("outer", ""), "SKILL.md");
        skill(&format!("{own}/outer/examples"), &front("inner", ""), "SKILL.md");
        skill(&format!("{shared}/it's here"), &front("quote", ""), "SKILL.md");
        symlink(format!("{shared}/it's here"), format!("{own}/it's linked")).unwrap();
        skill(&format!("{shared}/a -> b"), &front("arrow", ""), "SKILL.md");
        symlink(format!("{shared}/a -> b"), format!("{own}/x -> y")).unwrap();
        symlink(format!("{base}/missing"), format!("{own}/dangling")).unwrap();
        skill(&format!("{own}/a\x1fsplit"), &front("unit-separator", ""), "SKILL.md");
        skill(&format!("{own}/a\x1erecord"), &front("record-separator", ""), "SKILL.md");
        skill(&format!("{own}/a\nline"), &front("newline", ""), "SKILL.md");
        skill(&format!("{own}/a\n\nblank"), &front("blank-line", ""), "SKILL.md");
        skill(&format!("{own}/locked"), &front("locked", ""), "SKILL.md");
        fs::set_permissions(format!("{own}/locked/SKILL.md"), fs::Permissions::from_mode(0o000)).unwrap();
        [own, shared, format!("{base}/absent"), format!("{cats}/group 0"), cats].into_iter().map(root_at).collect()
    }

    /// Every sh run done by `shell`, and the text each run printed kept.
    struct Recording {
        inner: Rc<dyn HostExec>,
        shell: String,
        vars: Option<IndexMap<String, String>>,
        said: Rc<RefCell<Vec<String>>>,
    }

    impl HostExec for Recording {
        fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
            self.inner.which(bin)
        }

        fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
            Box::pin(async move {
                let cmd = if cmd == "sh" { self.shell.as_str() } else { cmd };
                let out = self.inner.run(cmd, args, RunOptions { env: self.vars.clone().or(opts.env), ..opts }).await;
                if let Some(o) = &out {
                    self.said.borrow_mut().push(o.clone());
                }
                out
            })
        }
    }

    fn recording(shell: &str, vars: Option<IndexMap<String, String>>) -> (Host, Rc<RefCell<Vec<String>>>) {
        let live = node_host().unwrap();
        let said = Rc::new(RefCell::new(Vec::new()));
        let exec = Recording { inner: live.exec.clone(), shell: shell.to_string(), vars, said: said.clone() };
        (Host { exec: Rc::new(exec), ..live }, said)
    }

    fn vars_of(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    /// The records that name a folder under their root, in order; the rest are the pieces of a name with a newline in it.
    fn records(said: &str) -> Vec<String> {
        let parts: Vec<&str> = said.split('\u{1e}').collect();
        if parts.len() < 2 {
            return Vec::new();
        }
        parts[1..parts.len() - 1]
            .iter()
            .filter(|r| {
                let mut it = r.split('\u{1f}');
                let root = it.next().unwrap_or("");
                let dir = it.next().unwrap_or("");
                dir.starts_with(&format!("{root}/"))
            })
            .map(|r| r.to_string())
            .collect()
    }

    fn shells() -> Vec<&'static str> {
        ["sh", "/bin/dash"].into_iter().filter(|s| *s == "sh" || Path::new(s).exists()).collect()
    }

    #[tokio::test]
    async fn prints_the_same_records_as_one_head_and_awk_per_file_did_under() {
        for shell in shells() {
            let dir = temp();
            let base = dir.path().to_str().unwrap().to_string();
            let roots = tree(&base, 300);
            let vars = vars_of(&[("LANG", "en_US.UTF-8"), ("LC_ALL", "en_US.UTF-8"), ("QUOTING_STYLE", "shell-always")]);
            let (host, said) = recording(shell, Some(vars.clone()));
            detect_skills(&host, &roots).await;
            let mut args = vec!["-c".to_string(), per_file_script(), "sh".to_string()];
            args.extend(roots.iter().map(|r| r.dir.clone()));
            let before =
                node_host().unwrap().exec.run(shell, &args, RunOptions { env: Some(vars), timeout_ms: Some(60_000), signal: None }).await;
            let before = before.unwrap_or_else(|| panic!("the per-file reader printed nothing under {shell}"));
            assert!(before.split('\u{1e}').count() > 300, "{shell}");
            let said = said.borrow();
            assert!(said[0].ends_with("\u{1e}END\n"), "{shell}");
            let (mut now, mut then) = (records(&said[0]), records(&before));
            now.sort();
            then.sort();
            assert_eq!(now, then, "{shell}");
            // One root sits inside the other under cats, and the one find reaches each file there once, so only the rest keep their order.
            let cats = format!("{base}/cats");
            let apart = |rs: Vec<String>| -> Vec<String> { rs.into_iter().filter(|r| !r.starts_with(&cats)).collect() };
            assert_eq!(apart(records(&said[0])), apart(records(&before)), "{shell}");
        }
    }

    #[tokio::test]
    async fn reads_an_awk_that_dies_as_a_list_cut_short_under() {
        for shell in shells() {
            let dir = temp();
            let base = dir.path().to_str().unwrap().to_string();
            let roots = tree(&format!("{base}/home"), 3);
            let stubs = format!("{base}/stubs");
            fs::create_dir(&stubs).unwrap();
            let real = ["/usr/bin/awk", "/bin/awk"].into_iter().find(|p| Path::new(p).exists()).unwrap();
            write_stub(&format!("{stubs}/awk"), &format!("#!/bin/sh\n{real} \"$@\"\nexit 2\n"));
            let (host, _) = recording(shell, Some(vars_of(&[("PATH", &format!("{stubs}:/usr/bin:/bin"))])));
            let read = detect_skills(&host, &roots).await;
            assert_eq!(serde_json::to_value(&read.skills).unwrap(), json!([]), "{shell}");
            assert_eq!(read.refused, vec!["skills: the folders could not be read"], "{shell}");
        }
    }

    #[tokio::test]
    async fn starts_as_many_processes_for_300_skills_as_for_10_and_for_25_roots_as_for_5() {
        async fn count(shell: &str, n: usize, more: usize) -> usize {
            let dir = temp();
            let base = dir.path().to_str().unwrap().to_string();
            let mut roots = tree(&format!("{base}/home"), n);
            for i in 0..more {
                skill(&format!("{base}/more {i}/one"), &front(&format!("more-{i}"), ""), "SKILL.md");
                roots.push(root_at(format!("{base}/more {i}")));
            }
            let stubs = format!("{base}/stubs");
            let log = format!("{base}/log");
            fs::create_dir(&stubs).unwrap();
            fs::write(&log, "").unwrap();
            for bin in ["head", "awk", "readlink", "ls", "find", "cat", "stat"] {
                let Some(real) = ["/usr/bin", "/bin"].into_iter().map(|d| format!("{d}/{bin}")).find(|p| Path::new(p).exists()) else {
                    continue;
                };
                write_stub(&format!("{stubs}/{bin}"), &format!("#!/bin/sh\necho {bin} >> '{log}'\nexec {real} \"$@\"\n"));
            }
            let (host, _) = recording(shell, Some(vars_of(&[("PATH", &format!("{stubs}:/usr/bin:/bin"))])));
            let read = detect_skills(&host, &roots).await;
            assert_eq!(read.refused, Vec::<String>::new(), "{shell}");
            fs::read_to_string(&log).unwrap().split('\n').filter(|l| !l.is_empty()).count()
        }
        for shell in shells() {
            assert_eq!(count(shell, 300, 0).await, count(shell, 10, 0).await, "{shell}");
            assert_eq!(count(shell, 10, 20).await, count(shell, 10, 0).await, "{shell}");
        }
    }
}

/// Rules of spec/skills.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use super::*;
    use crate::common::fake_host::*;
    use wsp_catalog::catalog::catalog_agents;
    use wsp_catalog::skills::{Lands, SkillRoot, SkillRoots};
    use wsp_collect::detect::skills::{detect_skills, skill_md_frontmatter, skill_roots, SkillRootAt, LINKED_SCRIPT, SCRIPT};
    use wsp_wire::types::agents_report::{AgentsProject, SkillScope};

    const R: &str = "/Users/dev/.a/skills";

    fn rec(root: &str, dir: &str, link: &str, off: &str, head: &str) -> String {
        format!("\u{1e}{root}\u{1f}{dir}\u{1f}{link}\u{1f}{off}\u{1f}{head}")
    }

    fn root(dir: &str, scope: SkillScope, agent: Option<&str>, project: Option<AgentsProject>) -> SkillRootAt {
        SkillRootAt { dir: dir.into(), scope, agent: agent.map(Into::into), project, skipped: None, plugin: None }
    }

    fn agent(id: &str, user: &[&str], project: &[&str]) -> wsp_catalog::catalog::AgentEntry {
        let mut a = catalog_agents()[0].clone();
        a.id = id.into();
        let roots = |dirs: &[&str]| dirs.iter().map(|d| SkillRoot { dir: d.to_string(), lands: Lands::Copy }).collect();
        a.skill_roots = SkillRoots { user: roots(user), project: roots(project) };
        a
    }

    #[test]
    fn detect_skills_with_only_skipped_roots_runs_nothing() {
        let host = empty_host();
        let mut skipped = root(R, SkillScope::Project, None, None);
        skipped.skipped = Some("skills: ~/.a/skills was not read".into());
        let got = block_on(detect_skills(&host, &[skipped]));
        assert!(got.skills.is_empty());
        assert_eq!(got.refused, vec!["skills: ~/.a/skills was not read"]);
        assert_eq!(host.calls(), Vec::<String>::new());
    }

    #[test]
    fn detect_skills_reads_records_off_a_canned_answer() {
        let said = [
            rec(R, &format!("{R}/pdf"), "", "", "name: pdf\ndescription: Read PDFs\n"),
            // A skill's own example, a dot folder, an unknown root and a dir outside its root are no skills.
            rec(R, &format!("{R}/pdf/examples/inner"), "", "", "name: inner\n"),
            rec(R, &format!("{R}/.hidden/x"), "", "", "name: h\n"),
            rec("/other", "/other/y", "", "", "name: y\n"),
            rec(R, &format!("{R}/cat/zeta"), "../../shared/zeta", "1", "description: Z\n"),
            // The same folder again adds no second path.
            rec(R, &format!("{R}/pdf"), "", "", "name: pdf\n"),
            rec(R, "/elsewhere/q", "", "", ""),
        ]
        .concat()
            + "\u{1e}END\n";
        let host = fake_host(FakeLaptop::new().exec(&format!("sh -c {SCRIPT} sh {R}"), &said));
        let got = block_on(detect_skills(&host, &[root(R, SkillScope::User, Some("a"), None)]));
        assert_eq!(host.calls(), vec![format!("run sh -c {SCRIPT} sh {R} (20000 ms)")]);
        assert_eq!(got.refused, Vec::<String>::new());
        assert_eq!(
            json_of(&got.skills),
            json!([
                {"name": "pdf", "description": "Read PDFs", "paths": [{"path": "~/.a/skills/pdf", "agent": "a"}], "scope": "user"},
                {"name": "zeta", "description": "Z", "paths": [{"path": "~/.a/skills/cat/zeta", "agent": "a", "linkTo": "~/.a/shared/zeta", "off": true}], "scope": "user"}
            ])
        );
    }

    #[test]
    fn detect_skills_sorts_rows_by_name_scope_then_project_with_collate() {
        let project =
            |id: &str, name: &str| Some(AgentsProject { id: id.into(), name: name.into(), path: format!("/Users/dev/code/{id}") });
        let said = [
            rec(R, &format!("{R}/beta"), "", "", ""),
            rec(R, &format!("{R}/Alpha"), "", "", ""),
            rec(R, &format!("{R}/alpha"), "", "", ""),
            rec(R, &format!("{R}/_x"), "", "", ""),
            rec("/p2/s", "/p2/s/beta", "", "", ""),
            rec("/p1/s", "/p1/s/beta", "", "", ""),
        ]
        .concat()
            + "\u{1e}END";
        let roots = [
            root(R, SkillScope::User, None, None),
            root("/p2/s", SkillScope::Project, None, project("p2", "zed")),
            root("/p1/s", SkillScope::Project, None, project("p1", "Ann")),
        ];
        let host = fake_host(FakeLaptop::new().exec(&format!("sh -c {SCRIPT} sh {R} /p2/s /p1/s"), &said));
        let got = block_on(detect_skills(&host, &roots));
        let order: Vec<(String, Value, Option<String>)> =
            got.skills.iter().map(|r| (r.name.clone(), json_of(&r.scope), r.project.as_ref().map(|p| p.name.clone()))).collect();
        // localeCompare: punctuation first, then lowercase before uppercase of the same letters.
        assert_eq!(
            order,
            vec![
                ("_x".into(), json!("user"), None),
                ("alpha".into(), json!("user"), None),
                ("Alpha".into(), json!("user"), None),
                ("beta".into(), json!("project"), Some("Ann".into())),
                ("beta".into(), json!("project"), Some("zed".into())),
                ("beta".into(), json!("user"), None),
            ]
        );
    }

    #[test]
    fn an_answer_with_no_end_mark_is_cut_short_and_its_stray_piece_is_no_record() {
        let host = fake_host(FakeLaptop::new().exec(&format!("sh -c {SCRIPT} sh {R}"), "\u{1e}EN"));
        let got = block_on(detect_skills(&host, &[root(R, SkillScope::User, None, None)]));
        assert!(got.skills.is_empty());
        assert_eq!(got.refused, vec!["skills: the answer was cut short, so the list is not whole"]);
    }

    #[test]
    fn skill_md_frontmatter_reads_only_the_first_4096_units() {
        // "---\n" and "description: " are 17 units, so 4079 of the two-byte é fit: a cut by bytes would keep 2039.
        let text = format!("---\ndescription: {}", "\u{e9}".repeat(5000));
        let got = skill_md_frontmatter(&text);
        assert_eq!(got.description.as_deref().map(|d| d.chars().count()), Some(4079));
    }

    #[test]
    fn skill_roots_gives_a_shared_folder_the_agent_whose_own_it_is() {
        let a = agent("a", &["~/.a/skills", "~/.agents/skills"], &[]);
        let b = agent("b", &["~/.agents/skills"], &[]);
        let host = empty_host();
        let got = block_on(skill_roots(&host, Some(&[&a, &b]), &[], &[]));
        // The shared folder keeps the place a added it at, and takes b, whose own it is.
        assert_eq!(
            got,
            vec![
                root("/Users/dev/.a/skills", SkillScope::User, Some("a"), None),
                root("/Users/dev/.agents/skills", SkillScope::User, Some("b"), None)
            ]
        );
    }

    #[test]
    fn skill_roots_marks_a_project_folder_that_links_out_of_the_repo() {
        let a = agent("a", &["~/.a/skills"], &[".a/skills", "./.agents/skills/"]);
        let b = agent("b", &["~/.b/skills"], &[".agents/skills"]);
        let p = AgentsProject { id: "p".into(), name: "app".into(), path: "/Users/dev/code/app".into() };
        let dirs = "/Users/dev/code/app/.a/skills /Users/dev/code/app/.agents/skills/ /Users/dev/code/app/.agents/skills";
        let answer = "/Users/dev/code/app/.a/skills\t/Users/dev/code/app/.a\t/elsewhere\n/Users/dev/code/app/.agents/skills\t/Users/dev/code/app/.agents/skills\t/x\n/not/asked\t/y\t/z\n\u{1e}END\n";
        let host = fake_host(FakeLaptop::new().exec(&format!("sh -c {LINKED_SCRIPT} sh /Users/dev/code/app {dirs}"), answer));
        let got = block_on(skill_roots(&host, Some(&[&a, &b]), std::slice::from_ref(&p), &[]));
        assert_eq!(host.calls(), vec![format!("run sh -c {LINKED_SCRIPT} sh /Users/dev/code/app {dirs} (20000 ms)")]);
        let skipped = |r: SkillRootAt, line: &str| SkillRootAt { skipped: Some(line.into()), ..r };
        // posix.join keeps a trailing slash, so `./.agents/skills/` is a root of its own beside `.agents/skills`.
        assert_eq!(
            got,
            vec![
                root("/Users/dev/.a/skills", SkillScope::User, Some("a"), None),
                root("/Users/dev/.b/skills", SkillScope::User, Some("b"), None),
                skipped(
                    root("/Users/dev/code/app/.a/skills", SkillScope::Project, Some("a"), Some(p.clone())),
                    "skills: ~/code/app/.a/skills links out of the repo through ~/code/app/.a, to /elsewhere, so its skills are not read"
                ),
                root("/Users/dev/code/app/.agents/skills/", SkillScope::Project, None, Some(p.clone())),
                skipped(
                    root("/Users/dev/code/app/.agents/skills", SkillScope::Project, Some("b"), Some(p.clone())),
                    "skills: ~/code/app/.agents/skills links out of the repo, to /x, so its skills are not read"
                ),
            ]
        );
    }
}

/// Every case of tests/fixtures/oracle/w2/skills.json (oracle/w2-skills.oracle.ts): spec skills's wave-2 rules. A case
/// with a `tree` runs on a real folder (`<dir>`) through the live host's files and processes.
mod w2_oracle {
    use super::*;

    fn agent(id: &str) -> &'static wsp_catalog::catalog::AgentEntry {
        wsp_catalog::catalog::catalog_agents().into_iter().find(|a| a.id == id).unwrap()
    }
    use common::fake_host::block_on;
    use common::oracle::{cases, host, w2};
    use wsp_collect::detect::skills::{SkillPlugin, SkillRootAt};
    use wsp_wire::types::agents_report::SkillScope;

    fn front(f: &SkillFront) -> Value {
        let mut o = serde_json::Map::new();
        if let Some(v) = &f.name {
            o.insert("name".into(), json!(v));
        }
        if let Some(v) = &f.description {
            o.insert("description".into(), json!(v));
        }
        if let Some(v) = f.names {
            o.insert("names".into(), json!(v));
        }
        Value::Object(o)
    }

    fn root_json(r: &SkillRootAt) -> Value {
        let mut o = serde_json::Map::new();
        o.insert("dir".into(), json!(r.dir));
        o.insert("scope".into(), serde_json::to_value(r.scope).unwrap());
        if let Some(v) = &r.agent {
            o.insert("agent".into(), json!(v));
        }
        if let Some(v) = &r.project {
            o.insert("project".into(), serde_json::to_value(v).unwrap());
        }
        if let Some(v) = &r.skipped {
            o.insert("skipped".into(), json!(v));
        }
        Value::Object(o)
    }

    fn root_of(v: &Value) -> SkillRootAt {
        SkillRootAt {
            dir: v["dir"].as_str().unwrap().into(),
            scope: serde_json::from_value::<SkillScope>(v["scope"].clone()).unwrap(),
            agent: v["agent"].as_str().map(str::to_string),
            project: v.get("project").map(|p| serde_json::from_value(p.clone()).unwrap()),
            skipped: v["skipped"].as_str().map(str::to_string),
            plugin: v.get("plugin").map(|p| SkillPlugin {
                id: p["id"].as_str().unwrap().into(),
                prefix: p["prefix"].as_str().unwrap().into(),
                on: p["on"].as_bool().unwrap(),
            }),
        }
    }

    fn agents_of(v: &Value) -> Option<Vec<&'static wsp_catalog::catalog::AgentEntry>> {
        v.as_array().map(|a| a.iter().map(|id| agent(id.as_str().unwrap())).collect())
    }

    #[test]
    fn front_matter_reads_as_the_ts_read_it() {
        let f = w2("skills");
        for case in cases(&f, "skill_frontmatter") {
            assert_eq!(front(&skill_frontmatter(case["args"][0].as_str().unwrap())), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "skill_md_frontmatter") {
            if case["out"].get("units").is_some() {
                continue;
            }
            assert_eq!(front(&skill_md_frontmatter(case["args"][0].as_str().unwrap())), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn skill_roots_answer_and_run_what_the_ts_did() {
        for case in cases(&w2("skills"), "skill_roots") {
            let h = host(&case["laptop"]);
            let a = &case["args"];
            let agents = agents_of(&a[0]);
            let projects: Vec<AgentsProject> = serde_json::from_value(a[1].clone()).unwrap();
            let got: Vec<Value> = block_on(skill_roots(&h, agents.as_deref(), &projects, &[])).iter().map(root_json).collect();
            assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }

    #[test]
    fn detect_skills_reads_what_the_ts_read() {
        for case in cases(&w2("skills"), "detect_skills") {
            let what = case["name"].as_str().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_str().unwrap().to_string();
            let fake = if let Some(tree) = case["tree"].as_object() {
                for (rel, v) in tree {
                    let path = format!("{root}/{}", rel.trim_end_matches('/'));
                    fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
                    match v {
                        Value::String(t) => fs::write(&path, t).unwrap(),
                        Value::Object(o) if o.contains_key("link") => {
                            symlink(o["link"].as_str().unwrap().replace("<dir>", &root), &path).unwrap()
                        }
                        _ => fs::create_dir_all(&path).unwrap(),
                    }
                }
                None
            } else {
                Some(host(&case["laptop"]))
            };
            let all: Vec<SkillRootAt> = serde_json::from_str::<Value>(&case["args"][0].to_string().replace("<dir>", &root))
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(root_of)
                .collect();
            let live;
            let on: &Host = match &fake {
                Some(f) => f,
                None => {
                    live = node_host().unwrap();
                    &live
                }
            };
            let got = block_on(detect_skills(on, &all));
            let got = json!({ "skills": got.skills, "refused": got.refused }).to_string();
            assert_eq!(got, case["out"].to_string().replace("<dir>", &root), "{what}");
            if let (Some(f), false) = (&fake, case["calls"].is_null()) {
                assert_eq!(json!(f.calls()), case["calls"], "calls of {what}");
            }
        }
    }
}
