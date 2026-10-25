// SPDX-License-Identifier: AGPL-3.0-only
//! place-word.ts has no TS test of its own; every expected value here is what packages/protocol/src/place-word.ts
//! returned under node at a3b870a6e.

mod support;

use wsp_words::place_word::*;

fn place(id: &str, name: &str, kind: Option<&str>) -> PlaceRef {
    PlaceRef { id: id.into(), name: name.into(), kind: kind.map(String::from) }
}

/// The record as JSON.stringify writes it: keys in the TS order, keyConsole and subThreads only where set.
fn js_words(words: Option<ProviderKeyWords>) -> String {
    let Some(w) = words else { return "null".into() };
    let mut v = serde_json::json!({ "name": w.name, "keyName": w.key_name });
    if let Some(c) = w.key_console {
        v["keyConsole"] = serde_json::json!(c);
    }
    if w.sub_threads {
        v["subThreads"] = serde_json::json!(true);
    }
    serde_json::to_string(&v).unwrap()
}

fn js_halves(h: Option<RefusalHalves>) -> String {
    match h {
        None => "null".into(),
        Some(h) => serde_json::to_string(&serde_json::json!({ "happened": h.happened, "fix": h.fix })).unwrap(),
    }
}

// Cases the spec pins; every expected value is what the TS function returned under node at a3b870a6e.
mod pinned {
    use super::*;

    #[test]
    fn a_provider_is_named_by_its_key_words() {
        assert_eq!(js_words(provider_key_words("box")), "{\"name\":\"Boat\",\"keyName\":\"Boat API key\",\"keyConsole\":\"boat.dev/dashboard\",\"subThreads\":true}"); // JSON.stringify(PROVIDER_KEY_WORDS["box"] ?? null)
        assert_eq!(js_words(provider_key_words("solari")), "{\"name\":\"Solari\",\"keyName\":\"Solari API key\",\"keyConsole\":\"console.getsolari.com\"}"); // JSON.stringify(PROVIDER_KEY_WORDS["solari"] ?? null)
        assert_eq!(js_words(provider_key_words("Boat")), "null"); // JSON.stringify(PROVIDER_KEY_WORDS["Boat"] ?? null)
        assert_eq!(js_words(provider_key_words("")), "null"); // JSON.stringify(PROVIDER_KEY_WORDS[""] ?? null)
        assert_eq!(js_words(provider_key_words("x")), "null"); // JSON.stringify(PROVIDER_KEY_WORDS["x"] ?? null)
        assert_eq!(provider_key_name("box"), "Boat"); // providerKeyName("box")
        assert_eq!(provider_key_name("solari"), "Solari"); // providerKeyName("solari")
        assert_eq!(provider_key_name("here"), "here"); // providerKeyName("here")
        assert_eq!(provider_key_name(""), ""); // providerKeyName("")
        assert_eq!(sub_threads_only("box"), true); // subThreadsOnly("box")
        assert_eq!(sub_threads_only("solari"), false); // subThreadsOnly("solari")
        assert_eq!(sub_threads_only("here"), false); // subThreadsOnly("here")
        assert_eq!(place_said(&place("p1", "box", Some("provider"))), "Boat"); // placeSaid({"id":"p1","name":"box","kind":"provider"})
        assert_eq!(place_said(&place("p2", "spoo", None)), "spoo"); // placeSaid({"id":"p2","name":"spoo"})
        assert_eq!(place_said(&place("p3", "solari", Some("provider"))), "Solari"); // placeSaid({"id":"p3","name":"solari","kind":"provider"})
        assert_eq!(place_said(&place("p4", "weird", Some("provider"))), "weird"); // placeSaid({"id":"p4","name":"weird","kind":"provider"})
        assert_eq!(cloud_named(&["box","solari"] as &[&str], "BOAT"), Some("box".to_string())); // cloudNamed(["box","solari"], "BOAT")
        assert_eq!(cloud_named(&["box","solari"] as &[&str], "Solari"), Some("solari".to_string())); // cloudNamed(["box","solari"], "Solari")
        assert_eq!(cloud_named(&["box"] as &[&str], "nope"), None); // cloudNamed(["box"], "nope")
        assert_eq!(cloud_named(&[] as &[&str], "box"), None); // cloudNamed([], "box")
        assert_eq!(cloud_named(&["box","box"] as &[&str], "box"), Some("box".to_string())); // cloudNamed(["box","box"], "box")
    }

