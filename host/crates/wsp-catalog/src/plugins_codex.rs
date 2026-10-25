// SPDX-License-Identifier: AGPL-3.0-only
//! Codex's plugins, asked of `codex app-server`, and the script that asks it. Port of
//! packages/catalog/src/plugins-codex.ts and codex-app-server.ts at a3b870a6e; the spec is spec/plugins_codex.md.
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_wire::types::agents_report::PluginBrings;
use wsp_wire::types::agents_report::PluginMissing;
use wsp_wire::types::agents_report::PluginScope;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::agents_report::config_changed_refusal;
use wsp_words::exit::HostError;

use crate::json_parse::parse_json;
use crate::jsonc::js_stringify;
use crate::plugins::{
    join_path, js_trim, nothing_brought, plugin_id_parts, sorted, unique, FoundPlugin, PeekDir, PeekGroup, PluginIo, PluginsFound,
};

/// codex-app-server.ts:448.
pub const LINE_WAIT_S: u64 = 10;

/// `{"jsonrpc":"2.0","id":<id>,"method":<method>,"params":<params>}` as `JSON.stringify` writes it
/// (codex-app-server.ts:450).
pub fn request(id: i64, method: &str, params: Value) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":{},\"params\":{}}}",
        js_stringify(&Value::String(method.to_string())),
        js_stringify(&params)
    )
}

/// codex-app-server.ts:452.
pub fn notification(method: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"method\":{},\"params\":{{}}}}", js_stringify(&Value::String(method.to_string())))
}

/// codex-app-server.ts:454.
pub fn initialize_request(id: i64) -> String {
    request(id, "initialize", json!({ "clientInfo": { "name": "wsp", "version": "0" } }))
}

/// codex-app-server.ts:457.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStage {
    pub lines: Vec<String>,
    pub answers: u64,
}

/// codex-app-server.ts:463.
pub fn app_server_script(codex: &str, stages: &[ServerStage]) -> String {
    let mut waited: u64 = 0;
    let mut staged: Vec<String> = Vec::new();
    for (at, stage) in stages.iter().enumerate() {
        let before = waited;
        waited += stage.answers;
        let quoted: Vec<String> = stage.lines.iter().map(|line| shell_quote(line)).collect();
        let body = [
            format!("printf '%s\\n' {} >&3", quoted.join(" ")),
            format!("while [ \"$answers\" -lt {waited} ] && IFS= read -r -t {LINE_WAIT_S} -u 4 line; do"),
            "  printf '%s\\n' \"$line\"".to_string(),
            "  case $line in '{\"id\":'*) answers=$((answers + 1)) ;; esac".to_string(),
            "done".to_string(),
        ];
        if at == 0 {
            staged.extend(body);
        } else {
            staged.push(format!("if [ \"$answers\" -ge {before} ]; then"));
            staged.extend(body.into_iter().map(|l| format!("  {l}")));
            staged.push("fi".to_string());
        }
    }
    let mut lines = vec![
        // Two named pipes rather than a coprocess: the Mac's own bash is 3.2, which has none, and the tests prove this
        // script on whatever bash runs them.
        "WSP_PROBE_DIR=$(mktemp -d) && mkfifo \"$WSP_PROBE_DIR/in\" \"$WSP_PROBE_DIR/out\"".to_string(),
        format!("{codex} app-server <\"$WSP_PROBE_DIR/in\" >\"$WSP_PROBE_DIR/out\" &"),
        "WSP_APP_SERVER_PID=$!".to_string(),
        "exec 3>\"$WSP_PROBE_DIR/in\" 4<\"$WSP_PROBE_DIR/out\"".to_string(),
        "answers=0".to_string(),
    ];
    lines.extend(staged);
    lines.extend([
        "exec 3>&- 4<&-".to_string(),
        // Only ever the pid the shell recorded: bare `kill 0` would signal the whole group.
        "kill \"$WSP_APP_SERVER_PID\" 2>/dev/null || :".to_string(),
        "rm -rf \"$WSP_PROBE_DIR\"".to_string(),
    ]);
    lines.join("\n")
}

/// codex-app-server.ts:493.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerAnswer {
    Result(Map<String, Value>),
    Error { code: Option<i64>, message: String, data: Option<Map<String, Value>> },
}

/// A JSON number as a request id: whole numbers only, as JavaScript's Map keys them.
fn id_of(value: &Value) -> Option<i64> {
    let n = value.as_number()?;
    n.as_i64().or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64))
}

