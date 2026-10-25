use serde_json::{json, Value};
use std::collections::HashMap;
use wsp_wire::views::activity::*;
use wsp_wire::types::js_number::{JsInt, JsNumber};

fn pr(pid: u32, ppid: u32, extra: Value) -> Value {
    let mut d = json!({"pid": pid, "ppid": ppid, "user": "ada", "state": "S", "comm": "node", "cmdline": "node", "cpu": 0, "rss": 1_000_000, "startedAt": 1_760_000_000_000i64});
    for (k, v) in extra.as_object().unwrap() { d[k] = v.clone(); }
    d
}

#[test]
fn scratch() {
    let owner = json!({"by":"fork","thread":"thr-aaaaaaaa"});
    let procs: Vec<wsp_frames::ProcEntry> = vec![
        pr(1, 0, json!({"comm":"wsp-daemon","cmdline":"wsp-daemon serve","rss":9_000_000})),
        pr(4100, 1, json!({"comm":"2.1.296","cmdline":"claude -p --input-format stream-json","rss":300_000_000,"cpu":3,"kind":"agent","owner":owner,"startedAt":1_760_000_000_000i64})),
        pr(4150, 4100, json!({"cmdline":"node wsp mcp --scoped","rss":70_000_000,"owner":owner,"startedAt":1_760_000_000_500i64})),
        pr(4200, 1, json!({"cmdline":"node /lab/node_modules/.bin/vite","rss":80_000_000,"cpu":10,"kind":"dev","owner":owner,"startedAt":1_760_000_001_000i64,"id":"88000"})),
        pr(4300, 4200, json!({"comm":"esbuild","cmdline":"/lab/node_modules/esbuild/bin/esbuild --service","rss":20_000_000,"owner":owner,"startedAt":1_760_000_002_000i64,"id":"88001"})),
        pr(5000, 4999, json!({"comm":"zsh","cmdline":"-zsh","rss":4_000_000})),
    ].into_iter().map(|v| serde_json::from_value(v).unwrap()).collect();
    let mut ports: HashMap<u32, Vec<JsInt>> = HashMap::new();
    ports.insert(4200, vec![JsInt::from(5173)]);
    let read = ActivityRead {
        computer: "this computer".into(), at: JsNumber::from(1_760_000_000_000i64), procs, daemon: 1, host: 4242,
        scopes: vec![ActivityScope { thread_id: "thr-aaaaaaaa".into(), title: "start the dev server".into(), workspace_id: "w1".into(), roots: vec![], ..Default::default() }],
        agent_bins: vec!["claude".into()], ports: Some(ports), cgroups: None,
    };
    let v = activity_of(&read);
    println!("VIEW {}", serde_json::to_string_pretty(&v).unwrap());
    panic!("scratch");
}
