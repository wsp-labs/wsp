#!/usr/bin/env python3
# Writes the verb-table half of cli-b's modules from table.json (dumped from the TS at 65f6b89c7): every entry's
# words, flags and tool text as data, every run and call body todo!() pointing at its TS lines.
import json, re, sys

TS = "/root/cli-b-ts"
OUT = "/root/wsp-vm/wt-2449-cli-b/host/crates/wsp-cli/src/verbs"
table = json.load(open("/root/cli-b-probe/table.json"))
FN = {"workspaces": "workspace_verbs", "agents": "agent_verbs", "computers": "computer_verbs", "projects": "project_verbs", "slate": "slate_verbs", "activity": "activity_verbs", "conversations": "conversation_verbs"}


def lit(s):
    out = json.dumps(s, ensure_ascii=False)
    if "\\u" in out:
        out = re.sub(r"\\u([0-9a-fA-F]{4})", lambda m: "\\u{" + m.group(1) + "}", out)
    return out


def snake(words):
    return re.sub(r"[^a-z0-9]+", "_", words.lower()).strip("_")


def kind(k):
    if isinstance(k, dict) and "optional" in k:
        return kind(k["optional"])
    if k == "string" or (isinstance(k, dict) and "string" in k):
        return "InputKind::String"
    if k == "boolean":
        return "InputKind::Boolean"
    if isinstance(k, dict) and "number" in k:
        checks = k["number"]
        if any(c[0] == "int" for c in checks):
            lo = [c for c in checks if c[0] == "min"]
            if lo:
                v = lo[0][1] if lo[0][2] else lo[0][1] + 1
                return f"InputKind::Integer {{ min: Some({v}) }}"
            return "InputKind::Integer { min: None }"
        return "InputKind::Number"
    if isinstance(k, dict) and "array" in k:
        return f"InputKind::StringList {{ min: {('Some(%d)' % k['min']) if 'min' in k else 'None'} }}"
    if isinstance(k, dict) and "enum" in k:
        return "InputKind::Enum(vec![" + ", ".join(lit(v) for v in k["enum"]) + "])"
    if isinstance(k, dict) and "literal" in k:
        return f"InputKind::Literal(serde_json::json!({json.dumps(k['literal'])}))"
    return "InputKind::Json"


def optional(k):
    return isinstance(k, dict) and "optional" in k


def ts_lines(file, name):
    """The line of the entry's name and of its run and call, in the TS file."""
    src = open(f"{TS}/{file}.ts").read().split("\n")
    words = name.split(" ")
    pat = re.compile(r'name: [`"]' + re.escape(name) + r'[`"]')
    at = next((i for i, l in enumerate(src) if pat.search(l)), None)
    if at is None and len(words) == 2:
        pat = re.compile(r"name: `" + re.escape(words[0]) + r" \$\{word\}`")
        at = next((i for i, l in enumerate(src) if pat.search(l)), None)
    run = call = None
    for i in range(at, len(src)):
        if run is None and re.search(r"^\s+run: async", src[i]):
            run = i
        if call is None and re.search(r"^\s+call: async", src[i]):
            call = i
        if i > at and re.search(r'^\s+name: [`"]', src[i]):
            break
    def end(s):
        ind = len(src[s]) - len(src[s].lstrip())
        for j in range(s + 1, len(src)):
            if src[j].strip() and len(src[j]) - len(src[j].lstrip()) <= ind:
                return j - 1 if re.match(r"^\s*[})\]]", src[j]) is None else (j if len(src[j]) - len(src[j].lstrip()) == ind else j - 1)
        return s
    return at + 1, (run + 1, end(run) + 1) if run is not None else None, (call + 1, end(call) + 1) if call is not None else None


def flags(opts):
    rows = []
    for o in opts:
        kind_ = "FlagKind::String" if o["type"] == "string" else "FlagKind::Boolean"
        rows.append(f'({lit(o["name"])}, FlagRow {{ kind: {kind_}, multiple: {"true" if o.get("multiple") else "false"}, short: {("Some(%s)" % repr(o["short"])) if o.get("short") else "None"}, value_with: {("Some(%s)" % lit(o["valueWith"])) if o.get("valueWith") else "None"} }})')
    return "FlagTable(vec![" + ", ".join(rows) + "])"


