// SPDX-License-Identifier: AGPL-3.0-only
// The sign-in rows, every status reader on the oracle's outputs, and everything derived from the rows, against
// tests/fixtures/signin.json, which the TS catalog wrote (oracle/dump.oracle.ts).
mod oracle_common;

use indexmap::IndexMap;
use oracle_common::{fixture, json, opt, s, same};
use serde_json::{json, Value};
use wsp_catalog::signin::*;
use wsp_catalog::signin_rows::*;

fn secrets() -> IndexMap<String, String> {
    IndexMap::from([("GEMINI_API_KEY".into(), "~/.zshrc".into()), ("OPENAI_API_KEY".into(), "~/.config/env".into()), ("ANTHROPIC_API_KEY".into(), "~/.zshrc".into())])
}

#[test]
fn every_row_is_the_ts_row_in_the_ts_order() {
    let want = fixture("signin.json")["rows"].clone();
    let keys: Vec<&String> = SIGN_IN_ROWS.keys().collect();
    let want_keys: Vec<&String> = want.as_object().unwrap().keys().collect();
    assert_eq!(keys, want_keys);
    for (k, row) in SIGN_IN_ROWS.iter() {
        same(k, json(row), &want[k.as_str()]);
        assert_eq!(sign_in_row(k), row);
    }
}

#[test]
fn every_status_reader_reads_every_output_as_the_ts_reader_does() {
    let w = fixture("signin.json")["readers"].clone();
    let empty = IndexMap::new();
    for (k, cases) in w.as_object().unwrap() {
        let status = sign_in_row(k).status().unwrap_or_else(|| panic!("{k} has a status in the TS"));
        for case in cases.as_array().unwrap() {
            let o = s(&case["output"]);
            let label = format!("{k} on {o:?}");
            same(&format!("{label} signedIn 0"), json(status.signed_in(o, 0)), &case["signedIn0"]);
            same(&format!("{label} signedIn 1"), json(status.signed_in(o, 1)), &case["signedIn1"]);
            same(&format!("{label} detail"), opt(status.detail(o, &secrets())), &case["detail"]);
            same(&format!("{label} detail with no secrets"), opt(status.detail(o, &empty)), &case["detailNoSecrets"]);
            same(&format!("{label} kind"), opt(status.kind(o)), &case["kind"]);
            same(&format!("{label} plan"), opt(status.plan(o)), &case["plan"]);
            assert_eq!(status.why(o), None, "{label}");
        }
    }
}

#[test]
fn every_rows_readers_answer_as_the_ts_readers_do() {
    let w = fixture("signin.json")["perRow"].clone();
    for (k, row) in SIGN_IN_ROWS.iter() {
        let want = &w[k.as_str()];
        same(&format!("{k} hasLogin"), json(has_login(row).is_some()), &want["hasLogin"]);
        same(&format!("{k} mintsToken"), json(mints_token(row).is_some()), &want["mintsToken"]);
        same(&format!("{k} loginThere"), opt(login_there(row)), &want["loginThere"]);
        same(&format!("{k} sharedLoginOf"), opt(shared_login_of(row)), &want["sharedLoginOf"]);
        same(&format!("{k} livesOnComputer"), json(lives_on_computer(row)), &want["livesOnComputer"]);
        same(&format!("{k} keyEnvOf"), opt(key_env_of(row)), &want["keyEnvOf"]);
        same(&format!("{k} questionsOf"), json(questions_of(row)), &want["questionsOf"]);
        same(&format!("{k} asksThePerson"), json(asks_the_person(row)), &want["asksThePerson"]);
        same(&format!("{k} signInRoadOf"), json(sign_in_road_of(row)), &want["signInRoadOf"]);
        same(&format!("{k} loginWords"), opt(has_login(row).map(login_words)), &want["loginWords"]);
        same(&format!("{k} signsInByDefault"), json(signs_in_by_default(row)), &want["signsInByDefault"]);
        same(&format!("{k} loginStatePaths"), json(login_state_paths(row)), &want["loginStatePaths"]);
    }
}

#[test]
fn login_and_keys_ids_and_the_keys_row_are_the_ts_ones() {
    let w = fixture("signin.json");
    for (id, want) in w["loginIdOf"].as_object().unwrap() {
        same(&format!("loginIdOf {id}"), json(login_id_of(id)), want);
    }
    for (id, want) in w["keysIdOf"].as_object().unwrap() {
        same(&format!("keysIdOf {id}"), json(keys_id_of(id)), want);
    }
    let hermes = KeyFiles { paths: vec!["~/.hermes/.env".into()], note: "n".into() };
    same("keysRowOf", json(keys_row_of(&hermes, sign_in_row("hermes").status())), &w["keysRowOf"]);
    let plain = KeyFiles { paths: vec!["~/.x".into()], note: "n".into() };
    same("keysRowOfNoStatus", json(keys_row_of(&plain, None)), &w["keysRowOfNoStatus"]);
    same("noSignIn", json(no_sign_in()), &w["noSignIn"]);
}

