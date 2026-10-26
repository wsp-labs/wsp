// SPDX-License-Identifier: AGPL-3.0-only
//! The JSX-like form: packages/protocol/test/slate/syntax.test.ts at 65f6b89c7, with the parse and print cases of
//! files.test.ts, the parse cases of kit.test.ts and hostile.test.ts, and the spec examples of examples.ts.

use std::rc::Rc;

use serde_json::{json, Value};
use wsp_slates::pending::slate::*;

const SPEC_EXAMPLES: &[(&str, &str)] = &[
    ("02-model-1", "<slate title=\"Deploy setup\">\n  <value name=\"id\" start=\"\" />\n  <derived name=\"ok\" value={$check.state == 'done' and $check.json.id != null} />\n  <run name=\"check\" cmd='gh api \"repos/$REPO/issues/$ID\"' env={{ REPO: $repo, ID: $id }} timeout={20} />\n  <value name=\"repo\" start=\"wsp-labs/wsp\" />\n  <when change={$id} do={start($check)} />\n  <column>\n    <input id=\"issue\" label=\"Issue number\" value={$id} mono />\n    <text when={$check.state == 'running'} tone=\"muted\">Checking</text>\n    <text when={$check.state == 'failed'} tone=\"bad\">No such issue: {$check.err}</text>\n    <text when={$ok} tone=\"good\">{$check.json.title}</text>\n    <button label=\"Next\" variant=\"primary\" when={$ok} onPress={set($step, 2)} />\n  </column>\n  <value name=\"step\" start={1} />\n</slate>"),
    ("03-syntax-1", "<slate title=\"Pull request\">\n  <derived name=\"failing\" value={pr.checks | where(item.state == 'fail')} />\n  <column>\n    <empty when={pr.number == null} title=\"No pull request yet\">This thread's branch has no pull request.</empty>\n    <facts when={pr.number != null}>\n      <fact label=\"State\" value={pr.word} />\n      <fact label=\"Review\" value={word(pr.review)} />\n      <fact label=\"Mergeable\" value={word(pr.mergeable)} tone={pr.mergeable == 'mergeable' ? 'good' : 'muted'} />\n    </facts>\n    <table id=\"checks\" when={pr.number != null} items={pr.checks} key={item.name} empty=\"No checks yet\">\n      <col title=\"Check\" value={item.name} />\n      <col title=\"State\" value={word(item.state)} tone={item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'} />\n      <col title=\"Took\" value={item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'} mono align=\"end\" />\n      <action label=\"Send to agent\" when={item.state == 'fail'} onPress={send(\"A check failed. Read its log and fix it.\", item.name, item.link)} />\n    </table>\n    <text when={len($failing) > 0} tone=\"muted\">{len($failing)} failing of {len(pr.checks)}</text>\n    <button label=\"Open on GitHub\" when={pr.number != null} onPress={open(pr.url)} />\n  </column>\n</slate>"),
    ("07-runs-1", "<slate title=\"Runs\">\n  <value name=\"id\" start=\"\" />\n  <value name=\"body\" start=\"\" />\n  <run name=\"check\" cmd='gh api \"repos/{owner}/{repo}/issues/$ID\"' env={{ ID: $id }} timeout={20} />\n  <run name=\"tests\" cmd=\"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 $1\" args={[\"packages/host/test/parity.test.ts\"]} stream timeout={300} once />\n  <run name=\"note\" cmd=\"cat > notes/latest.md\" stdin={$body} on=\"host\" cwd=\"notes\" />\n  <run name=\"issues\" tool=\"linear.list_issues\" args={{ first: 50 }} every=\"60s\" />\n  <run name=\"board\" resource=\"linear:linear://team/WSP/board\" every=\"60s\" />\n  <column>\n    <input label=\"Issue\" value={$id} />\n    <input label=\"Note\" value={$body} lines={4} />\n    <button label=\"Check\" onPress={start($check)} />\n    <button label=\"Run the parity test\" onPress={start($tests)} />\n    <output run={$tests} lines={12} />\n    <button label=\"Save the note\" onPress={start($note)} />\n  </column>\n</slate>"),
    ("08-secrets-1", "<slate title=\"Deploy key\">\n  <secret name=\"token\" />\n  <value name=\"saved\" start={false} />\n  <run name=\"save\" cmd='printf \"VERCEL_TOKEN=%s\\n\" \"$TOKEN\" >> .env' env={{ TOKEN: $token }} on=\"host\" />\n  <when done={$save} do={set($saved, $save.exit == 0)} />\n  <column>\n    <input label=\"Vercel token\" value={$token} kind=\"password\" />\n    <text when={$token.set and not $saved} tone=\"muted\">Filled, {$token.len} characters</text>\n    <button label=\"Write it to .env\" when={$token.set} onPress={start($save)} />\n    <text when={$saved} tone=\"good\">Written to .env</text>\n  </column>\n</slate>"),
    ("appendix-examples-1", "<slate title=\"Pull request\">\n  <derived name=\"failing\" value={pr.checks | where(item.state == 'fail')} />\n  <run name=\"merge\" cmd='gh pr merge --squash --auto \"$PR\"' env={{ PR: pr.number }} timeout={30}\n    confirm=\"Enable auto-merge on this pull request? It merges when the checks pass.\" />\n  <column>\n    <empty when={pr.number == null} title=\"No pull request yet\">This thread's branch has no pull request.</empty>\n    <facts id=\"head\" when={pr.number != null}>\n      <fact label=\"State\" value={pr.word} />\n      <fact label=\"Review\" value={word(pr.review)} />\n      <fact label=\"Mergeable\" value={word(pr.mergeable)} tone={pr.mergeable == 'mergeable' ? 'good' : pr.mergeable == 'conflicting' ? 'warning' : 'muted'} />\n    </facts>\n    <table id=\"checks\" when={pr.number != null} items={pr.checks} key={item.name} empty=\"No checks yet\">\n      <col title=\"Check\" value={item.name} />\n      <col title=\"State\" value={word(item.state)} tone={item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'} />\n      <col title=\"Took\" value={item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'} mono align=\"end\" />\n      <action label=\"Send to agent\" when={item.state == 'fail'} onPress={send(\"A check failed. Read its log and fix it.\", item.name, item.link)} />\n    </table>\n    <text id=\"summary\" when={len($failing) > 0} tone=\"muted\">{len($failing)} failing of {len(pr.checks)}</text>\n    <row when={pr.number != null}>\n      <button label=\"Open on GitHub\" onPress={open(pr.url)} />\n      <button label=\"Checks and comments\" variant=\"quiet\" onPress={pane(\"pr\")} />\n      <button id=\"automerge\" label=\"Auto-merge\" when={pr.mergeable == 'mergeable' and pr.review == 'approved' and $merge.state != 'done'} onPress={start($merge)} />\n    </row>\n    <text when={$merge.state == 'done'} tone=\"good\">Auto-merge is on</text>\n    <text when={$merge.state == 'failed'} tone=\"bad\">gh refused: {short($merge.err, 120)}</text>\n  </column>\n</slate>"),
    ("appendix-examples-2", "<slate title=\"Cart rounding fix\">\n  <value name=\"steps\" start={[\n    { title: \"Find where the cart rounds totals\", done: true },\n    { title: \"Round once at the end\", done: true },\n    { title: \"Fix the two failing tests\", done: false },\n    { title: \"Open a pull request\", done: false }\n  ]} />\n  <derived name=\"done\" value={$steps | where(item.done) | count} />\n  <run name=\"tests\" cmd=\"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 test/cart.test.ts\" stream timeout={300} once />\n  <when done={$tests} do={set($steps[2].done, $tests.exit == 0)} />\n  <column>\n    <meter id=\"progress\" label=\"Steps\" value={$done} max={len($steps)} format=\"fraction\" />\n    <checklist id=\"todo\" items={$steps} key={item.title} title={item.title} done={item.done} editable />\n    <section title=\"Files changed\" collapsible>\n      <table items={thread.changes.files} key={item.path} empty=\"Nothing changed yet\">\n        <col title=\"File\" value={item.path} mono />\n        <col title=\"Added\" value={item.additions} tone=\"good\" align=\"end\" />\n        <col title=\"Removed\" value={item.deletions} tone=\"bad\" align=\"end\" />\n      </table>\n    </section>\n    <row>\n      <button label=\"Run the cart tests\" when={$tests.state != 'running'} onPress={start($tests)} />\n      <button id=\"next\" label=\"Go on to the next step\" variant=\"primary\"\n        when={$done < len($steps) and thread.status != 'working'}\n        onPress={send(\"Go on to the next unchecked step on the slate.\", $steps)} />\n    </row>\n    <output run={$tests} lines={10} />\n  </column>\n</slate>"),
    ("appendix-examples-3", "<slate title=\"Deploy setup\">\n  <value name=\"step\" start={1} />\n  <value name=\"repo\" start=\"acme/landing\" />\n  <value name=\"project\" start=\"\" />\n  <secret name=\"vercelToken\" />\n  <run name=\"check\" cmd='vercel project inspect \"$PROJECT\" 2>&1 | head -c 4000'\n    env={{ PROJECT: $project, VERCEL_TOKEN: $vercelToken }} on=\"host\" timeout={30} />\n  <run name=\"env\" cmd='grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env'\n    env={{ VERCEL_TOKEN: $vercelToken }} on=\"host\" timeout={10} />\n  <run name=\"ci\" cmd='gh secret set VERCEL_TOKEN --repo \"$REPO\"' stdin={$vercelToken}\n    env={{ REPO: $repo }} on=\"host\" timeout={60} />\n  <derived name=\"checked\" value={$check.state == 'done' and contains($check.out, $project)} />\n  <derived name=\"written\" value={$env.state == 'done' and $ci.state == 'done'} />\n  <when change={$project} do={start($check)} />\n  <when change={$checked} do={set($step, $checked ? 3 : 2)} />\n  <when change={$written} do={set($step, $written ? 4 : 3)} />\n  <column>\n    <text emphasis=\"strong\">Step {$step} of 4</text>\n\n    <section title=\"1. The Vercel token\" when={$step == 1}>\n      <markdown>Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent.</markdown>\n      <button label=\"Open Vercel tokens\" onPress={open(\"https://vercel.com/account/tokens\")} />\n      <input label=\"Vercel token\" value={$vercelToken} kind=\"password\" />\n      <button label=\"Next\" variant=\"primary\" when={$vercelToken.set} onPress={set($step, 2)} />\n    </section>\n\n    <section title=\"2. The project\" when={$step == 2 or $step == 3}>\n      <input id=\"project-name\" label=\"Project name on Vercel\" value={$project} mono />\n      <text when={$check.state == 'held'} tone=\"muted\">Approve the check to go on</text>\n      <text when={$check.state == 'running'} tone=\"muted\">Checking</text>\n      <text when={$check.state == 'failed'} tone=\"bad\">Vercel did not find it: {short($check.err, 160)}</text>\n      <text when={$checked} tone=\"good\">Found {$project}</text>\n    </section>\n\n    <section title=\"3. Write it down\" when={$step == 3}>\n      <facts>\n        <fact label=\"Repo\" value={$repo} mono />\n        <fact label=\"Project\" value={$project} mono />\n      </facts>\n      <row>\n        <button label=\"Write .env\" when={$env.state != 'done'} onPress={start($env)} />\n        <button label=\"Set the CI secret\" when={$ci.state != 'done'} onPress={start($ci)} />\n      </row>\n      <text when={$env.state == 'done'} tone=\"good\">.env written</text>\n      <text when={$ci.state == 'done'} tone=\"good\">CI secret set</text>\n      <text when={$env.state == 'failed' or $ci.state == 'failed'} tone=\"bad\">{short(concat($env.err, $ci.err), 200)}</text>\n    </section>\n\n    <section title=\"4. Done\" when={$step == 4}>\n      <text tone=\"good\">The token is in .env and in the repo's secrets.</text>\n      <button id=\"write\" label=\"Tell the agent\" variant=\"primary\"\n        onPress={send(\"The Vercel setup is done on the slate; wire the deploy hook next.\", $repo, $project, $vercelToken)} />\n    </section>\n  </column>\n</slate>"),
    ("appendix-examples-4", "<slate title=\"Linear\">\n  <value name=\"hide\" start=\"Done\" />\n  <value name=\"pick\" start=\"\" />\n  <run name=\"issues\" tool=\"linear.list_issues\" args={{ first: 50 }} every=\"60s\" always />\n  <run name=\"finish\" tool=\"linear.update_issue\" args={{ id: $pick, state: \"Done\" }} />\n  <derived name=\"open\" value={$issues.json | where(item.state != $hide) | sortBy(item.priority)} />\n  <when done={$finish} do={start($issues)} />\n  <column>\n    <text when={mcp.linear.state != 'allowed'} tone=\"muted\">Allow Linear in this thread's slate to see the board.</text>\n    <row align=\"between\">\n      <text emphasis=\"strong\">{len($open)} open</text>\n      <select label=\"Hide\" value={$hide}>\n        <option value=\"Done\" />\n        <option value=\"Canceled\" />\n        <option value=\"In Review\" />\n      </select>\n    </row>\n    <table id=\"board\" items={$open} key={item.id} empty=\"Nothing open\">\n      <col title=\"Issue\" value={item.identifier} mono />\n      <col title=\"Title\" value={item.title} />\n      <col title=\"Who\" value={item.assignee} tone=\"muted\" />\n      <action label=\"Work on this\" onPress={send(\"Work on this Linear issue. Read it on Linear first.\", item.identifier, item.url)} />\n      <action label=\"Done\" onPress={[set($pick, item.id), start($finish)]} />\n    </table>\n  </column>\n</slate>"),
    ("appendix-examples-5", "<slate title=\"Focus\">\n  <value name=\"startedAt\" start={null} />\n  <value name=\"minutes\" start=\"25\" />\n  <derived name=\"length\" value={num($minutes) * 60000} />\n  <derived name=\"left\" value={exists($startedAt) ? max(0, $startedAt + $length - time.now) : null} />\n  <derived name=\"over\" value={exists($startedAt) and time.now >= $startedAt + $length} />\n  <derived name=\"working\" value={thread.plan.steps | where(item.state == 'working') | first} />\n  <column>\n    <text emphasis=\"strong\">Focus timer</text>\n    <text when={exists($startedAt)} size=\"large\" mono>{duration($left)} left</text>\n    <meter label=\"Elapsed\" when={exists($startedAt)} value={clamp(time.now - $startedAt, 0, $length)} max={$length} format=\"none\" />\n    <text when={$over} tone=\"good\" emphasis=\"strong\">Time. Stretch, then press Reset.</text>\n    <row>\n      <button label=\"Start\" variant=\"primary\" when={not exists($startedAt)} onPress={set($startedAt, time.now)} />\n      <button label=\"Reset\" variant=\"quiet\" when={exists($startedAt)} onPress={set($startedAt, null)} />\n      <select label=\"Minutes\" value={$minutes} options={[\"15\", \"25\", \"45\"]} />\n    </row>\n    <text tone=\"muted\" size=\"small\">Meanwhile the agent is on {orElse($working.text, 'nothing in particular')}</text>\n  </column>\n</slate>"),
];