/// Each answer by its numeric id, the last of an id winning (codex-app-server.ts:500).
pub fn answers_of(text: &str) -> IndexMap<i64, ServerAnswer> {
    let mut by_id: IndexMap<i64, ServerAnswer> = IndexMap::new();
    for raw in text.split('\n') {
        let line = js_trim(raw);
        if !line.starts_with('{') {
            continue;
        }
        let Ok(Value::Object(message)) = parse_json(line) else {
            continue;
        };
        let Some(id) = message.get("id").and_then(id_of) else {
            continue;
        };
        if let Some(result) = message.get("result").and_then(Value::as_object) {
            by_id.insert(id, ServerAnswer::Result(result.clone()));
        } else if let Some(error) = message.get("error").and_then(Value::as_object) {
            by_id.insert(
                id,
                ServerAnswer::Error {
                    code: error.get("code").and_then(id_of),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("codex refused the request without saying why")
                        .to_string(),
                    data: error.get("data").and_then(Value::as_object).cloned(),
                },
            );
        }
    }
    by_id
}

/// The result answered under one id, or nothing where the server answered an error or nothing at all (codex-app-server.ts:525).
pub fn result_of(answers: &IndexMap<i64, ServerAnswer>, id: i64) -> Option<&Map<String, Value>> {
    match answers.get(&id)? {
        ServerAnswer::Result(result) => Some(result),
        ServerAnswer::Error { .. } => None,
    }
}

/// Printed in place of the answers where no codex answers on the login's PATH: no plugins, and nothing to refuse.
const NO_CODEX: &str = "__WSP_NO_CODEX__";

const INIT: i64 = 1;
const LIST: i64 = 2;
const CONFIG: i64 = 3;

/// The requests as one script on the computer, or nothing said where codex is not there (plugins-codex.ts:22).
async fn ask(io: &impl PluginIo, lines: &[String]) -> Option<String> {
    let stage = ServerStage { lines: lines.to_vec(), answers: lines.len() as u64 };
    io.run(&format!(
        "command -v codex >/dev/null 2>&1 || {{ echo {NO_CODEX}; exit 0; }}\ncd ~ || exit 1\n{}",
        app_server_script("codex", &[stage])
    ))
    .await
}

/// A JSON string that is not empty (plugins-codex.ts:25).
fn nonempty(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from)
}

/// The objects of a list, in its order.
fn records(value: Option<&Value>) -> Vec<&Map<String, Value>> {
    value.and_then(Value::as_array).map_or_else(Vec::new, |items| items.iter().filter_map(Value::as_object).collect())
}

/// The distinct strings under `key` of a list's objects, sorted (plugins-codex.ts:26).
fn names(value: Option<&Value>, key: &str) -> Vec<String> {
    let found =
        value.and_then(Value::as_array).into_iter().flatten().filter_map(|item| item.get(key).and_then(Value::as_str).map(String::from));
    sorted(unique(found))
}

/// What one plugin/read answer says the plugin brings (plugins-codex.ts:30).
fn brought_by(read: Option<&Map<String, Value>>) -> PluginBrings {
    let Some(plugin) = read.and_then(|r| r.get("plugin")).and_then(Value::as_object) else {
        return nothing_brought();
    };
    let servers = plugin
        .get("mcpServers")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |items| sorted(items.iter().filter_map(Value::as_str).map(String::from).collect()));
    PluginBrings {
        skills: names(plugin.get("skills"), "name"),
        commands: vec![],
        subagents: vec![],
        hooks: names(plugin.get("hooks"), "eventName"),
        servers,
        lsp: vec![],
        apps: names(plugin.get("apps"), "name"),
    }
}

/// A plugin's source: its url, path or package (plugins-codex.ts:42).
fn source_of(source: Option<&Value>) -> Option<String> {
    let source = source?.as_object()?;
    nonempty(source.get("url")).or_else(|| nonempty(source.get("path"))).or_else(|| nonempty(source.get("package")))
}

/// The user layer of a config/read with its layers: its file and the version a write names (plugins-codex.ts:45).
fn user_layer(config: &Map<String, Value>) -> (Option<String>, Option<String>) {
    let user = config
        .get("layers")
        .and_then(Value::as_array)
        .and_then(|layers| layers.iter().find(|l| l.get("name").and_then(|n| n.get("type")).and_then(Value::as_str) == Some("user")));
    let Some(user) = user else {
        return (None, None);
    };
    let file = nonempty(user.get("name").and_then(|n| n.get("file")));
    (file, nonempty(user.get("version")))
}

/// The local folder a config.toml marketplace names, when it is one (plugins-codex.ts:99).
fn local_source(known: Option<&Map<String, Value>>, id: &str) -> Option<String> {
    let at = known?.get(&plugin_id_parts(id).1)?.as_object()?;
    if at.get("source_type").and_then(Value::as_str) != Some("local") {
        return None;
    }
    at.get("source").and_then(Value::as_str).filter(|s| s.starts_with('/')).map(String::from)
}

