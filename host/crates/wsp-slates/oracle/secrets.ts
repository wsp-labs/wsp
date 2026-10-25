function secretForms(text: string): string[] {
  const forms = new Set<string>([text]);
  const bytes = Buffer.from(text, "utf8");
  const b64 = (b: Buffer): string[] => { const std = b.toString("base64"); const url = std.replace(/\+/g, "-").replace(/\//g, "_"); return [std, std.replace(/=+$/, ""), url, url.replace(/=+$/, "")]; };
  for (const f of b64(bytes)) forms.add(f);
  for (const shift of [0, 1, 2]) {
    const all = Buffer.concat([Buffer.alloc(shift), bytes]).toString("base64");
    const from = shift === 0 ? 0 : 4;
    const to = Math.floor((shift + bytes.length) / 3) * 4;
    const middle = all.slice(from, to);
    if (middle.length < 8) continue;
    forms.add(middle);
    forms.add(middle.replace(/\+/g, "-").replace(/\//g, "_"));
  }
  const url = encodeURIComponent(text);
  forms.add(url); forms.add(url.replace(/%[0-9A-F]{2}/g, m => m.toLowerCase())); forms.add(url.replace(/%20/g, "+"));
  forms.delete("");
  return [...forms];
}
function scrub(names: Map<string, string>, text: string): string {
  if (text === "") return text;
  const forms: [string, string][] = [];
  for (const [name, k] of names) for (const f of secretForms(k)) forms.push([f, `[secret:${name}]`]);
  forms.sort((a, b) => b[0].length - a[0].length);
  let out = text;
  for (const [form, mark] of forms) if (out.includes(form)) out = out.split(form).join(mark);
  return out;
}
const p = (l: string, v: unknown) => console.log(`${l} => ${JSON.stringify(v)}`);
p("scrub.nested", scrub(new Map([["s", "secret"], ["t", "cret"]]), "a secret"));
p("scrub.markword", scrub(new Map([["x", "secret"]]), "secret secret"));
p("scrub.plus", scrub(new Map([["sp", "a b"]]), "a b, a+b, a%20b, YSBi"));
p("scrub.basic", scrub(new Map([["token", "vrcl_Ab+9/zz&q=1 ünï_0123456789"]]), Buffer.from("user:vrcl_Ab+9/zz&q=1 ünï_0123456789").toString("base64")));
p("scrub.tie", scrub(new Map([["a", "abcd"], ["b", "wxyz"]]), "abcd wxyz"));
try { p("forms.lone", secretForms("a\ud800b")); } catch (e) { p("forms.lone.throws", String(e)); }
p("forms.middle", secretForms("0123456789abcdef"));
p("json.parse", [JSON.parse(" {\"a\":1}\n"), (() => { try { return JSON.parse(""); } catch (e) { return String(e).slice(0, 40); } })()]);
p("firstline", "  not json\nmore".trim().split("\n")[0]!.slice(0, 120));
