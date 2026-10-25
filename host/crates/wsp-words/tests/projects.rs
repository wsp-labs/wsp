// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/projects.test.ts, lines 1-278 (the zod parse assertions of lines 62-82 and
//! 109-111 are wsp-wire's and are left out).

#![allow(unused_imports)]

mod support;

use indexmap::IndexMap;
use serde_json::json;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::projects::{ADD_FORMS_LINE, MEMORY_KEPT_CLAUSE, REGISTERING_LINE};
use wsp_wire::views::workspace::{ProjectSource, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::project_path::{folder_name, guarded_folder, hidden_folder, FolderMachine};
use wsp_words::projects::*;
use wsp_words::words::computer::{is_mac_machine, Platform};
use wsp_words::workspace_state::kind_words;

fn folder_source(path: &str) -> ProjectSource {
    serde_json::from_value(json!({ "kind": "folder", "path": path })).unwrap()
}

fn git_source(url: &str) -> ProjectSource {
    serde_json::from_value(json!({ "kind": "git", "url": url })).unwrap()
}

fn machine(home: &str, mac: Option<bool>) -> FolderMachine {
    FolderMachine { home: Some(home.to_string()), mac }
}

mod what_one_word_to_wsp_add_names {
    use super::*;

    #[test]
    fn a_login_is_a_computer_a_url_or_a_repo_path_is_a_repo_a_path_is_a_folder_and_anyt() {
        assert_eq!(source_kind("root@spoo").unwrap(), SourceKind::Computer);
        assert_eq!(source_kind("root@178.156.161.168").unwrap(), SourceKind::Computer);
        assert_eq!(source_kind("git@github.com:spoo-me/frontend.git").unwrap(), SourceKind::Git);
        assert_eq!(source_kind("https://github.com/spoo-me/frontend").unwrap(), SourceKind::Git);
        assert_eq!(source_kind("spoo-me/frontend.git").unwrap(), SourceKind::Git);
        assert_eq!(source_kind("/Users/dev/wsp").unwrap(), SourceKind::Folder);
        assert_eq!(source_kind("~/spoo/spoo-landing").unwrap(), SourceKind::Folder);
        assert_eq!(source_kind("./frontend").unwrap(), SourceKind::Folder);
        // A path is a path first: a folder somebody called repo.git is theirs on this computer, not a url.
        assert_eq!(source_kind("/Users/me/repo.git").unwrap(), SourceKind::Folder);
        let e = source_kind("spoo").unwrap_err();
        assert!(e.message.contains(ADD_FORMS_LINE), "{}", e.message);
        // owner/repo is the word the host's own command line takes; the host in front of it names the other host.
        assert_eq!(source_kind("spoo-me/frontend").unwrap(), SourceKind::Github);
        assert_eq!(source_kind("gitlab.com/dev/thing").unwrap(), SourceKind::Gitlab);
        // The record keeps the word that command reads: no host in front of it and no .git after it.
        let gitlab: ProjectSource = serde_json::from_value(json!({ "kind": "gitlab", "repo": "dev/thing" })).unwrap();
        assert_eq!(project_source_of("gitlab.com/dev/thing.git", &SourceKind::Gitlab, None), gitlab);
        let github: ProjectSource = serde_json::from_value(json!({ "kind": "github", "repo": "spoo-me/frontend" })).unwrap();
        assert_eq!(project_source_of("spoo-me/frontend", &SourceKind::Github, None), github);
    }

    #[test]
    fn a_project_is_named_by_its_repo_s_last_word_without_git_or_by_the_folder_s_own_na() {
        assert_eq!(project_name_of(&git_source("https://github.com/spoo-me/frontend.git")), "frontend");
        assert_eq!(project_name_of(&git_source("https://github.com/spoo-me/frontend")), "frontend");
        assert_eq!(project_name_of(&git_source("git@github.com:spoo-me/frontend.git")), "frontend");
        assert_eq!(project_name_of(&folder_source("/Users/z/spoo/spoo-landing")), "spoo-landing");
        assert_eq!(project_name_of(&folder_source("/Users/z/spoo/spoo-landing/")), "spoo-landing");
    }

    #[test]
    fn the_checkout_sits_under_the_folder_the_landing_road_names_and_a_folder_of_yours() {
        assert_eq!(
            project_path_on(&git_source("https://github.com/spoo-me/frontend"), "spoo-landing", Some("/srv")).unwrap(),
            "/srv/spoo-landing"
        );
        assert_eq!(
            project_path_on(&git_source("https://github.com/spoo-me/frontend"), "spoo-landing", Some("/root")).unwrap(),
            "/root/spoo-landing"
        );
        // A folder is its own path whatever the road says, which is why the road that clones nothing names no folder.
        assert_eq!(
            project_path_on(&folder_source("/Users/z/spoo/spoo-landing"), "spoo-landing", None).unwrap(),
            "/Users/z/spoo/spoo-landing"
        );
        // And a repo on such a computer is a wiring fault, refused before any path is spelled for it.
        let e = project_path_on(&git_source("https://github.com/spoo-me/frontend"), "spoo-landing", None).unwrap_err();
        assert!(e.message.contains("holds no checkout of its own"), "{}", e.message);
    }

    #[test]
    fn which_sources_a_computer_takes_is_its_kind_s_own_row_so_no_road_decides_it_for_i() {
        // A computer that clones takes a repo any of the three ways it can be named, and a folder on this computer,
        // which it clones from that folder's own remote and seeds what git ignores onto. This computer clones a repo
        // into a folder the person names and works that folder.
        assert_eq!(kind_words(&WorkspaceKind::Local).project_sources.to_vec(), vec!["git", "github", "gitlab", "folder"]);
        assert_eq!(kind_words(&WorkspaceKind::Cloud).project_sources.to_vec(), vec!["git", "github", "gitlab"]);
        // Copying the folder beside itself is its own word: a computer that clones takes a folder as a source and
        // still holds a checkout of it, so the two cannot be read off one list.
        assert_eq!([kind_words(&WorkspaceKind::Local).copies_folder, kind_words(&WorkspaceKind::Cloud).copies_folder], [true, false]);
    }

    #[test]
    fn the_refusals_name_the_project_the_folder_and_the_computer() {
        assert_eq!(
            same_source_refusal("spoo-landing", "spoo"),
            "that source is already a project on spoo, spoo-landing; one source on one computer is one project"
        );
        assert_eq!(cwd_outside_line("~/elsewhere", "spoo"), "~/elsewhere is not in spoo or a worktree of it; wsp add it first");
        assert_eq!(no_branches_line("notes"), "notes is not a git repo, so it has no branches");
        assert_eq!(
            thread_opened_line("th_1", Some("spoo"), "~/.wsp/worktrees/pr_1/feat-x"),
            "thread th_1 on spoo in ~/.wsp/worktrees/pr_1/feat-x"
        );
        assert_eq!(thread_opened_line("th_1", None, "~/spoo"), "thread th_1 in ~/spoo");
    }
}

mod the_project_a_workspace_holds {
    use super::*;

    #[test]
    fn a_view_carries_the_machine_s_home_where_the_kind_knows_it_so_a_client_shortens_a() {
        assert_eq!(home_shortened("/root/spoo", Some("/root")), "~/spoo");
        assert_eq!(home_shortened("/root", Some("/root")), "~");
        assert_eq!(home_shortened("/rooted/spoo", Some("/root")), "/rooted/spoo");
        assert_eq!(home_shortened("/root/spoo", None), "/root/spoo");
    }
}

mod what_a_computer_is_called_in_a_row {
    use super::*;

    #[test]
    fn is_this_computer_s_own_word_for_the_computer_the_host_runs_on_and_the_name_this() {
        let named: IndexMap<String, String> = IndexMap::from([("pl_box".to_string(), "hetzner".to_string())]);
        assert_eq!(computer_named(HERE_PLACE_ID, Some(&named), &Platform::Darwin), "this Mac");
        assert_eq!(computer_named(HERE_PLACE_ID, Some(&named), &Platform::Linux), "this computer");
        assert_eq!(computer_named("pl_box", Some(&named), &Platform::Linux), "hetzner");
        // A caller that could not read the list says the id rather than inventing a name for it.
        assert_eq!(computer_named("pl_box", None, &Platform::Darwin), "pl_box");
    }

    #[test]
    fn the_line_a_recorded_project_answers_with_names_the_computer_the_same_way_and_on() {
        let project: ProjectView = serde_json::from_value(json!({
            "id": "pr_1",
            "name": "spoo-landing",
            "computer": "pl_box",
            "source": { "kind": "git", "url": "https://github.com/dev/spoo.git" },
            "path": "/root/spoo-landing",
            "remote": "https://github.com/dev/spoo.git",
            "defaultBranch": "main",
            "memoryKey": "-root-spoo-landing",
            "memoryDir": "/wsp/projects/pr_1/memory",
            "createdAt": "t",
        }))
        .unwrap();
        let hetzner: IndexMap<String, String> = IndexMap::from([("pl_box".to_string(), "hetzner".to_string())]);
        assert_eq!(
            added_project_line(&project, Some(&hetzner), &Platform::Darwin),
            "spoo-landing pr_1: https://github.com/dev/spoo.git on hetzner, at /root/spoo-landing"
        );
        // This computer's own word is the host's platform's, never a default: a line written on a Linux host says
        // this computer where a Mac says this Mac, and neither reads the other's word.
        let here: ProjectView = serde_json::from_value(support::over(
            serde_json::to_value(&project).unwrap(),
            json!({ "id": "pr_2", "name": "wsp", "computer": HERE_PLACE_ID, "source": { "kind": "folder", "path": "/Users/dev/wsp" }, "path": "/Users/dev/wsp" }),
        ))
        .unwrap();
        let none: IndexMap<String, String> = IndexMap::new();
        let on_mac = added_project_line(&here, Some(&none), &Platform::Darwin);
        assert!(on_mac.contains("on this Mac, at /Users/dev/wsp"), "{on_mac}");
        let on_linux = added_project_line(&here, Some(&none), &Platform::Linux);
        assert!(on_linux.contains("on this computer, at /Users/dev/wsp"), "{on_linux}");
        // A project here takes a thread at once, and the line says the one that opens it.
        assert_eq!(on_mac.split('\n').nth(1), Some(r#"open a thread on it with: wsp run 'wsp' "<message>""#));
        // The add's own stage says that one sentence and no other: a caller holding the computer's word already,
        // which is the road the landing takes, reads the same line off it.
        assert_eq!(added_project_on(&project, "hetzner"), added_project_line(&project, Some(&hetzner), &Platform::Darwin));
    }

    #[test]
    fn the_line_the_add_starts_with_says_what_is_being_recorded_and_from_where_and_coun() {
        let folder = folder_source("/Users/dev/spoo-landing");
        // A seed travels with it, so what travels is the seeding's own line in the menu's counts and this one says
        // none of it: a count of ticked files here read as the whole of a seed carrying a memory folder too.
        assert_eq!(adding_project_line("spoo-landing", &folder, true), "spoo-landing from /Users/dev/spoo-landing.");
        // Nothing of the person's folder goes with a repo the computer clones itself, which nothing else says.
        assert_eq!(
            adding_project_line("spoo-ts", &git_source("https://github.com/dev/spoo.git"), false),
            "spoo-ts from https://github.com/dev/spoo.git, nothing seeded."
        );
    }

    #[test]
    fn the_remove_on_a_computer_you_joined_leaves_the_project_s_folder_there_as_it_is_a() {
        // The clause has one home, which the tool describing the remove reads too.
        assert_eq!(MEMORY_KEPT_CLAUSE, "the memory its agent keeps on that computer stays");
        assert_eq!(
            project_left_on_computer_line("spoo-landing", "spoo", "/root/spoo-landing"),
            "spoo-landing is no longer a project on spoo; its folder /root/spoo-landing stays there as it is"
        );
    }
}

mod getting_a_project_onto_a_machine {
    use super::*;

    #[test]
    fn a_register_says_nothing_was_copied_and_the_flags_a_copy_takes_have_no_meaning_he() {
        assert_eq!(REGISTERING_LINE, "already on this computer, registering");
        assert_eq!(registered_line("/Users/dev/wsp"), "wsp registered at /Users/dev/wsp; nothing was copied.");
        assert_eq!(
            register_takes_no_consent_line(&["--keep"]),
            "--keep has no meaning on this computer: the folder is registered at its path and nothing is carried, cut or replaced"
        );
        assert_eq!(
            register_takes_no_consent_line(&["keep", "agents"]),
            "keep, agents have no meaning on this computer: the folder is registered at its path and nothing is carried, cut or replaced"
        );
        assert_eq!(folder_name("/Users/dev/wsp/"), "wsp");
        assert_eq!(folder_name("wsp"), "wsp");
    }

    #[test]
    fn a_folder_browser_hides_the_machine_s_own_folders_the_dot_named_ones_anywhere_and() {
        let mac = machine("/Users/dev", Some(true));
        // A dot-named folder is the machine's own wherever it sits, and with nothing known about the machine at all.
        assert!(hidden_folder("/Users/dev/.config", Some(&mac)));
        assert!(hidden_folder("/root/.wsp", None));
        assert!(!hidden_folder("/Users/dev/code", Some(&mac)));
        // The Library the Mac keeps in the home itself, which its Finder hides too.
        assert!(hidden_folder("/Users/dev/Library", Some(&mac)));
        assert!(hidden_folder("/Users/dev/Library/", Some(&mac)));
        // A home is wherever the login puts it, and a lab account's is not /Users/<name>: the home decides, not the
        // path's shape, or every persona home on a Mac gets the junk drawer back.
        assert!(hidden_folder("/Users/Shared/lab/priya/Library", Some(&machine("/Users/Shared/lab/priya", Some(true)))));
        assert!(hidden_folder("/var/folders/t/session/Library", Some(&machine("/var/folders/t/session", Some(true)))));
        // A Library somebody made inside their own work is theirs, and so is one on a machine that is not a Mac.
        assert!(!hidden_folder("/Users/dev/code/app/Library", Some(&mac)));
        assert!(!hidden_folder("/root/Library", Some(&machine("/root", Some(false)))));
        assert!(!hidden_folder("/Users/dev/Library", Some(&machine("/Users/dev", None))));
        assert!(!hidden_folder("/Users/dev/Library", None));
    }

    #[test]
    fn a_folder_browser_stays_out_of_the_folders_a_mac_guards_in_its_home_and_of_none_o() {
        let mac = machine("/Users/dev", Some(true));
        for name in ["Desktop", "Documents", "Downloads", "Pictures", "Movies", "Music", "Library/Mobile Documents", "Library/CloudStorage"]
        {
            assert!(guarded_folder(&format!("/Users/dev/{name}"), Some(&mac)));
            assert!(guarded_folder(&format!("/Users/dev/{name}/"), Some(&mac)));
        }
        // Inside one is somewhere the person went, and one inside their own work is theirs.
        assert!(!guarded_folder("/Users/dev/Documents/GitHub", Some(&mac)));
        assert!(!guarded_folder("/Users/dev/code/Documents", Some(&mac)));
        // Nothing else guards a home's Desktop.
        assert!(!guarded_folder("/home/dev/Desktop", Some(&machine("/home/dev", Some(false)))));
        assert!(!guarded_folder("/Users/dev/Desktop", None));
    }

    #[test]
    fn a_machine_says_it_is_a_mac_by_the_name_its_maker_gives_it_or_by_the_kernel_where() {
        assert!(is_mac_machine(Some("macOS 15.5")));
        assert!(is_mac_machine(Some("Darwin 25.4.0")));
        assert!(!is_mac_machine(Some("Ubuntu 24.04.1 LTS")));
        assert!(!is_mac_machine(Some("Debian GNU/Linux 12 (bookworm)")));
        // A machine that has answered nothing yet has said nothing to hide.
        assert!(!is_mac_machine(None));
        assert!(!is_mac_machine(Some("")));
    }
}

mod where_a_workspace_of_a_project_lands {
    use super::*;

    const HERE: (&str, &str) = (HERE_PLACE_ID, "zingzy-mbp");
    const SOLARI: (&str, &str) = ("solari", "Solari");
    const BOX: (&str, &str) = ("p_2", "hetzner");

    #[test]
    fn copies_a_folder_on_this_computer_for_a_project_here_forks_on_the_host_s_own_prov() {
        assert_eq!(workspace_lands(HERE_PLACE_ID, Some("solari")), WorkspaceLands::Here);
        assert_eq!(workspace_lands("solari", Some("solari")), WorkspaceLands::Wired);
        assert_eq!(workspace_lands("p_2", Some("solari")), WorkspaceLands::Place { place: "p_2".to_string() });
        assert_eq!(workspace_lands("solari", None), WorkspaceLands::Place { place: "solari".to_string() });
    }

    #[test]
    fn puts_a_project_here_on_this_computer_s_row_and_never_on_the_provider_the_host_fo() {
        assert!(lands_on(HERE_PLACE_ID, HERE.0, HERE.1));
        assert!(!lands_on(HERE_PLACE_ID, SOLARI.0, SOLARI.1));
        assert!(lands_on("solari", SOLARI.0, SOLARI.1));
        assert!(lands_on("Solari", SOLARI.0, SOLARI.1));
        assert!(lands_on("hetzner", BOX.0, BOX.1));
        assert!(!lands_on("p_2", SOLARI.0, SOLARI.1));
    }
}

mod a_repo_cloned_on_this_computer {
    use super::*;

    #[test]
    fn takes_an_https_url_an_ssh_url_git_s_scp_form_and_owner_repo_and_nothing_a_transp() {
        for word in [
            "https://github.com/spoo-me/spoo.me",
            "https://gitlab.example.com:8443/team/app.git",
            "ssh://git@github.com/spoo-me/spoo.me.git",
            "ssh://github.com/spoo-me/spoo.me.git",
            "ssh://deploy-bot.v2@host.example/srv/app.git",
            "ssh://git@host.example:2222/srv/app.git",
            "git@github.com:spoo-me/spoo.me.git",
            "spoo-me/spoo.me",
            "gitlab.com/team/app",
        ] {
            assert_eq!(clone_url_refusal(word), None, "{word}");
        }
        for word in [
            "file:///etc",
            "ext::sh -c touch% /tmp/x",
            "http://github.com/a/b",
            "git://github.com/a/b",
            "https://user:token@github.com/a/b",
            "https://-oProxyCommand=x/a/b",
            "ssh://-oProxyCommand=evil/x",
            "git@-oProxyCommand=x:a/b",
            "git@host:-x/y",
            "https://github.com/a/b --upload-pack=x",
            "-uhttps://github.com/a/b",
            "-a/b",
            "https://",
            "ssh://git@github.com",
            "ssh://-oProxyCommand=true@github.com/a/b",
            "ssh://git$(id)@github.com/a/b",
            "ssh://gi%60t@github.com/a/b",
            "ssh://a:b@github.com/x",
            "ssh://git@-host/a/b",
            "-git@github.com:a/b",
            "git$x@github.com:a/b",
        ] {
            assert_eq!(
                clone_url_refusal(word),
                Some(format!("{word} is not a repo address wsp clones; give its https or ssh url, or owner/repo")),
                "{word}"
            );
        }
    }

    #[test]
    fn says_git_s_own_last_line_when_a_clone_fails_and_how_to_sign_in_when_the_failure() {
        let sign_in = "; sign in with gh auth login, or use the repo's ssh url";
        assert_eq!(
            clone_failed_line("Cloning into '/x'...\nfatal: destination path '/x' already exists and is not an empty directory.\n"),
            "fatal: destination path '/x' already exists and is not an empty directory."
        );
        assert_eq!(
            clone_failed_line("Cloning into '/x'...\nfatal: could not read Username for 'https://github.com': terminal prompts disabled\n"),
            format!("fatal: could not read Username for 'https://github.com': terminal prompts disabled{sign_in}")
        );
        assert_eq!(
            clone_failed_line("remote: Repository not found.\nfatal: repository 'https://github.com/me/private.git/' not found\n"),
            format!("fatal: repository 'https://github.com/me/private.git/' not found{sign_in}")
        );
        assert_eq!(
            clone_failed_line("git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository.\n"),
            format!("fatal: Could not read from remote repository.{sign_in}")
        );
        assert_eq!(
            clone_failed_line("GraphQL: Could not resolve to a Repository with the name 'me/private'. (repository)\n"),
            format!("GraphQL: Could not resolve to a Repository with the name 'me/private'. (repository){sign_in}")
        );
        assert_eq!(clone_failed_line(""), "the clone failed and said nothing");
    }

    #[test]
    fn a_url_with_no_computer_names_both_roads_here_into_a_folder_or_a_computer_that_cl() {
        assert_eq!(
            no_computer_for_source_line("https://github.com/a/b", &["spoo"]),
            "https://github.com/a/b is a repo: clone it here with --into <folder>, or name the computer that clones it with --on spoo"
        );
        assert_eq!(no_computer_for_source_line("a/b", &[] as &[&str]), "a/b is a repo: clone it here with --into <folder>");
    }
}

mod what_a_new_worktree_ran_before_its_first_thread {
    use super::*;

    #[test]
    fn says_each_command_on_a_line_of_its_own_so_a_command_holding_a_separator_reads_as() {
        let after = r#"read answer; echo "got:$answer""#;
        let failed = WorktreeCommandEnd::Exited { exit_code: 1, said: String::new() };
        assert_eq!(
            worktree_setup_line(&[
                worktree_step_words("cargo fetch --locked", "daemon", None),
                worktree_step_words(after, ".", Some(&failed))
            ]),
            ["Before this thread started, its new worktree ran:", "- cargo fetch --locked in daemon", &format!("- {after} (exited 1)")]
                .join("\n")
        );
    }
}

mod clone_urls_parse_as_whatwg_does {
    use wsp_words::projects::clone_url_refusal;

    #[test]
    fn reads_the_host_after_idna_mapping_and_percent_decoding() {
        // node 22, new URL(w): hostname "xn--bcher-kva.example", "git.com", "host"; port 99999, "[::1]" and
        // "ex%2Fample.com" are refused by the URL parse or by the host pattern.
        for ok in ["https://bücher.example/a/b", "https://ＧＩＴ.com/x", "https://h%41st/x", "https://faß.de/x"] {
            assert_eq!(clone_url_refusal(ok), None, "{ok}");
        }
        for refused in ["https://git.example.com:99999/x", "https://[::1]/x", "https://ex%2Fample.com/x", "https://host/"] {
            assert_eq!(
                clone_url_refusal(refused),
                Some(format!("{refused} is not a repo address wsp clones; give its https or ssh url, or owner/repo")),
                "{refused}"
            );
        }
    }
}