    #[test]
    fn a_word_names_a_place_by_id_or_name_and_a_provider_in_any_case() {
        assert_eq!(names_place(&place("p1", "spoo", None), "spoo"), true); // namesPlace({"id":"p1","name":"spoo"}, "spoo")
        assert_eq!(names_place(&place("p1", "spoo", None), "SPOO"), false); // namesPlace({"id":"p1","name":"spoo"}, "SPOO")
        assert_eq!(names_place(&place("p1", "spoo", None), "p1"), true); // namesPlace({"id":"p1","name":"spoo"}, "p1")
        assert_eq!(names_place(&place("box", "box", Some("provider")), "boat"), true); // namesPlace({"id":"box","name":"box","kind":"provider"}, "boat")
        assert_eq!(names_place(&place("box", "box", Some("provider")), "BOX"), true); // namesPlace({"id":"box","name":"box","kind":"provider"}, "BOX")
        assert_eq!(names_place(&place("x", "y", Some("provider")), "Y"), true); // namesPlace({"id":"x","name":"y","kind":"provider"}, "Y")
        assert_eq!(names_place(&place("x", "y", Some("computer")), "Y"), false); // namesPlace({"id":"x","name":"y","kind":"computer"}, "Y")
        assert_eq!(names_place(&place("x", "\u{130}", Some("provider")), "i\u{307}"), true); // namesPlace({"id":"x","name":"İ","kind":"provider"}, "i̇")
    }

    #[test]
    fn renames_and_logins_are_refused_in_two_halves() {
        assert_eq!(js_halves(place_rename_refusal(&place("here", "This Mac", None), "x", &[])), "{\"happened\":\"This Mac keeps its own name\",\"fix\":\"Rename one of the computers you added instead.\"}"); // JSON.stringify(placeRenameRefusal({id:"here",name:"This Mac"}, "x", []) ?? null)
        assert_eq!(js_halves(place_rename_refusal(&place("box", "box", Some("provider")), "x", &[])), "{\"happened\":\"box keeps its own name\",\"fix\":\"Rename one of the computers you added instead.\"}"); // JSON.stringify(placeRenameRefusal({id:"box",name:"box",kind:"provider"}, "x", []) ?? null)
        assert_eq!(js_halves(place_rename_refusal(&place("p1", "a", None), "", &[])), "{\"happened\":\"a computer's name cannot be blank\",\"fix\":\"Give it a name with a letter or a digit in it.\"}"); // JSON.stringify(placeRenameRefusal({id:"p1",name:"a"}, "", []) ?? null)
        assert_eq!(js_halves(place_rename_refusal(&place("p1", "a", None), "b", &[place("p1", "b", None), place("p2", "c", None)])), "null"); // JSON.stringify(placeRenameRefusal({id:"p1",name:"a"}, "b", [{id:"p1",name:"b"},{id:"p2",name:"c"}]) ?? null)
        assert_eq!(js_halves(place_rename_refusal(&place("p1", "a", None), "c", &[place("p2", "c", None)])), "{\"happened\":\"c is already the name of another computer\",\"fix\":\"Pick a name no other computer goes by.\"}"); // JSON.stringify(placeRenameRefusal({id:"p1",name:"a"}, "c", [{id:"p2",name:"c"}]) ?? null)
        assert_eq!(js_halves(place_rename_refusal(&place("p1", "a", None), "boat", &[place("box", "box", Some("provider"))])), "{\"happened\":\"boat is already the name of another computer\",\"fix\":\"Pick a name no other computer goes by.\"}"); // JSON.stringify(placeRenameRefusal({id:"p1",name:"a"}, "boat", [{id:"box",name:"box",kind:"provider"}]) ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "")), "{\"happened\":\"an ssh login cannot be blank\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "root@h")), "null"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "root@h") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "root@10.0.0.1")), "null"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "root@10.0.0.1") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "u.s-e_r@h-o.st:22")), "null"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "u.s-e_r@h-o.st:22") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "root@[::1]")), "null"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "root@[::1]") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "root@")), "{\"happened\":\"root@ is not an ssh login like root@host\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "root@") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "@h")), "{\"happened\":\"@h is not an ssh login like root@host\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "@h") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "root@h h")), "{\"happened\":\"root@h h is not an ssh login like root@host\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "root@h h") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "r\u{e9}@h")), "{\"happened\":\"r\u{e9}@h is not an ssh login like root@host\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "ré@h") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("p1", "a", None), "a@b@c")), "{\"happened\":\"a@b@c is not an ssh login like root@host\",\"fix\":\"Write it as the user and the host ssh logs in with, root@hetzner for one.\"}"); // JSON.stringify(placeSshRefusal({id:"p1",name:"a"}, "a@b@c") ?? null)
        assert_eq!(js_halves(place_ssh_refusal(&place("here", "h", None), "root@h")), "{\"happened\":\"h is reached over no ssh login of yours\",\"fix\":\"Give a new login to one of the computers you added instead.\"}"); // JSON.stringify(placeSshRefusal({id:"here",name:"h"}, "root@h") ?? null)
        assert_eq!(js_halves(Some(place_ssh_other_refusal("root@h", "spoo"))), "{\"happened\":\"root@h reaches a computer that is not spoo, so nothing was saved\",\"fix\":\"Give the login that reaches spoo itself.\"}"); // JSON.stringify(placeSshOtherRefusal("root@h", "spoo") ?? null)
        assert_eq!(js_halves(Some(place_ssh_unchecked_refusal("root@h", "spoo", "timeout"))), "{\"happened\":\"root@h could not be checked as spoo, so nothing was saved: timeout\",\"fix\":\"Check that ssh root@h logs in as root, or through a sudo that asks no password, and try again.\"}"); // JSON.stringify(placeSshUncheckedRefusal("root@h", "spoo", "timeout") ?? null)
        assert_eq!(place_login_elsewhere("root@h", "spoo", None), "root@h no longer reaches the computer added here as spoo"); // placeLoginElsewhere("root@h", "spoo")
        assert_eq!(place_login_elsewhere("root@h", "spoo", Some("lab")), "root@h reaches the other computer added here as lab"); // placeLoginElsewhere("root@h", "spoo", "lab")
        assert_eq!(js_halves(Some(place_login_other_refusal("root@h", &place("p1", "spoo", None), None))), "{\"happened\":\"root@h no longer reaches the computer added here as spoo, so nothing was changed there\",\"fix\":\"Give spoo the login that reaches it with wsp computers set spoo --ssh <user@host>, then try again.\"}"); // JSON.stringify(placeLoginOtherRefusal("root@h", {id:"p1",name:"spoo"}) ?? null)
        assert_eq!(js_halves(Some(place_login_other_refusal("root@h", &place("p1", "spoo", None), Some("lab")))), "{\"happened\":\"root@h reaches the other computer added here as lab, so nothing was changed there\",\"fix\":\"This record of spoo stands beside that one; take it away with wsp remove p1.\"}"); // JSON.stringify(placeLoginOtherRefusal("root@h", {id:"p1",name:"spoo"}, "lab") ?? null)
        assert_eq!(js_halves(Some(place_login_unchecked_refusal("root@h", "spoo", "why"))), "{\"happened\":\"root@h could not be checked as spoo, so nothing was changed there: why\",\"fix\":\"Check that ssh root@h logs in as root, or through a sudo that takes the password you typed, and try again.\"}"); // JSON.stringify(placeLoginUncheckedRefusal("root@h", "spoo", "why") ?? null)
        assert_eq!(place_login_elsewhere_removed_line("root@h", "spoo", None), "root@h no longer reaches the computer added here as spoo, so nothing was changed on the machine it reaches; spoo is off this host, and whatever wsp left on spoo itself stays until wsp leave runs there"); // placeLoginElsewhereRemovedLine("root@h", "spoo")
        assert_eq!(place_login_elsewhere_removed_line("root@h", "spoo", Some("lab")), "root@h reaches the other computer added here as lab, which stays added, so nothing was changed there; this record of spoo is off this host"); // placeLoginElsewhereRemovedLine("root@h", "spoo", "lab")
    }
}

