// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/app-address.test.ts whole (lines 1-142): the workspace, thread, subagent and fresh
//! screen an address opens on, the pairing code at its end, and the wsp:// link.

use wsp_words::app_address::*;

fn address(workspace_id: &str) -> AppAddress {
    AppAddress { workspace_id: workspace_id.to_string(), ..Default::default() }
}

fn thread(workspace_id: &str, thread_id: &str) -> AppAddress {
    AppAddress { thread_id: Some(thread_id.to_string()), ..address(workspace_id) }
}

fn subagent(workspace_id: &str, thread_id: &str, call: &str) -> AppAddress {
    AppAddress { subagent: Some(call.to_string()), ..thread(workspace_id, thread_id) }
}

fn fresh(workspace_id: &str) -> AppAddress {
    AppAddress { fresh: Some(true), ..address(workspace_id) }
}

fn target(kind: LinkKind, id: &str) -> LinkTarget {
    LinkTarget { kind, id: id.to_string() }
}

mod the_workspace_a_page_opens_on {
    use super::*;

    #[test]
    fn round_trips_an_id_through_the_hash() {
        assert_eq!(workspace_hash("ws_a1b2"), "#w/ws_a1b2");
        assert_eq!(app_hash(&address("ws_a1b2")), "#w/ws_a1b2");
        assert_eq!(address_from_hash(&workspace_hash("ws_a1b2")), Some(address("ws_a1b2")));
    }

    #[test]
    fn names_no_workspace_for_the_app_s_own_hashes_an_empty_one_or_no_hash_at_all() {
        assert_eq!(address_from_hash("#gallery"), None);
        assert_eq!(address_from_hash(""), None);
        assert_eq!(address_from_hash("#w/"), None);
    }
}

mod the_thread_a_page_opens_on {
    use super::*;

    #[test]
    fn round_trips_a_workspace_and_a_thread_and_the_workspace_reads_out_of_the_same_has() {
        assert_eq!(app_hash(&thread("ws_a1b2", "thr_9")), "#w/ws_a1b2/t/thr_9");
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9"), Some(thread("ws_a1b2", "thr_9")));
    }

    #[test]
    fn names_no_thread_for_a_workspace_hash_an_empty_thread_or_an_unrelated_hash() {
        assert_eq!(address_from_hash("#w/ws_a1b2").and_then(|a| a.thread_id), None);
        assert_eq!(address_from_hash("#w/ws_a1b2/t/"), Some(address("ws_a1b2")));
        assert_eq!(address_from_hash("#gallery"), None);
    }

    #[test]
    fn escapes_what_an_id_carries() {
        assert_eq!(address_from_hash(&app_hash(&thread("ws a", "t/1"))), Some(thread("ws a", "t/1")));
    }
}

mod the_subagent_a_page_opens_on {
    use super::*;

    #[test]
    fn round_trips_a_workspace_its_lead_thread_and_the_call_that_launched_the_subagent() {
        let a = subagent("ws_a1b2", "thr_9", "toolu_01Ab");
        assert_eq!(app_hash(&a), "#w/ws_a1b2/t/thr_9/a/toolu_01Ab");
        assert_eq!(address_from_hash(&app_hash(&a)), Some(a));
    }

    #[test]
    fn escapes_what_each_id_carries_and_names_no_subagent_where_the_hash_ends_at_the_th() {
        let odd = subagent("ws a", "t/a/1", "call/a/2");
        assert_eq!(address_from_hash(&app_hash(&odd)), Some(odd));
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9"), Some(thread("ws_a1b2", "thr_9")));
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9/a/"), Some(thread("ws_a1b2", "thr_9")));
    }

    #[test]
    fn is_written_only_under_a_thread_a_subagent_with_no_lead_named_is_the_workspace_al() {
        let a = AppAddress { subagent: Some("toolu_01Ab".to_string()), ..address("ws_a1b2") };
        assert_eq!(app_hash(&a), "#w/ws_a1b2");
    }

    #[test]
    fn keeps_the_pairing_code_off_what_it_names() {
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9/a/toolu_01Ab/c/7K3MQP2X"), Some(subagent("ws_a1b2", "thr_9", "toolu_01Ab")));
    }
}

mod the_screen_a_workspace_s_next_thread_is_written_on {
    use super::*;

    #[test]
    fn has_an_address_of_its_own_which_names_no_thread() {
        assert_eq!(app_hash(&fresh("ws_a1b2")), "#w/ws_a1b2/new");
        assert_eq!(address_from_hash("#w/ws_a1b2/new"), Some(fresh("ws_a1b2")));
    }