PAGE = {"front": "Page::Front", "agent": "Page::Agent", "host": "Page::Host", "dev": "Page::Dev", "app": "Page::App"}


def gen(file):
    entries = table[file]
    body, fns = [], []
    for v in entries:
        at, run, call = ts_lines(file, v["name"])
        s = snake(v["name"])
        tool = None
        if "tool" in v:
            t = v["tool"]
            tname = v["name"].replace(" ", "_")
            inputs = ",\n                ".join(
                "super::activity::processes_in()" if (file == "workspaces" and i["name"] == "processes") else f'ToolInput {{ name: {lit(i["name"])}, kind: {kind(i["kind"])}, optional: {"true" if optional(i["kind"]) else "false"}, describe: {("Some(%s.to_string())" % lit(i["describe"])) if i.get("describe") else "None"} }}'
                for i in t["input"]
            )
            tool = f"""Tool {{
            description: {lit(t["description"])}.to_string(),
            input: vec![{(chr(10) + "                " + inputs + "," + chr(10) + "            ") if inputs else ""}],
            output: vec![{", ".join(lit(o) for o in t["output"])}],
            stream: vec![{", ".join(lit(o) for o in t.get("stream", []))}],
            call: call_{snake(tname)},
        }}"""
            fns.append(f"""/// The `{tname}` tool's call (`verbs/{file}.ts:{call[0]}-{call[1]}`).
fn call_{snake(tname)}(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {{
    Box::pin(async move {{
        let _ = (args, deps);
        todo!("verbs/{file}.ts:{call[0]}-{call[1]}")
    }})
}}""")
        if run is not None:
            fns.append(f"""/// `wsp {v["name"]}` (`verbs/{file}.ts:{run[0]}-{run[1]}`).
fn run_{s}(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {{
    Box::pin(async move {{
        let _ = ctx;
        todo!("verbs/{file}.ts:{run[0]}-{run[1]}")
    }})
}}""")
        o = lambda key: ("Some(%s)" % lit(v[key])) if key in v else "None"
        if v.get("command"):
            body.append(f"""    // verbs/{file}.ts:{at}
    Verb::CommandTool(CommandToolVerb {{
        name: {lit(v["name"])},
        tool: {tool},
    }})""")
        elif "toolOnly" in v:
            body.append(f"""    // verbs/{file}.ts:{at}
    Verb::ToolOnly(ToolOnlyVerb {{
        name: {lit(v["name"])},
        tool: {tool},
        tool_only: {lit(v["toolOnly"])},
        reads_here: {o("readsHere")},
    }})""")
        else:
            common = f"""name: {lit(v["name"])},
        usage: {lit(v["usage"])}.to_string(),
        about: {lit(v["about"])}.to_string(),
        page: {PAGE[v["page"]]},
        options: {flags(v["options"])},
        run: run_{s},"""
            tail = f"""reads_here: {o("readsHere")},
        starts_no_host: {o("startsNoHost")},
        any_release: {o("anyRelease")},
        cloud: {"true" if v.get("cloud") else "false"},
        cloud_flags: vec![{", ".join(lit(x) for x in v.get("cloudFlags", []))}],"""
            if "cliOnly" in v:
                body.append(f"""    // verbs/{file}.ts:{at}
    Verb::CliOnly(CliOnlyVerb {{
        {common}
        {tail}
        cli_only: {lit(v["cliOnly"])},
        host_side: {o("hostSide")},
    }})""")
            else:
                body.append(f"""    // verbs/{file}.ts:{at}
    Verb::Cli(CliVerb {{
        {common}
        tool: {tool},
        {tail}
    }})""")
    return f"""/// The entries of `verbs/{file}.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn {FN[file]}() -> Vec<Verb> {{
    vec![
{(","+chr(10)).join(body)},
    ]
}}

{(chr(10)+chr(10)).join(fns)}
"""


if __name__ == "__main__":
    for f in FN:
        head = open(f"/root/cli-b-probe/heads/{f}.rs").read().rstrip("\n")
        open(f"{OUT}/{f}.rs", "w").write(head + "\n\n" + gen(f).rstrip("\n") + "\n")
    open(f"{OUT}/activity_table.rs", "w").write(open("/root/cli-b-probe/heads/activity_table.rs").read())