fn doc(text: &str) -> SlateDoc {
    let r = parse_slate(text);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    r.document.expect("a document")
}

fn codes(text: &str) -> Vec<String> {
    parse_slate(text).errors.into_iter().map(|e| e.code).collect()
}

fn value_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("serializes")
}

fn wrap(piece: &str, decls: &str) -> String {
    format!("<slate>\n{decls}  <column>\n    {piece}\n  </column>\n</slate>")
}

fn apply(base: &SlateDoc, text: &str) -> SlateApplied {
    let p = parse_slate_patch(text, Some(base));
    assert!(p.errors.is_empty(), "{:?}", p.errors);
    apply_slate_patch(Some(base), &slate_start_values(base), &p.ops.expect("ops"), &p.lines)
}

fn applied_doc(r: &SlateApplied) -> &SlateDoc {
    r.document.as_ref().and_then(|d| d.as_ref()).expect("a document")
}

fn base_doc() -> SlateDoc {
    doc(r#"<slate title="P">
  <value name="steps" start={[{ title: "a", done: false }]} />
  <column>
    <meter id="week" label="Weekly" value={usage.week.percent} />
    <text id="eta">Soon</text>
    <section id="more" title="More"><text id="inner">x</text></section>
  </column>
</slate>"#)
}

fn table_doc() -> SlateDoc {
    doc(r#"<slate title="P">
  <value name="procs" start={[{ name: "node", cpu: 80, mem: 524 }]} />
  <column>
    <table id="ps" items={$procs}>
      <col title="Name" value={item.name} />
      <col title="CPU" value={item.cpu} />
    </table>
  </column>
</slate>"#)
}

#[test]
fn spec_examples_compile_and_validate() {
    for (name, text) in SPEC_EXAMPLES {
        let r = parse_slate(text);
        assert!(r.errors.is_empty(), "{name}: {:?}", r.errors);
        assert_eq!(r.document.as_ref().map(|d| d.schema), Some(2), "{name}");
    }
}

#[test]
fn spec_examples_print_back_to_the_same_document_and_printing_is_idempotent() {
    for (name, text) in SPEC_EXAMPLES {
        let d = doc(text);
        let printed = print_slate(&d);
        let again = parse_slate(&printed);
        assert!(again.errors.is_empty(), "{name}: {:?}", again.errors);
        let back = again.document.expect("a document");
        assert_eq!(back, d, "{name}");
        assert_eq!(print_slate(&back), printed, "{name}");
    }
}

#[test]
fn attributes_compile_to_literals_bindings_formats_and_steps() {
    let d = doc(r#"<slate title="T">
  <value name="n" start={1} />
  <run name="check" cmd='gh api "$URL"' env={{ URL: $url, MODE: "x" }} timeout={20} every="30s" />
  <value name="url" start="" />
  <when change={$url} do={start($check)} />
  <column>
    <meter id="week" label="Weekly" value={usage.week.percent} max={100} note={`resets ${until(usage.week.resetsAt)}`} />
    <text tone="muted">{len(pr.checks)} checks, {'{'}literal{'}'}</text>
    <button label="Go" onPress={[set($n, $n + 1), send("Went.", $n, pr.url)]} />
  </column>
</slate>"#);
    assert_eq!(
        value_of(&d.pieces["week"].props),
        json!({"label": "Weekly", "value": {"bind": "usage.week.percent"}, "max": 100, "note": {"format": "resets ${until(usage.week.resetsAt)}"}})
    );
    assert_eq!(value_of(&d.pieces["text-1"].props.as_ref().unwrap()["value"]), json!({"format": "${len(pr.checks)} checks, {literal}"}));
    assert_eq!(
        value_of(&d.pieces["button-1"].on.as_ref().unwrap()["press"]),
        json!([{"do": "set", "path": "$n", "value": {"bind": "$n + 1"}}, {"do": "send", "text": "Went.", "with": ["$n", "pr.url"]}])
    );
    assert_eq!(
        value_of(&d.runs["check"]),
        json!({"kind": "cmd", "cmd": "gh api \"$URL\"", "env": {"URL": {"bind": "$url"}, "MODE": "x"}, "timeout": 20, "every": 30})
    );
    assert_eq!(value_of(&d.reactions), json!([{"on": {"change": ["$url"]}, "do": [{"do": "start", "run": "check"}]}]));
}

#[test]
fn common_mistakes_name_their_codes_and_lines() {
    let p = parse_slate(&wrap(r#"<text muted>Hi</text>"#, ""));
    assert_eq!(p.errors.iter().map(|e| e.code.as_str()).collect::<Vec<_>>(), vec!["P102"]);
    assert_eq!(p.errors[0].line, Some(3));
    assert_eq!(p.errors[0].fix.as_deref(), Some("tone=\"muted\""));
    assert_eq!(codes(&wrap(r#"<meter label="W" value="usage.week.percent" />"#, "")), vec!["T303"]);
    assert_eq!(codes(&wrap(r#"<text label="Say \"hi\"" />"#, "")), vec!["P100"]);
    assert_eq!(codes(&wrap("<text>{items.map(i => i.name)}</text>", "")), vec!["X420"]);
    assert_eq!(codes(&wrap("<text>{len($failing.length)}</text>", "  <value name=\"failing\" start={[]} />\n")), vec!["X420"]);
    assert!(codes(&wrap("<text when={pr.number != null && pr.draft}>x</text>", "")).is_empty());
    assert!(codes(&wrap(r#"<button label="Go" onClick={send("x")} />"#, "")).contains(&"A602".to_string()));
    assert_eq!(codes(&wrap(r#"<meter label="W" value={1}>"#, "")), vec!["P100"]);
    assert_eq!(codes("<slate><text>a</text><text>b</text></slate>"), vec!["D202"]);
    assert_eq!(codes(&wrap(r#"<button label="Go" onPress={() => set($x, 1)} />"#, "  <value name=\"x\" start={0} />\n")), vec!["A600"]);
    assert_eq!(
        codes(&wrap("<text>x</text>", "  <value name=\"url\" start=\"\" />\n  <run name=\"r\" cmd={`gh api ${$url}`} />\n")),
        vec!["K700"]
    );
    assert_eq!(
        codes(&wrap(
            "<text>x</text>",
            "  <value name=\"id\" start=\"\" />\n  <run name=\"check\" cmd=\"true\" />\n  <when change={$id}>start($check)</when>\n"
        )),
        vec!["A601"]
    );
    assert_eq!(codes(&wrap("<text>Hello {name}</text>", "  <value name=\"name\" start=\"\" />\n")), vec!["X401"]);
    assert_eq!(
        parse_slate(&wrap("<text>Hello {name}</text>", "  <value name=\"name\" start=\"\" />\n")).errors[0].fix.as_deref(),
        Some("$name")
    );
    assert_eq!(codes(&wrap("<text tone={good}>x</text>", "")), vec!["X401"]);
    assert_eq!(codes(&wrap(r#"<meter label="W" value={usage.weekley.percent} colour="red" />"#, "")), vec!["X401", "T312"]);
    let e = &parse_slate(&wrap(r#"<meter label="W" value={usage.weekley.percent} />"#, "")).errors[0];
    assert_eq!(
        (e.code.as_str(), e.piece.as_deref(), e.prop.as_deref(), e.line, e.fix.as_deref()),
        ("X401", Some("meter-1"), Some("value"), Some(3), Some("usage.week.percent"))
    );
}

#[test]
fn every_error_is_collected_in_one_pass() {
    let r = parse_slate("<slate>\n  <column>\n    <meter label=\"A\" value={usage.weekley.percent} />\n    <text tone=\"loud\">x</text>\n    <bogus />\n  </column>\n</slate>");
    let found: Vec<(String, Option<i64>)> = r.errors.iter().map(|e| (e.code.clone(), e.line)).collect();
    assert_eq!(found, vec![("X401".to_string(), Some(3)), ("T306".to_string(), Some(4)), ("T300".to_string(), Some(5))]);
}

#[test]
fn a_patch_sets_a_piece_text_from_a_props_text_child_and_refuses_one_on_a_piece_without_text() {
    let base = base_doc();
    let r = apply(&base, "<props id=\"eta\">Due in {$steps | count} steps</props>");
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(value_of(&applied_doc(&r).pieces["eta"].props), json!({"value": {"format": "Due in ${$steps | count} steps"}}));
    assert_eq!(
        value_of(&applied_doc(&apply(&base, "<props id=\"eta\" tone=\"muted\">Later</props>")).pieces["eta"].props),
        json!({"value": "Later", "tone": "muted"})
    );
    let codes_of = |text: &str| parse_slate_patch(text, Some(&base)).errors.into_iter().map(|e| e.code).collect::<Vec<_>>();
    assert_eq!(codes_of("<props id=\"week\">Hi</props>"), vec!["P105"]);
    assert_eq!(codes_of("<props id=\"eta\" value=\"a\">b</props>"), vec!["T303"]);
}

#[test]
fn a_patch_targets_a_piece_whose_id_is_also_a_runs_name() {
    let d = doc("<slate>\n  <run name=\"spot\" cmd=\"true\" />\n  <column><text id=\"spot\">x</text></column>\n</slate>");
    let p = parse_slate_patch("<props id=\"spot\" tone=\"muted\" />", Some(&d));
    assert!(p.errors.is_empty());
    let r = apply_slate_patch(Some(&d), &slate_start_values(&d), &p.ops.expect("ops"), &p.lines);
    assert_eq!(value_of(&applied_doc(&r).pieces["spot"].props), json!({"value": "x", "tone": "muted"}));
}

#[test]
fn a_patch_merges_props_and_removes_one_with_null() {
    let r = apply(&base_doc(), "<props id=\"week\" tone=\"warning\" note={null} />");
    assert!(r.errors.is_empty());
    assert_eq!(
        value_of(&applied_doc(&r).pieces["week"].props),
        json!({"label": "Weekly", "value": {"bind": "usage.week.percent"}, "tone": "warning"})
    );
}

#[test]
fn a_col_inside_props_merges_into_the_column_with_its_title() {
    let table = table_doc();
    let patch = |text: &str| {
        let p = parse_slate_patch(text, Some(&table));
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let r = apply_slate_patch(Some(&table), &slate_start_values(&table), &p.ops.expect("ops"), &p.lines);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        applied_doc(&r).clone()
    };
    let toned = patch("<props id=\"ps\"><col title=\"CPU\" tone={item.cpu > 50 ? 'bad' : 'default'} /></props>");
    assert_eq!(
        value_of(&toned.pieces["ps"].props.as_ref().unwrap()["columns"]),
        json!([{"title": "Name", "value": {"bind": "item.name"}}, {"title": "CPU", "value": {"bind": "item.cpu"}, "tone": {"bind": "item.cpu > 50 ? 'bad' : 'default'"}}])
    );
    assert!(print_slate(&toned).contains("<col title=\"CPU\" value={item.cpu} tone={item.cpu > 50 ? 'bad' : 'default'} />"));
    let untoned = apply_slate_patch(
        Some(&toned),
        &slate_start_values(&toned),
        &parse_slate_patch(
            "<props id=\"ps\"><col title=\"CPU\" tone={null} /><col title=\"Mem\" value={item.mem} mono /></props>",
            Some(&toned),
        )
        .ops
        .expect("ops"),
        &parse_slate_patch(
            "<props id=\"ps\"><col title=\"CPU\" tone={null} /><col title=\"Mem\" value={item.mem} mono /></props>",
            Some(&toned),
        )
        .lines,
    );
    assert_eq!(
        value_of(&applied_doc(&untoned).pieces["ps"].props.as_ref().unwrap()["columns"]),
        json!([{"title": "Name", "value": {"bind": "item.name"}}, {"title": "CPU", "value": {"bind": "item.cpu"}}, {"title": "Mem", "value": {"bind": "item.mem"}, "mono": true}])
    );
    assert_eq!(codes_for_patch(&table, "<props id=\"ps\"><text>x</text></props>"), vec!["P105"]);
    assert!(parse_slate_patch("<col title=\"CPU\" tone=\"bad\" />", Some(&table)).errors[0]
        .message
        .contains("<props id=\"...\"><col ... /></props>"));
}

fn codes_for_patch(base: &SlateDoc, text: &str) -> Vec<String> {
    parse_slate_patch(text, Some(base)).errors.into_iter().map(|e| e.code).collect()
}

#[test]
fn a_patch_adds_moves_removes_and_replaces_pieces_by_id() {
    let base = base_doc();
    let added = apply(&base, "<add under=\"root\" at={1}><text id=\"new\">New</text></add>");
    assert_eq!(value_of(&applied_doc(&added).pieces["column-1"].children), json!(["week", "new", "eta", "more"]));
    assert_eq!(
        value_of(&applied_doc(&apply(&base, "<move id=\"eta\" under=\"more\" at={0} />")).pieces["more"].children),
        json!(["eta", "inner"])
    );
    let removed = apply(&base, "<remove id=\"more\" />");
    assert!(applied_doc(&removed).pieces.get("more").is_none());
    assert!(applied_doc(&removed).pieces.get("inner").is_none());
    let replaced = apply(&base, "<section id=\"more\" title=\"Less\"><text>y</text></section>");
    assert!(applied_doc(&replaced).pieces.get("inner").is_none());
    assert_eq!(value_of(&applied_doc(&replaced).pieces["more"].children), json!(["text-1"]));
}

#[test]
fn a_patch_adds_declarations_and_keeps_the_live_values_both_documents_declare() {
    let base = base_doc();
    let mut live = slate_start_values(&base);
    live.insert("steps".to_string(), json!([{"title": "a", "done": true}]));
    let text =
        "<value name=\"note\" start=\"\" />\n<run name=\"check\" cmd=\"true\" />\n<when id=\"w\" change={$note} do={start($check)} />";
    let p = parse_slate_patch(text, Some(&base));
    let r = apply_slate_patch(Some(&base), &live, &p.ops.expect("ops"), &p.lines);
    assert!(r.errors.is_empty(), "{:?}", r.errors);
    assert_eq!(
        value_of(&r.values.clone().expect("values")),
        json!({"steps": [{"title": "a", "done": true}], "note": "", "check": {"state": "idle", "runs": 0}})
    );
    assert_eq!(
        value_of(&applied_doc(&r).reactions),
        json!([{"id": "w", "on": {"change": ["$note"]}, "do": [{"do": "start", "run": "check"}]}])
    );
}

#[test]
fn a_patch_refuses_a_reaction_without_an_id_a_result_that_fails_validation_and_holds_undo_for_the_host() {
    let base = base_doc();
    assert_eq!(parse_slate_patch("<when change={$steps} do={set($steps, [])} />", Some(&base)).errors[0].code, "P107");
    let bad = apply(&base, "<props id=\"week\" value={usage.weekley.percent} />");
    assert!(bad.document.is_none());
    assert_eq!(bad.errors[0].code, "X401");
    assert_eq!(apply(&base, "<remove name=\"steps\" />\n<text id=\"eta\">{len($steps)}</text>").errors[0].code, "S501");
    let cleared = apply(&base, "<clear />");
    assert_eq!(cleared.document, Some(None));
    assert_eq!(apply(&base, "<undo />").errors[0].code, "V752");
}

#[test]
fn json_sent_as_text_or_as_a_document_says_a_slate_is_jsx_like_text_and_shows_one() {
    let as_text = parse_slate("{\"title\": \"Gold\", \"pieces\": []}");
    let e = &as_text.errors[0];
    assert_eq!(e.code, "P100");
    assert_eq!(e.message, "this is JSON, and a slate is JSX-like text that starts with <slate>; slate_catalog shows it");
    assert_eq!(e.fix.as_deref(), Some("<slate title=\"Gold\"><number label=\"Spot\" value={$spot.json.usd} unit=\"USD\" /></slate>"));
    let as_doc = validate_slate(&json!({"title": "Gold", "sections": []}));
    assert_eq!(as_doc.errors[0].code, "D200");
    assert_eq!(
        as_doc.errors[0].message,
        "this JSON is not a slate: a slate is the JSX-like text slate_catalog shows, sent as text, never JSON of your own"
    );
    let version = validate_slate(&json!({"schema": 3}));
    assert_eq!((version.errors[0].code.as_str(), version.errors[0].fix.as_deref()), ("D200", Some("update wsp")));
    let wrapped = validate_slate(&json!({"slate": "<slate><text>x</text></slate>"}));
    assert_eq!(wrapped.errors[0].code, "D200");
    assert_eq!(wrapped.errors[0].message, "this JSON wraps slate markup; send the markup itself as text, not inside document");
}

#[test]
fn an_escaped_quote_in_an_attribute_string_shows_the_quoting_that_works() {
    let refused = parse_slate("<slate><run name=\"p\" cmd=\"python3 -c \\\"print(1)\\\"\" /><text>x</text></slate>");
    let e = &refused.errors[0];
    assert_eq!(e.code, "P100");
    assert_eq!(e.message, "attribute strings take no escapes (cmd at line 1): a backslash does not hide a \" inside \"...\"");
    assert_eq!(e.fix.as_deref(), Some("cmd='echo \"hi\"', single quotes outside the double ones; with both quotes inside, the block form <run name=\"x\">{`...`}</run>"));
    assert!(parse_slate("<slate><run name=\"p\" cmd='python3 -c \"print(1)\"' /><text>x</text></slate>").errors.is_empty());
    let entity = parse_slate("<slate><run name=\"p\" cmd=\"python3 &quot;$SLATE_DIR/x.py&quot;\" /><text>x</text></slate>");
    assert_eq!(entity.errors[0].code, "P100");
    assert_eq!(
        entity.errors[0].message,
        "attribute strings take no HTML entities (cmd at line 1): &quot; stays as written, nothing decodes it"
    );
    assert_eq!(entity.errors[0].fix.as_deref(), Some("cmd='python3 \"$SLATE_DIR/x.py\"', single quotes outside the double ones"));
    let amp = parse_slate("<slate><text value=\"Q&amp;A\" /></slate>");
    assert_eq!(amp.errors[0].code, "P100");
    assert_eq!(amp.errors[0].fix.as_deref(), Some("write the character itself: value=\"... & ...\""));
    assert!(parse_slate("<slate><text value=\"spot & retail\" /></slate>").errors.is_empty());
}

#[test]
fn a_printed_slate_prints_a_run_a_piece_names_as_a_formula_and_reads_it_back() {
    let d = doc("<slate><run name=\"tests\" cmd=\"echo\" /><output run={$tests} /></slate>");
    let printed = print_slate(&d);
    assert!(printed.contains("run={$tests}"), "{printed}");
    assert_eq!(doc(&printed).pieces, d.pieces);
}

#[test]
fn a_dollar_written_before_a_hole_stays_a_dollar_sign() {
    let written = doc("<slate><value name=\"price\" start={12} /><text>Price: ${$price}</text></slate>");
    let text = written
        .pieces
        .values()
        .find(|p| p.kind == "text")
        .and_then(|p| p.props.as_ref())
        .and_then(|p| p.get("value"))
        .expect("a text value")
        .clone();
    let ctx = SlateEvalContext {
        now: 0,
        item: None,
        index: None,
        resolve: Rc::new(|p: &str| if p == "$price" { Some(json!(12)) } else { None }),
    };
    assert_eq!(resolve_slate_prop(&text, &ctx), Some(json!("Price: $12")));
    assert_eq!(doc(&print_slate(&written)), written);
}

#[test]
fn a_run_description_is_one_literal_line_on_a_tool_or_resource_run_and_trimmed() {
    let run = |attrs: &str| parse_slate(&format!("<slate><run name=\"r\" {attrs} /><column><output run={{$r}} /></column></slate>"));
    let tool = run("tool=\"cf.execute\" description=\"  Reads the last hour of traffic \"");
    let r = value_of(&tool.document.expect("a document").runs["r"]);
    assert_eq!((r["kind"].clone(), r["description"].clone()), (json!("tool"), json!("Reads the last hour of traffic")));
    let resource = run("resource=\"cf:cf://zones\" description=\"Reads the zones\"");
    let r = value_of(&resource.document.expect("a document").runs["r"]);
    assert_eq!((r["kind"].clone(), r["description"].clone()), (json!("resource"), json!("Reads the zones")));
    let cmd = run("cmd=\"true\" description=\"Does nothing\"");
    assert_eq!(
        cmd.errors.iter().map(|e| (e.code.clone(), e.message.clone())).collect::<Vec<_>>(),
        vec![("K701".to_string(), "a cmd run takes no description: its sheet shows the command itself".to_string())]
    );
    assert_eq!(
        run("tool=\"cf.execute\" description={$x}").errors.iter().map(|e| e.code.clone()).collect::<Vec<_>>(),
        vec!["T303".to_string()]
    );
    assert_eq!(
        run("tool=\"cf.execute\" description=\" \"").errors.iter().map(|e| e.code.clone()).collect::<Vec<_>>(),
        vec!["T303".to_string()]
    );
}

fn json_doc(pieces: Value, extra: Value) -> SlateDoc {
    let children: Vec<String> = pieces.as_object().expect("pieces").keys().cloned().collect();
    let mut all = json!({"schema": 2, "root": "top", "values": {"v": {"start": 1}}, "derived": {}, "runs": {}, "reactions": [], "pieces": {"top": {"type": "column", "children": children}}});
    for (k, v) in pieces.as_object().expect("pieces") {
        all["pieces"][k] = v.clone();
    }
    for (k, v) in extra.as_object().expect("extra") {
        all[k] = v.clone();
    }
    validate_slate(&all).document.expect("a valid document")
}

#[test]
fn printing_and_parsing_again_gives_back_the_same_document_for_quotes_control_characters_entities_and_backticks() {
    let docs = vec![
        json_doc(json!({"t": {"type": "text", "props": {"value": "x"}}}), json!({"title": "He said \"hi\" and 'bye'"})),
        json_doc(
            json!({"b": {"type": "button", "props": {"label": "Go"}, "on": {"press": [{"do": "send", "text": "a\rb\u{1}c"}]}}}),
            json!({}),
        ),
        json_doc(json!({"s": {"type": "section", "props": {"title": "Q&amp;A"}}}), json!({"values": {"v": {"start": "a &amp; b"}}})),
        json_doc(json!({"s": {"type": "section", "props": {"title": "x", "note": {"format": "a `b` ${$v} $${c}"}}}}), json!({})),
        json_doc(
            json!({"o": {"type": "output", "props": {"run": "$r"}}}),
            json!({"runs": {"r": {"kind": "cmd", "cmd": "echo '&amp;' \"x\""}}}),
        ),
        json_doc(
            json!({"o": {"type": "output", "props": {"run": "$r"}}}),
            json!({"runs": {"r": {"kind": "tool", "server": "cf", "tool": "execute", "args": {"code": "1"}, "description": "Reads the \"last\" hour"}, "s": {"kind": "resource", "server": "cf", "uri": "cf://z", "description": "Reads the zones"}}}),
        ),
    ];
    for d in docs {
        let printed = print_slate(&d);
        let back = parse_slate(&printed);
        assert!(back.errors.is_empty(), "{printed}: {:?}", back.errors);
        assert_eq!(back.document.expect("a document"), d, "{printed}");
    }
}

const TRAFFIC: &str = r#"<slate title="Traffic">
  <run name="traffic" cmd='python3 "$SLATE_DIR/traffic.py"' every={60} />
  <column>
    <number id="hits" label="Hits today" value={$traffic.json.hits} unit="hits" />
  </column>
  <file name="traffic.py">{`
    import json, re
    hits = {"hits": 0}
    for line in open("access.log"):
        if re.search(r"GET /", line):
            hits["hits"] += 1
    print(json.dumps(hits))
  `}</file>
</slate>"#;

const PY: &str = "import json, re\nhits = {\"hits\": 0}\nfor line in open(\"access.log\"):\n    if re.search(r\"GET /\", line):\n        hits[\"hits\"] += 1\nprint(json.dumps(hits))";

#[test]
fn a_slates_files_declare_code_as_written_with_indentation_and_braces_kept() {
    let d = doc(TRAFFIC);
    let files = d.files.clone().expect("files");
    assert_eq!(files.get("traffic.py").map(String::as_str), Some(PY));
    assert_eq!(files.len(), 1);
    assert_eq!(d.pieces.values().map(|p| p.kind.as_str()).collect::<Vec<_>>(), vec!["column", "number"]);
}

#[test]
fn a_slates_files_print_back_to_the_same_document() {
    let d = doc(TRAFFIC);
    let printed = print_slate(&d);
    assert!(printed.contains("  <file name=\"traffic.py\">{`\n    import json, re\n"), "{printed}");
    assert_eq!(doc(&printed), d);
    let mut raw = d.clone();
    raw.files = Some([("q.sh".to_string(), "echo `date`".to_string())].into_iter().collect());
    assert_eq!(doc(&print_slate(&raw)).files.expect("files").get("q.sh").map(String::as_str), Some("echo `date`"));
}

#[test]
fn a_file_is_replaced_by_name_in_a_patch_and_removed_with_remove_name() {
    let d = doc(TRAFFIC);
    let values = slate_start_values(&d);
    let next = parse_slate_patch("<file name=\"traffic.py\">{`print(1)`}</file>", Some(&d));
    assert!(next.errors.is_empty());
    assert_eq!(value_of(&next.ops.clone().expect("ops")), json!([{"op": "file", "name": "traffic.py", "text": "print(1)"}]));
    let replaced = apply_slate_patch(Some(&d), &values, &next.ops.expect("ops"), &next.lines);
    assert!(replaced.errors.is_empty());
    assert_eq!(applied_doc(&replaced).files.clone().expect("files").get("traffic.py").map(String::as_str), Some("print(1)"));
    let added_ops = parse_slate_patch("<file name=\"shape.py\">{`print(2)`}</file>", Some(&d)).ops.expect("ops");
    let added = apply_slate_patch(Some(&d), &values, &added_ops, &Value::Null);
    assert_eq!(applied_doc(&added).files.clone().expect("files").keys().cloned().collect::<Vec<_>>(), vec!["traffic.py", "shape.py"]);
    let gone = parse_slate_patch("<remove name=\"traffic.py\" />", Some(&d));
    assert_eq!(value_of(&gone.ops.clone().expect("ops")), json!([{"op": "file", "name": "traffic.py", "text": null}]));
    let removed = apply_slate_patch(Some(&d), &values, &gone.ops.expect("ops"), &gone.lines);
    assert!(applied_doc(&removed).files.is_none());
    assert_eq!(removed.warnings.iter().map(|w| w.code.clone()).collect::<Vec<_>>(), vec!["W015".to_string()]);
    assert_eq!(codes_for_patch(&d, "<remove name=\"nope.py\" />"), vec!["D203"]);
}

#[test]
fn a_hostile_name_the_object_prototype_owns_is_an_unknown_name_and_never_a_function() {
    let doc_v = parse_slate("<slate><value name=\"v\" start={{ a: 1 }} /><text>{$v.constructor}</text></slate>");
    assert!(doc_v.document.is_some(), "{:?}", doc_v.errors);
    assert!(codes("<slate><text>{toString(1)}</text></slate>").contains(&"X404".to_string()));
    assert!(!parse_slate("<slate><constructor /></slate>").errors[0].message.contains("function"));
    for name in ["constructor", "toString", "hasOwnProperty", "__proto__"] {
        let r = parse_slate(&format!("<slate><text {name}=\"a\">x</text></slate>"));
        assert!(!r.errors.is_empty(), "{name}");
    }
    assert!(codes("<slate><text>x</text></slate>").is_empty());
}

#[test]
fn a_huge_or_too_deep_text_is_refused_by_its_limit_before_it_is_read() {
    let pieces = (0..20_000).map(|_| "<text tone=\"muted\">line</text>").collect::<Vec<_>>().join("\n");
    let over = compile_slate_text(&format!("<slate>\n<column>\n{pieces}\n</column>\n</slate>"));
    assert_eq!(over.errors.iter().map(|e| e.code.clone()).collect::<Vec<_>>(), vec!["D208".to_string()]);
    let deep = format!("<slate>{}<text>x</text>{}</slate>", "<column>".repeat(3_000), "</column>".repeat(3_000));
    assert_eq!(compile_slate_text(&deep).errors.iter().map(|e| e.code.clone()).collect::<Vec<_>>(), vec!["D206".to_string()]);
}

#[test]
fn a_literal_that_does_not_read_names_the_text_it_stopped_at_in_utf16_units() {
    let r = parse_slate("<slate><value name=\"v\" start={😀😀😀😀😀😀😀😀😀😀} /><column><text>x</text></column></slate>");
    let e = &r.errors[0];
    assert_eq!(e.code, "S503");
    assert_eq!(
        e.message,
        format!(
            "start takes a literal (a string, a number, true, false, null, a list or a record of literals): not a literal: \"{}\"",
            "😀".repeat(8)
        )
    );
}

#[test]
fn a_record_literal_keeps_javascript_key_order_with_index_keys_first() {
    let d = doc(
        "<slate><value name=\"v\" start={{ \"2\": 1, b: 2, \"10\": 3, \"01\": 4, \"1\": 5 }} /><column><text>x</text></column></slate>",
    );
    let keys: Vec<&str> = d.values["v"].start.as_object().expect("a record").keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["1", "2", "10", "b", "01"]);
    let bare = parse_slate("<slate><value name=\"v\" start={{ 10: 3 }} /><column><text>x</text></column></slate>");
    assert_eq!(bare.errors[0].code, "S503");
    assert_eq!(
        bare.errors[0].message,
        "start takes a literal (a string, a number, true, false, null, a list or a record of literals): a key at \"10: 3 }\""
    );
}

#[test]
fn the_printed_form_of_a_document_with_a_title_is_the_same_on_a_second_print() {
    let d = doc("<slate title=\"T\"><column><text>x</text></column></slate>");
    let printed = print_slate(&d);
    assert_eq!(printed, "<slate title=\"T\">\n  <column id=\"column-1\">\n    <text id=\"text-1\">x</text>\n  </column>\n</slate>");
    assert_eq!(print_slate(&doc(&printed)), printed);
}