    #[test]
    fn is_not_read_into_a_workspace_whose_own_id_ends_that_way_and_a_thread_address_is() {
        assert_eq!(address_from_hash("#w/new"), Some(address("new")));
        assert_eq!(address_from_hash(&app_hash(&address("ws/new"))), Some(address("ws/new")));
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9/new"), Some(thread("ws_a1b2", "thr_9/new")));
    }
}

mod the_code_wsp_init_puts_in_the_address_of_the_page_it_opens {
    use super::*;

    fn code(code: &str, rest: &str) -> PairingCode {
        PairingCode { code: code.to_string(), rest: rest.to_string() }
    }

    #[test]
    fn rides_the_end_of_the_hash_beside_the_workspace_and_alone_when_there_is_no_worksp() {
        assert_eq!(pairing_code_of("#w/ws_a1b2/c/7K3MQP2X"), Some(code("7K3MQP2X", "#w/ws_a1b2")));
        assert_eq!(pairing_code_of("#c/7K3MQP2X"), Some(code("7K3MQP2X", "")));
    }

    #[test]
    fn is_not_part_of_what_the_address_names_so_the_workspace_and_the_thread_read_as_th() {
        assert_eq!(address_from_hash("#w/ws_a1b2/c/7K3MQP2X"), Some(address("ws_a1b2")));
        assert_eq!(address_from_hash("#w/ws_a1b2/t/thr_9/c/7K3MQP2X"), Some(thread("ws_a1b2", "thr_9")));
        assert_eq!(address_from_hash("#w/ws_a1b2/new/c/7K3MQP2X"), Some(fresh("ws_a1b2")));
        assert_eq!(address_from_hash("#c/7K3MQP2X"), None);
    }

    #[test]
    fn reads_no_code_off_a_hash_carrying_none_an_empty_one_or_the_app_s_own_hashes() {
        assert_eq!(pairing_code_of("#w/ws_a1b2"), None);
        assert_eq!(pairing_code_of("#w/ws_a1b2/c/"), None);
        assert_eq!(pairing_code_of("#c/"), None);
        assert_eq!(pairing_code_of("#gallery"), None);
        assert_eq!(pairing_code_of(""), None);
    }
}

mod a_wsp_link {
    use super::*;

    #[test]
    fn names_a_thread_a_workspace_a_project_or_a_settings_page_by_its_id_and_carries_it() {
        assert_eq!(address_from_link("wsp://thread/th_9f3a"), Some(target(LinkKind::Thread, "th_9f3a")));
        assert_eq!(address_from_link("wsp://workspace/ws_a1b2"), Some(target(LinkKind::Workspace, "ws_a1b2")));
        assert_eq!(address_from_link("wsp://project/pr_77c0"), Some(target(LinkKind::Project, "pr_77c0")));
        assert_eq!(address_from_link("wsp://settings/keybindings"), Some(target(LinkKind::Settings, "keybindings")));
        assert_eq!(address_from_link("wsp://settings/keybindings/"), Some(target(LinkKind::Settings, "keybindings")));
        assert_eq!(
            address_from_link("WSP://thread/0b6e1c2a-7f1d-4c55-9a9b-3f1f2a6c8d10"),
            Some(target(LinkKind::Thread, "0b6e1c2a-7f1d-4c55-9a9b-3f1f2a6c8d10"))
        );
        for t in [target(LinkKind::Thread, "th_9f3a"), target(LinkKind::Settings, "keybindings")] {
            assert_eq!(link_from_hash(&link_hash(&t)), Some(t));
        }
        assert_eq!(link_hash(&target(LinkKind::Thread, "th_9f3a")), "#open/thread/th_9f3a");
        // A link's hash is not a workspace address, and a workspace address is not a link.
        assert_eq!(address_from_hash(&link_hash(&target(LinkKind::Workspace, "ws_a"))), None);
        assert_eq!(link_from_hash(&workspace_hash("ws_a")), None);
    }

    #[test]
    fn names_nothing_for_any_other_scheme_route_or_shape_ids_alone_ride_a_link_never_an() {
        let long = format!("wsp://thread/{}", "a".repeat(129));
        for url in [
            "https://thread/th_1",
            "wsp:thread/th_1",
            "wsp://send/th_1",
            "wsp://thread/",
            "wsp://thread",
            "wsp://thread/th_1/extra",
            "wsp://thread/th_1?send=rm%20-rf",
            "wsp://thread/th_1#x",
            "wsp://user:pw@thread/th_1",
            "wsp://thread:9/th_1",
            "wsp://thread/th%201",
            "wsp://thread/..",
            "wsp://thread/%2e%2e",
            "wsp://thread/<script>",
            long.as_str(),
            "not a url",
            "",
        ] {
            assert_eq!(address_from_link(url), None, "{url}");
        }
        assert_eq!(link_from_hash("#open/send/th_1"), None);
        assert_eq!(link_from_hash("#open/thread/a%20b"), None);
    }
}
