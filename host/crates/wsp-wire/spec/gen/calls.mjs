// calls.mjs <ts files...>: each exported function, constant or TS type and the outside names it reaches, transitively
// through the file's own declarations. Prints JSON.
const { default: ts } = await import(`${process.env.WSP_REPO}/node_modules/.pnpm/typescript@6.0.3/node_modules/typescript/lib/typescript.js`);
import fs from "node:fs";

const result = {};
for (const file of process.argv.slice(2)) {
  const text = fs.readFileSync(file, "utf8");
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true);
  const imported = new Map();
  const decls = new Map();
  for (const st of sf.statements) {
    if (ts.isImportDeclaration(st)) {
      const from = st.moduleSpecifier.text;
      const nb = st.importClause?.namedBindings;
      if (nb && ts.isNamedImports(nb)) for (const el of nb.elements) imported.set(el.name.text, from);
      continue;
    }
    const names = [];
    if (ts.isFunctionDeclaration(st) && st.name) names.push(st.name.text);
    else if (ts.isVariableStatement(st)) { for (const d of st.declarationList.declarations) if (ts.isIdentifier(d.name)) names.push(d.name.text); }
    else if ((ts.isInterfaceDeclaration(st) || ts.isTypeAliasDeclaration(st) || ts.isEnumDeclaration(st)) && st.name) names.push(st.name.text);
    const exported = st.modifiers?.some(m => m.kind === ts.SyntaxKind.ExportKeyword) ?? false;
    const kind = ts.isFunctionDeclaration(st) ? "fn" : ts.isVariableStatement(st) ? "const" : "type";
    const line = sf.getLineAndCharacterOfPosition(st.getStart()).line + 1;
    const endLine = sf.getLineAndCharacterOfPosition(st.getEnd()).line + 1;
    for (const n of names) decls.set(n, { node: st, exported, kind, line, endLine });
  }
  const direct = new Map();
  for (const [n, d] of decls) {
    const refs = new Set();
    const visit = node => {
      if (ts.isIdentifier(node) && node.text !== n) refs.add(node.text);
      ts.forEachChild(node, visit);
    };
    visit(d.node);
    direct.set(n, refs);
  }
  const out = {};
  for (const [n, d] of decls) {
    if (!d.exported) continue;
    const seen = new Set([n]);
    const outside = new Set();
    const local = new Set();
    const stack = [n];
    while (stack.length) {
      const at = stack.pop();
      for (const r of direct.get(at) ?? []) {
        if (imported.has(r)) outside.add(`${imported.get(r)}:${r}`);
        else if (decls.has(r) && !seen.has(r)) { seen.add(r); local.add(r); stack.push(r); }
      }
    }
    const isZod = d.kind === "const" && /=\s*z\./.test(text.slice(d.node.getStart(), d.node.getEnd())) || /\.(extend|partial|pick|omit)\(/.test(text.slice(d.node.getStart(), d.node.getEnd())) && d.kind === "const";
    const init = ts.isVariableStatement(d.node) ? d.node.declarationList.declarations.find(x => x.name.text === n)?.initializer : undefined;
    const isFn = d.kind === "fn" || (init !== undefined && (ts.isArrowFunction(init) || ts.isFunctionExpression(init)));
    out[n] = { kind: isFn ? "fn" : isZod ? "zod" : d.kind, line: d.line, endLine: d.endLine, outside: [...outside].sort(), local: [...local].sort() };
  }
  result[file] = out;
}
console.log(JSON.stringify(result, null, 1));