/// A process-level refusal of the read (plugins-codex.ts:66).
fn unanswered() -> PluginsFound {
    PluginsFound {
        plugins: vec![],
        refused: vec!["plugins: Codex's app server did not answer, so Codex's plugins were not read".to_string()],
        version: None,
        file: None,
    }
}

/// Codex's shelf: plugin/list for the rows, config/read for what config.toml names and the version a write checks,
/// plugin/read for what each brings (plugins-codex.ts:344).
pub async fn read_codex_plugins(io: &impl PluginIo) -> PluginsFound {
    let said = ask(
        io,
        &[
            initialize_request(INIT),
            request(LIST, "plugin/list", json!({})),
            request(CONFIG, "config/read", json!({ "includeLayers": true })),
        ],
    )
    .await;
    if said.as_deref().is_some_and(|s| s.contains(NO_CODEX)) {
        return PluginsFound::default();
    }
    let answers = answers_of(said.as_deref().unwrap_or(""));
    let home = result_of(&answers, INIT).and_then(|r| nonempty(r.get("codexHome")));
    // A Codex with no plugins answers plugin/list as a method it does not know, as 0.162.1 answers any such: none to read.
    if let Some(ServerAnswer::Error { message, .. }) = answers.get(&LIST) {
        if message.contains("unknown variant `plugin/list`") {
            return PluginsFound::default();
        }
    }
    let (Some(list), Some(config)) = (result_of(&answers, LIST), result_of(&answers, CONFIG)) else {
        return unanswered();
    };
    let marketplaces = records(list.get("marketplaces"));
    type Installed<'a> = (&'a Map<String, Value>, &'a Map<String, Value>, &'a str);
    let mut installed: Vec<Installed> = Vec::new();
    for mk in &marketplaces {
        for p in records(mk.get("plugins")) {
            if p.get("installed") == Some(&Value::Bool(true)) {
                if let Some(id) = p.get("id").and_then(Value::as_str) {
                    installed.push((mk, p, id));
                }
            }
        }
    }
    // What each brings, in one more run, since plugin/read takes the marketplace file plugin/list names.
    let reads = if installed.is_empty() {
        None
    } else {
        let mut lines = vec![initialize_request(INIT)];
        for (i, (mk, p, _)) in installed.iter().enumerate() {
            let mut params = Map::new();
            if let Some(name) = p.get("name") {
                params.insert("pluginName".to_string(), name.clone());
            }
            if let Some(path) = mk.get("path").and_then(Value::as_str) {
                params.insert("marketplacePath".to_string(), Value::String(path.to_string()));
            }
            lines.push(request(LIST + i as i64, "plugin/read", Value::Object(params)));
        }
        Some(answers_of(ask(io, &lines).await.as_deref().unwrap_or("")))
    };
    let mut plugins: Vec<FoundPlugin> = installed
        .iter()
        .enumerate()
        .map(|(i, (mk, p, id))| {
            let (name, marketplace) = plugin_id_parts(id);
            let face = p.get("interface").and_then(Value::as_object);
            let read = reads.as_ref().and_then(|r| result_of(r, LIST + i as i64));
            let version = nonempty(p.get("localVersion"));
            let description = nonempty(read.and_then(|r| r.get("plugin")).and_then(Value::as_object).and_then(|pl| pl.get("description")))
                .or_else(|| nonempty(face.and_then(|f| f.get("shortDescription"))));
            FoundPlugin {
                id: id.to_string(),
                name: nonempty(p.get("name")).unwrap_or_else(|| name.clone()),
                marketplace: nonempty(mk.get("name")).unwrap_or_else(|| marketplace.clone()),
                // Where `codex plugin add` put it on 0.162.1, which plugin/list does not say.
                path: match (&home, &version) {
                    (Some(home), Some(version)) => {
                        Some(join_path(&[home.as_str(), "plugins", "cache", marketplace.as_str(), name.as_str(), version.as_str()]))
                    }
                    _ => None,
                },
                version,
                scope: PluginScope::User,
                project: None,
                on: p.get("enabled") == Some(&Value::Bool(true)),
                missing: None,
                set_in: None,
                description,
                source: source_of(p.get("source")),
                brings: brought_by(read),
                skills: None,
            }
        })
        .collect();
    // A plugin config.toml names that plugin/list does not return loads nowhere, and Codex says nothing of it.
    let conf = config.get("config").and_then(Value::as_object);
    let named = conf.and_then(|c| c.get("plugins")).and_then(Value::as_object);
    let known = conf.and_then(|c| c.get("marketplaces")).and_then(Value::as_object);
    let listed: Vec<&str> = marketplaces
        .iter()
        .flat_map(|mk| records(mk.get("plugins")).into_iter().filter_map(|p| p.get("id").and_then(Value::as_str)))
        .collect();
    let mut absent: Vec<String> = named.map_or_else(Vec::new, |n| n.keys().filter(|k| !listed.contains(&k.as_str())).cloned().collect());
    absent = sorted(absent);
    let sources = unique(absent.iter().filter_map(|id| local_source(known, id)));
    let seen = if sources.is_empty() {
        None
    } else {
        io.peek(&[PeekGroup { roots: Some(sources), dirs: Some(vec![PeekDir { path: ".".to_string(), depth: 1 }]), ..Default::default() }])
            .await
    };
    for id in &absent {
        let (name, marketplace) = plugin_id_parts(id);
        let setting = named.and_then(|n| n.get(id));
        let source = local_source(known, id);
        let folder_there = source.as_ref().map(|s| seen.as_ref().and_then(|p| p.list(s)).is_some());
        plugins.push(FoundPlugin {
            id: id.clone(),
            name,
            marketplace,
            version: None,
            scope: PluginScope::User,
            project: None,
            on: setting.and_then(|s| s.get("enabled")) == Some(&Value::Bool(true)),
            missing: Some(if folder_there == Some(false) { PluginMissing::Marketplace } else { PluginMissing::Unlisted }),
            path: None,
            set_in: None,
            description: None,
            source,
            brings: nothing_brought(),
            skills: None,
        });
    }
    // One config.toml names that plugin/list has in a marketplace and not installed loads nowhere either.
    for mk in &marketplaces {
        for p in records(mk.get("plugins")) {
            let Some(id) = p.get("id").and_then(Value::as_str) else {
                continue;
            };
            if p.get("installed") == Some(&Value::Bool(true)) || !named.is_some_and(|n| n.contains_key(id)) {
                continue;
            }
            let (name, marketplace) = plugin_id_parts(id);
            let setting = named.and_then(|n| n.get(id));
            plugins.push(FoundPlugin {
                id: id.to_string(),
                name: nonempty(p.get("name")).unwrap_or(name),
                marketplace: nonempty(mk.get("name")).unwrap_or(marketplace),
                version: None,
                scope: PluginScope::User,
                project: None,
                on: setting.and_then(|s| s.get("enabled")) == Some(&Value::Bool(true)),
                missing: Some(PluginMissing::Uninstalled),
                path: None,
                set_in: None,
                description: None,
                source: source_of(p.get("source")),
                brings: nothing_brought(),
                skills: None,
            });
        }
    }
    let refused: Vec<String> = list
        .get("marketplaceLoadErrors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| e.get("message").and_then(Value::as_str))
        .map(|m| format!("plugins: Codex could not load a marketplace: {m}"))
        .collect();
    let (file, version) = user_layer(config);
    PluginsFound { plugins, refused, version, file }
}