#[test]
fn the_shared_logins_and_every_path_list_are_the_ts_lists_in_order() {
    let w = fixture("signin.json");
    same("sharedLogins", json(&*SHARED_LOGINS), &w["sharedLogins"]);
    same("sharesIn", json(shares_in("/var/lib/wsp/logins")), &w["sharesIn"]);
    same("neverInImage", json(&*NEVER_IN_IMAGE), &w["neverInImage"]);
    same("secretsFiles", json!({ "sh": SECRETS_FILE_SH, "fish": SECRETS_FILE_FISH }), &w["secretsFiles"]);
    same("loginPaths", json(&*LOGIN_PATHS), &w["loginPaths"]);
    same("neverOnBrokered", json(&*NEVER_ON_BROKERED), &w["neverOnBrokered"]);
    same("ghLoginPaths", json(&*GH_LOGIN_PATHS), &w["ghLoginPaths"]);
    same("vaultVariables", json(VAULT_VARIABLES.iter().collect::<Vec<_>>()), &w["vaultVariables"]);
    let codex = shared_login_of(sign_in_row("codex")).unwrap();
    assert_eq!(shared_file_in(codex), "codex/auth.json");
    assert_eq!(login_home_in("/l", codex), "/l/codex");
}

#[test]
fn the_vault_keeps_row_variables_out_of_server_values_and_names_their_rows() {
    let w = fixture("signin.json");
    let record: IndexMap<String, String> = IndexMap::from([
        ("ANTHROPIC_API_KEY".into(), "a".into()),
        ("CLAUDE_CODE_OAUTH_TOKEN".into(), "t".into()),
        ("WSP_MCP_LINEAR_AUTHORIZATION".into(), "tok".into()),
        ("NOTION_TOKEN".into(), "n".into()),
        ("OPENAI_API_KEY".into(), "o".into()),
        ("WSP_MCP_X_AUTHORIZATION_BEARER".into(), "kept".into()),
    ]);
    let got = server_values_of(&record);
    same("serverValuesOf", json(&got), &w["serverValuesOf"]);
    let order: Vec<&String> = got.keys().collect();
    let want: Vec<&String> = w["serverValuesOf"].as_object().unwrap().keys().collect();
    assert_eq!(order, want, "a bearer name goes ahead of the values, as the TS spread puts it");
    for (name, want) in w["vaultVariableRow"].as_object().unwrap() {
        same(&format!("vaultVariableRow {name}"), opt(vault_variable_row(name)), want);
    }
}

#[test]
fn a_paste_is_the_token_only_where_the_whole_trimmed_paste_is_its_shape() {
    let token = mints_token(sign_in_row("claude")).unwrap();
    for case in fixture("signin.json")["tokenIn"].as_array().unwrap() {
        same(&format!("tokenIn {}", case["pasted"]), opt(token_in(token, s(&case["pasted"]))), &case["out"]);
    }
}

#[test]
fn the_secret_a_status_names_and_geminis_source_are_the_ts_words() {
    let w = fixture("signin.json");
    for case in w["secretNamed"].as_array().unwrap() {
        same(&format!("secretNamed {}", case["output"]), opt(secret_named(s(&case["output"]), &secrets())), &case["out"]);
    }
    for case in w["geminiSource"].as_array().unwrap() {
        let o = s(&case["output"]);
        same(&format!("geminiSource {o:?}"), opt(gemini_source(o, &secrets())), &case["out"]);
        same(&format!("geminiSource bare {o:?}"), opt(gemini_source(o, &IndexMap::new())), &case["bare"]);
    }
}

#[test]
fn claudes_status_json_is_read_from_the_first_brace_to_the_last() {
    let out = "warning: x\n{\"loggedIn\": true, \"authMethod\": \"claude.ai\"}\ntrailing } text";
    assert!(!claude_signed_in(out), "a brace after the object makes the span unparseable, as the TS greedy match does");
    assert!(claude_signed_in("{\"loggedIn\": true}"));
    assert!(!claude_signed_in("{\"loggedIn\": \"true\"}"));
    assert_eq!(claude_source("{\"loggedIn\": true, \"authMethod\": \"oauth_token\"}", &IndexMap::new()), Some(TOKEN_SOURCE.to_string()));
}