mod place_names {
    use serde_json::{json, Value};
    use wsp_wire::views::place::PlaceView;
    use wsp_wire::views::workspace::WorkspaceView;
    use wsp_words::place_name::*;

    fn pv(v: Value) -> PlaceView {
        serde_json::from_value(crate::support::over(crate::support::place_view(), v)).unwrap()
    }

    fn pvs(v: Value) -> Vec<PlaceView> {
        v.as_array().unwrap().iter().map(|p| pv(p.clone())).collect()
    }

    fn wv(v: Value) -> WorkspaceView {
        serde_json::from_value(crate::support::over(crate::support::workspace_view(), v)).unwrap()
    }

    #[test]
    fn a_place_is_named_by_its_label_else_its_provider_words() {
        assert_eq!(is_here(None), false); // isHere(undefined)
        assert_eq!(is_here(Some(&pv(json!({"id":"here","kind":"computer","name":"mac"})))), true); // isHere({"id":"here","kind":"computer","name":"mac"})
        assert_eq!(is_provider_place(&pv(json!({"id":"here","kind":"computer","name":"mac"}))), false); // isProviderPlace({"id":"here","kind":"computer","name":"mac"})
        assert_eq!(place_name(&pv(json!({"id":"here","kind":"computer","name":"mac"}))), "mac"); // placeName({"id":"here","kind":"computer","name":"mac"})
        assert_eq!(is_here(Some(&pv(json!({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})))), false); // isHere({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})
        assert_eq!(is_provider_place(&pv(json!({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"}))), false); // isProviderPlace({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})
        assert_eq!(place_name(&pv(json!({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"}))), "Spoo box"); // placeName({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})
        assert_eq!(is_here(Some(&pv(json!({"id":"box","kind":"provider","name":"box"})))), false); // isHere({"id":"box","kind":"provider","name":"box"})
        assert_eq!(is_provider_place(&pv(json!({"id":"box","kind":"provider","name":"box"}))), true); // isProviderPlace({"id":"box","kind":"provider","name":"box"})
        assert_eq!(place_name(&pv(json!({"id":"box","kind":"provider","name":"box"}))), "Boat"); // placeName({"id":"box","kind":"provider","name":"box"})
        assert_eq!(place_name(&pv(json!({"id":"p9","kind":"computer","name":"x","label":""}))), ""); // placeName({id:"p9",kind:"computer",name:"x",label:""})
        assert_eq!(table_name(&wsp_words::place_word::PlaceRef { id: "box".into(), name: "box".into(), kind: Some("provider".into()) }), "Boat"); // tableName({name:"box",kind:"provider"})
        assert_eq!(here_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}]))), "mac"); // hereName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])
        assert_eq!(here_name(&[]), ""); // hereName([])
    }

    #[test]
    fn a_workspace_without_a_place_is_brokered_on_a_cloud() {
        assert_eq!(is_brokered(&wv(json!({"kind":"cloud"})), None), true); // isBrokered({"kind":"cloud"}, undefined)
        assert_eq!(is_brokered(&wv(json!({"kind":"cloud","place":"box"})), Some(&pv(json!({"id":"box","kind":"provider","name":"box"})))), true); // isBrokered({"kind":"cloud","place":"box"}, {"id":"box","kind":"provider","name":"box"})
        assert_eq!(is_brokered(&wv(json!({"kind":"cloud","place":"p1"})), Some(&pv(json!({"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})))), false); // isBrokered({"kind":"cloud","place":"p1"}, {"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"})
        assert_eq!(is_brokered(&wv(json!({"kind":"cloud","place":"p1"})), None), false); // isBrokered({"kind":"cloud","place":"p1"}, undefined)
        assert_eq!(is_brokered(&wv(json!({"kind":"local"})), None), false); // isBrokered({"kind":"local"}, undefined)
        assert_eq!(is_brokered(&wv(json!({})), None), true); // isBrokered({}, undefined)
        assert_eq!(brokered_agent_refusal("codex", &["a computer you joined", "this computer"]).message, "codex does not run on a cloud machine yet: wsp keeps no key there. Use a computer you joined or this computer."); // brokeredAgentRefusal("codex", ["a computer you joined", "this computer"]).message
        assert_eq!(brokered_agent_refusal("codex", &[] as &[&str]).fix.unwrap_or_default(), "Use ."); // brokeredAgentRefusal("codex", []).fix
        assert_eq!(brokered_agent_refusal("codex", &["x"]).kind.map(|k| k.as_str().to_string()).unwrap_or_default(), "usage"); // brokeredAgentRefusal("codex", ["x"]).kind
        assert_eq!(codex_login_unread_refusal().message, "Codex on a cloud machine runs on the Codex login on your computer, and wsp read none just now. Check that Codex is signed in on your computer (codex login status), then try again."); // codexLoginUnreadRefusal().message
        assert_eq!(codex_login_unread_refusal().kind.map(|k| k.as_str().to_string()).unwrap_or_default(), "auth"); // codexLoginUnreadRefusal().kind
    }

    #[test]
    fn a_workspace_finds_its_place_by_id_then_kind_then_provider() {
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"place":"p1"}))).map(|p| p.id.clone()), Some("p1".to_string())); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"place":"p1"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"place":"p1"}))), "Spoo box"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"place":"p1"})
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"place":"nope"}))).map(|p| p.id.clone()), None); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"place":"nope"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"place":"nope"}))), "a provider"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"place":"nope"})
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"local"}))).map(|p| p.id.clone()), Some("here".to_string())); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"local"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"local"}))), "mac"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"local"})
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud","provider":"solari"}))).map(|p| p.id.clone()), Some("solari".to_string())); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud","provider":"solari"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud","provider":"solari"}))), "Solari"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud","provider":"solari"})
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud"}))).map(|p| p.id.clone()), Some("box".to_string())); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud"}))), "Boat"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud"})
        assert_eq!(place_of(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud","provider":"nope"}))).map(|p| p.id.clone()), None); // placeOf([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud","provider":"nope"})?.id
        assert_eq!(workspace_computer_name(&pvs(json!([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}])), &wv(json!({"kind":"cloud","provider":"nope"}))), "nope"); // workspaceComputerName([{"id":"p1","kind":"computer","name":"spoo","label":"Spoo box"},{"id":"here","kind":"computer","name":"mac"},{"id":"box","kind":"provider","name":"box"},{"id":"solari","kind":"provider","name":"solari"}], {"kind":"cloud","provider":"nope"})
    }
}