/// The switch (plugins-codex.ts:421): the app server's `config/value/write` on `plugins.<id>.enabled`, checked against
/// the config version the read named.
pub async fn turn_codex(io: &impl PluginIo, plugin: &FoundPlugin, on: bool, read: &PluginsFound) -> Result<Option<String>, HostError> {
    let mut params = Map::new();
    params.insert("keyPath".to_string(), Value::String(format!("plugins.{}.enabled", plugin.id)));
    params.insert("value".to_string(), Value::Bool(on));
    params.insert("mergeStrategy".to_string(), Value::String("upsert".to_string()));
    if let Some(version) = &read.version {
        params.insert("expectedVersion".to_string(), Value::String(version.clone()));
    }
    let write = request(LIST, "config/value/write", Value::Object(params));
    let said = ask(io, &[initialize_request(INIT), write]).await;
    let answers = answers_of(said.as_deref().unwrap_or(""));
    if result_of(&answers, LIST).and_then(|r| r.get("status")) == Some(&Value::String("ok".to_string())) {
        return Ok(None);
    }
    let error = match answers.get(&LIST) {
        Some(ServerAnswer::Error { message, data, .. }) => Some((message.as_str(), data.as_ref())),
        _ => None,
    };
    let conflict = error.and_then(|(_, data)| data).and_then(|d| d.get("config_write_error_code")).and_then(Value::as_str)
        == Some("configVersionConflict");
    if conflict {
        return Ok(Some(config_changed_refusal(read.file.as_deref().unwrap_or("config.toml"))));
    }
    let said = match error {
        Some((message, _)) => message.strip_suffix('.').unwrap_or(message).to_string(),
        None => "Codex's app server did not answer".to_string(),
    };
    Ok(Some(format!("{said}, so nothing was switched.")))
}
