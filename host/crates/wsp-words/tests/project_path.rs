// SPDX-License-Identifier: AGPL-3.0-only
//! project-path.ts has no test file of its own at a3b870a6e; every case below is pinned off the TS under node.

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    #[allow(unused_imports)] use wsp_words::project_path::*;

    use wsp_words::project_path::FolderMachine;

    fn js<T: serde::Serialize>(v: &T) -> String {
        wsp_words::exit::json_line(&serde_json::to_value(v).unwrap(), None)
    }

    #[test]
    fn a_path_inside_a_project_reads_through_its_other_names() {
        assert_eq!(wsp_words::project_path::under_project("/r", "/r"), true); // underProject("/r", "/r")
        assert_eq!(wsp_words::project_path::under_project("/r/a", "/r"), true); // underProject("/r/a", "/r")
        assert_eq!(wsp_words::project_path::under_project("/ra", "/r"), false); // underProject("/ra", "/r")
        assert_eq!(wsp_words::project_path::under_project("/", "/r"), false); // underProject("/", "/r")
        assert_eq!(wsp_words::project_path::under_project("/r/", "/r"), true); // underProject("/r/", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("src/a.ts", "/r", "/r"), Some("src/a.ts".to_string())); // repoPathOf("src/a.ts", "/r", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("/r/src/a.ts", "/x", "/r"), Some("src/a.ts".to_string())); // repoPathOf("/r/src/a.ts", "/x", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("../b/c", "/r/a", "/r"), Some("b/c".to_string())); // repoPathOf("../b/c", "/r/a", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("./x/../y", "/r", "/r"), Some("y".to_string())); // repoPathOf("./x/../y", "/r", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("/elsewhere/a", "/r", "/r"), None); // repoPathOf("/elsewhere/a", "/r", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("/r", "/r", "/r"), None); // repoPathOf("/r", "/r", "/r")
        assert_eq!(wsp_words::project_path::repo_path_of("a", "/private/tmp/r", "/tmp/r"), None); // repoPathOf("a", "/private/tmp/r", "/tmp/r")
        assert_eq!(wsp_words::project_path::repo_path_of("/tmp/r/a", "/tmp/r", "/private/tmp/r"), Some("a".to_string())); // repoPathOf("/tmp/r/a", "/tmp/r", "/private/tmp/r")
        assert_eq!(wsp_words::project_path::repo_path_of("..", "/r", "/r"), None); // repoPathOf("..", "/r", "/r")
        assert_eq!(wsp_words::project_path::worktree_of("/r/wt/a/b", &["/r","/r/wt/a"] as &[&str]), Some("/r/wt/a".to_string())); // worktreeOf("/r/wt/a/b", ["/r","/r/wt/a"])
        assert_eq!(wsp_words::project_path::worktree_of("/r", &["/r/wt/a"] as &[&str]), None); // worktreeOf("/r", ["/r/wt/a"])
        assert_eq!(wsp_words::project_path::worktree_of("/x", &[] as &[&str]), None); // worktreeOf("/x", [])
        assert_eq!(wsp_words::project_path::worktree_of("/tmp/r/sub", &["/private/tmp/r"] as &[&str]), Some("/private/tmp/r".to_string())); // worktreeOf("/tmp/r/sub", ["/private/tmp/r"])
        assert_eq!(wsp_words::project_path::worktree_of("/r/wt/ab", &["/r/wt/a","/r"] as &[&str]), Some("/r".to_string())); // worktreeOf("/r/wt/ab", ["/r/wt/a","/r"])
    }

    #[test]
    fn claude_keys_its_projects_per_utf16_unit() {
        assert_eq!(wsp_words::project_path::claude_project_key("/Users/me/my project"), "-Users-me-my-project"); // claudeProjectKey("/Users/me/my project")
        assert_eq!(wsp_words::project_path::claude_project_key("/root/wsp-vm/wt-2445-words"), "-root-wsp-vm-wt-2445-words"); // claudeProjectKey("/root/wsp-vm/wt-2445-words")
        assert_eq!(wsp_words::project_path::claude_project_key("\u{e9}/\u{1f600}"), "----"); // claudeProjectKey("é/😀")
        assert_eq!(wsp_words::project_path::claude_project_key(""), ""); // claudeProjectKey("")
        assert_eq!(wsp_words::project_path::claude_project_key("a.b_c"), "a-b-c"); // claudeProjectKey("a.b_c")
        assert_eq!(wsp_words::project_path::claude_memory_dir("/root/.claude", "-root-x"), "/root/.claude/projects/-root-x/memory"); // claudeMemoryDir("/root/.claude", "-root-x")
        assert_eq!(wsp_words::project_path::claude_memory_dir("/root/.claude//", "k"), "/root/.claude/projects/k/memory"); // claudeMemoryDir("/root/.claude//", "k")
        assert_eq!(wsp_words::project_path::claude_memory_dir("", "k"), "/projects/k/memory"); // claudeMemoryDir("", "k")
    }

    #[test]
    fn folder_names_slugs_and_aliases() {
        assert_eq!(wsp_words::project_path::folder_name("/a/b/"), "b"); // folderName("/a/b/")
        assert_eq!(wsp_words::project_path::folder_name("/a/b"), "b"); // folderName("/a/b")
        assert_eq!(wsp_words::project_path::folder_name("b"), "b"); // folderName("b")
        assert_eq!(wsp_words::project_path::folder_name("/"), ""); // folderName("/")
        assert_eq!(wsp_words::project_path::folder_name(""), ""); // folderName("")
        assert_eq!(wsp_words::project_path::folder_name("a//"), "a"); // folderName("a//")
        assert_eq!(wsp_words::project_path::folder_slug("My Project!"), "my-project"); // folderSlug("My Project!")
        assert_eq!(wsp_words::project_path::folder_slug("---"), "work"); // folderSlug("---")
        assert_eq!(wsp_words::project_path::folder_slug(""), "work"); // folderSlug("")
        assert_eq!(wsp_words::project_path::folder_slug("\u{dc}n\u{ef}code Name"), "n-code-name"); // folderSlug("Ünïcode Name")
        assert_eq!(wsp_words::project_path::folder_slug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bcd"), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"); // folderSlug("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bcd")
        assert_eq!(wsp_words::project_path::folder_slug("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"), "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"); // folderSlug("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")
        assert_eq!(wsp_words::project_path::folder_slug("  lead  trail  "), "lead-trail"); // folderSlug("  lead  trail  ")
        assert_eq!(wsp_words::project_path::ssh_alias("My Box"), "wsp-my-box"); // sshAlias("My Box")
        assert_eq!(wsp_words::project_path::ssh_alias(""), "wsp-work"); // sshAlias("")
        assert_eq!(wsp_words::project_path::parent_folder_name("/a/b/c"), "b"); // parentFolderName("/a/b/c")
        assert_eq!(wsp_words::project_path::parent_folder_name("/a/b/"), "a"); // parentFolderName("/a/b/")
        assert_eq!(wsp_words::project_path::parent_folder_name("c"), ""); // parentFolderName("c")
        assert_eq!(wsp_words::project_path::parent_folder_name("/c"), ""); // parentFolderName("/c")
        assert_eq!(wsp_words::project_path::parent_folder_name(""), ""); // parentFolderName("")
    }

    #[test]
    fn a_mac_hides_and_guards_only_its_home_folders() {
        assert_eq!(wsp_words::project_path::hidden_folder("/h/.git", None), true); // hiddenFolder("/h/.git")
        assert_eq!(wsp_words::project_path::hidden_folder("/h/.cache/", None), true); // hiddenFolder("/h/.cache/")
        assert_eq!(wsp_words::project_path::hidden_folder("/h/Library", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), true); // hiddenFolder("/h/Library", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/Library", Some(&FolderMachine { home: Some("/h/".into()), mac: Some(true) })), true); // hiddenFolder("/h/Library", {"home":"/h/","mac":true})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/Library", Some(&FolderMachine { home: Some("/h".into()), mac: Some(false) })), false); // hiddenFolder("/h/Library", {"home":"/h","mac":false})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/Library", Some(&FolderMachine { home: None, mac: Some(true) })), false); // hiddenFolder("/h/Library", {"home":null,"mac":true})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/Library", Some(&FolderMachine { home: Some("".into()), mac: Some(true) })), false); // hiddenFolder("/h/Library", {"home":"","mac":true})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/w/Library", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), false); // hiddenFolder("/h/w/Library", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::hidden_folder("/h/src", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), false); // hiddenFolder("/h/src", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Desktop", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), true); // guardedFolder("/h/Desktop", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Desktop/", Some(&FolderMachine { home: Some("/h/".into()), mac: Some(true) })), true); // guardedFolder("/h/Desktop/", {"home":"/h/","mac":true})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Library/CloudStorage", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), true); // guardedFolder("/h/Library/CloudStorage", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Desktop", Some(&FolderMachine { home: Some("/h".into()), mac: None })), false); // guardedFolder("/h/Desktop", {"home":"/h"})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Desktop", None), false); // guardedFolder("/h/Desktop")
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Desktop/x", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), false); // guardedFolder("/h/Desktop/x", {"home":"/h","mac":true})
        assert_eq!(wsp_words::project_path::guarded_folder("/h/Library", Some(&FolderMachine { home: Some("/h".into()), mac: Some(true) })), false); // guardedFolder("/h/Library", {"home":"/h","mac":true})
    }

    #[test]
    fn the_paths_wsp_keeps_on_a_joined_computer() {
        assert_eq!(wsp_words::project_path::probe_path("/root"), "/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/usr/sbin:/usr/bin:/sbin:/bin"); // probePath("/root")
        assert_eq!(wsp_words::project_path::probe_path("/home/u/"), "/root/.local/bin:/usr/local/sbin:/usr/local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/usr/sbin:/usr/bin:/sbin:/bin"); // probePath("/home/u/")
        assert_eq!(wsp_words::project_path::probe_path("/usr"), "/root/.local/bin:/home/linuxbrew/.linuxbrew/bin:/home/linuxbrew/.linuxbrew/sbin:/root/go/bin:/root/.cargo/bin:/root/.local/share/pnpm:/root/.bun/bin:/sbin:/bin"); // probePath("/usr")
        assert_eq!(wsp_words::project_path::roots_path_in("/h"), "/h/.wsp/roots"); // rootsPathIn("/h")
        assert_eq!(wsp_words::project_path::work_folder_in("/h"), "/h/wsp-work"); // workFolderIn("/h")
        assert_eq!(js(&wsp_words::project_path::place_daemon_paths("/h")), "{\"wsp\":\"/h/.wsp\",\"dir\":\"/h/.wsp/daemon\",\"bundle\":\"/h/.wsp/daemon.tgz\",\"inbox\":\"/h/.wsp/inbox\",\"tokenPath\":\"/h/.wsp/daemon-token\",\"portFile\":\"/h/.wsp/daemon.port\",\"runDir\":\"/h/.wsp/run\",\"putDir\":\"/h/.wsp/put\",\"openSocket\":\"/h/.wsp/open.sock\",\"guestSocket\":\"/h/.wsp/daemon.sock\",\"guestBin\":\"/h/.wsp/place-bin\",\"dockerNotes\":\"/h/.wsp/docker-notes\",\"manifestPath\":\"/h/.wsp/manifest.json\",\"profileFile\":\"/h/.wsp/profile.sh\",\"unitDir\":\"/h/.config/systemd/user\",\"binDir\":\"/h/.local/bin\",\"openShim\":\"/h/.local/bin/wsp-open\",\"rootsPath\":\"/h/.wsp/roots\",\"placeFile\":\"/h/.wsp/place.json\",\"placeKey\":\"/h/.wsp/place-key.pem\",\"placeLog\":\"/h/.wsp/place.log\",\"placeFound\":\"/h/.wsp/place-found\",\"placeFoundPart\":\"/h/.wsp/place-found.part\",\"joinCode\":\"/h/.wsp/join-code\",\"installLogs\":\"/h/.wsp/install-logs\",\"readings\":\"/h/.wsp/readings\",\"sshDir\":\"/h/.wsp/ssh\"}"); // JSON.stringify(placeDaemonPaths("/h"))
        assert_eq!(js(&wsp_words::project_path::place_login_files("/h")), "[\"/h/.profile\",\"/h/.bash_profile\",\"/h/.bash_login\"]"); // JSON.stringify(placeLoginFiles("/h"))
        assert_eq!(js(&wsp_words::project_path::place_provision_paths("/h")), "{\"dir\":\"/h/.wsp/provision\",\"runDir\":\"/h/.wsp/provision/run\",\"log\":\"/h/.wsp/provision/log\",\"result\":\"/h/.wsp/provision/result.json\",\"staging\":\"/h/.wsp/provision/files\",\"landed\":\"/h/.wsp/provision/landed\",\"landing\":\"/h/.wsp/provision/landing\",\"asked\":\"/h/.wsp/provision/asked\"}"); // JSON.stringify(placeProvisionPaths("/h"))
        assert_eq!(js(&wsp_words::project_path::place_owned_paths("/h")), "[\"/h/.wsp/place.json\",\"/h/.wsp/place-key.pem\",\"/h/.wsp/place.log\",\"/h/.wsp/place-found\",\"/h/.wsp/place-found.part\",\"/h/.wsp/join-code\",\"/h/.wsp/daemon\",\"/h/.wsp/provision\",\"/h/.wsp/daemon.tgz\",\"/h/.wsp/inbox\",\"/h/.wsp/daemon-token\",\"/h/.wsp/roots\",\"/h/.wsp/profile.sh\",\"/h/.wsp/open.sock\",\"/h/.wsp/daemon.sock\",\"/h/.wsp/place-bin/wsp\",\"/h/.wsp/place-bin/docker\",\"/h/.wsp/place-bin\",\"/h/.wsp/docker-notes\",\"/h/.wsp/run\",\"/h/.wsp/daemon.port\",\"/h/.wsp/manifest.json\",\"/h/.wsp/put\",\"/h/.wsp/install-logs\",\"/h/.wsp/readings\",\"/h/.wsp/ssh\",\"/h/.local/bin/wsp-open\",\"/h/.local/bin/xdg-open\"]"); // JSON.stringify(placeOwnedPaths("/h"))
        assert_eq!(wsp_words::project_path::roots_path_in("/h/"), "/h/.wsp/roots"); // rootsPathIn("/h/")
        assert_eq!(wsp_words::project_path::work_folder_in("/h/"), "/h/wsp-work"); // workFolderIn("/h/")
        assert_eq!(js(&wsp_words::project_path::place_daemon_paths("/h/")), "{\"wsp\":\"/h/.wsp\",\"dir\":\"/h/.wsp/daemon\",\"bundle\":\"/h/.wsp/daemon.tgz\",\"inbox\":\"/h/.wsp/inbox\",\"tokenPath\":\"/h/.wsp/daemon-token\",\"portFile\":\"/h/.wsp/daemon.port\",\"runDir\":\"/h/.wsp/run\",\"putDir\":\"/h/.wsp/put\",\"openSocket\":\"/h/.wsp/open.sock\",\"guestSocket\":\"/h/.wsp/daemon.sock\",\"guestBin\":\"/h/.wsp/place-bin\",\"dockerNotes\":\"/h/.wsp/docker-notes\",\"manifestPath\":\"/h/.wsp/manifest.json\",\"profileFile\":\"/h/.wsp/profile.sh\",\"unitDir\":\"/h/.config/systemd/user\",\"binDir\":\"/h/.local/bin\",\"openShim\":\"/h/.local/bin/wsp-open\",\"rootsPath\":\"/h/.wsp/roots\",\"placeFile\":\"/h/.wsp/place.json\",\"placeKey\":\"/h/.wsp/place-key.pem\",\"placeLog\":\"/h/.wsp/place.log\",\"placeFound\":\"/h/.wsp/place-found\",\"placeFoundPart\":\"/h/.wsp/place-found.part\",\"joinCode\":\"/h/.wsp/join-code\",\"installLogs\":\"/h/.wsp/install-logs\",\"readings\":\"/h/.wsp/readings\",\"sshDir\":\"/h/.wsp/ssh\"}"); // JSON.stringify(placeDaemonPaths("/h/"))
        assert_eq!(js(&wsp_words::project_path::place_login_files("/h/")), "[\"/h/.profile\",\"/h/.bash_profile\",\"/h/.bash_login\"]"); // JSON.stringify(placeLoginFiles("/h/"))
        assert_eq!(js(&wsp_words::project_path::place_provision_paths("/h/")), "{\"dir\":\"/h/.wsp/provision\",\"runDir\":\"/h/.wsp/provision/run\",\"log\":\"/h/.wsp/provision/log\",\"result\":\"/h/.wsp/provision/result.json\",\"staging\":\"/h/.wsp/provision/files\",\"landed\":\"/h/.wsp/provision/landed\",\"landing\":\"/h/.wsp/provision/landing\",\"asked\":\"/h/.wsp/provision/asked\"}"); // JSON.stringify(placeProvisionPaths("/h/"))
        assert_eq!(js(&wsp_words::project_path::place_owned_paths("/h/")), "[\"/h/.wsp/place.json\",\"/h/.wsp/place-key.pem\",\"/h/.wsp/place.log\",\"/h/.wsp/place-found\",\"/h/.wsp/place-found.part\",\"/h/.wsp/join-code\",\"/h/.wsp/daemon\",\"/h/.wsp/provision\",\"/h/.wsp/daemon.tgz\",\"/h/.wsp/inbox\",\"/h/.wsp/daemon-token\",\"/h/.wsp/roots\",\"/h/.wsp/profile.sh\",\"/h/.wsp/open.sock\",\"/h/.wsp/daemon.sock\",\"/h/.wsp/place-bin/wsp\",\"/h/.wsp/place-bin/docker\",\"/h/.wsp/place-bin\",\"/h/.wsp/docker-notes\",\"/h/.wsp/run\",\"/h/.wsp/daemon.port\",\"/h/.wsp/manifest.json\",\"/h/.wsp/put\",\"/h/.wsp/install-logs\",\"/h/.wsp/readings\",\"/h/.wsp/ssh\",\"/h/.local/bin/wsp-open\",\"/h/.local/bin/xdg-open\"]"); // JSON.stringify(placeOwnedPaths("/h/"))
        assert_eq!(wsp_words::project_path::place_install_log("/h", "p1"), "/h/.wsp/install-logs/p1.log"); // placeInstallLog("/h", "p1")
        assert_eq!(wsp_words::project_path::held_place_script("/h"), "echo; echo WSP_PLACE_HELD\nif [ -e '/h/.wsp/place.json' ]; then { [ -f '/h/.wsp/place.json' ] && [ ! -s '/h/.wsp/place.json' ] && echo 'WSP_PLACE_LEFT file'; head -c 65536 '/h/.wsp/place.json' 2>/dev/null || echo 'WSP_PLACE_LEFT file'; }\nelif [ -L '/h/.wsp/place.json' ]; then echo 'WSP_PLACE_LEFT file'\nelif [ -e '/h/.wsp/place-key.pem' ] || [ -L '/h/.wsp/place-key.pem' ]; then echo 'WSP_PLACE_LEFT key'; fi; true"); // heldPlaceScript("/h")
        assert_eq!(wsp_words::project_path::held_place_script("/home/it's me"), "echo; echo WSP_PLACE_HELD\nif [ -e '/home/it'\\''s me/.wsp/place.json' ]; then { [ -f '/home/it'\\''s me/.wsp/place.json' ] && [ ! -s '/home/it'\\''s me/.wsp/place.json' ] && echo 'WSP_PLACE_LEFT file'; head -c 65536 '/home/it'\\''s me/.wsp/place.json' 2>/dev/null || echo 'WSP_PLACE_LEFT file'; }\nelif [ -L '/home/it'\\''s me/.wsp/place.json' ]; then echo 'WSP_PLACE_LEFT file'\nelif [ -e '/home/it'\\''s me/.wsp/place-key.pem' ] || [ -L '/home/it'\\''s me/.wsp/place-key.pem' ]; then echo 'WSP_PLACE_LEFT key'; fi; true"); // heldPlaceScript("/home/it's me")
        assert_eq!(wsp_words::project_path::stand_in_records_path("/s"), "/s/records.json"); // standInRecordsPath("/s")
        assert_eq!(wsp_words::project_path::stand_in_machine_path("/s", "m1"), "/s/machines/m1"); // standInMachinePath("/s", "m1")
        assert_eq!(wsp_words::project_path::stand_in_records_path("/s/"), "/s/records.json"); // standInRecordsPath("/s/")
        assert_eq!(wsp_words::project_path::stand_in_machine_path("/s/", "m1"), "/s/machines/m1"); // standInMachinePath("/s/", "m1")
    }
}
