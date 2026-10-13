// SPDX-License-Identifier: AGPL-3.0-only
import { type FragmentDefinitionNode, type OperationDefinitionNode, parse, type SelectionSetNode, valueFromASTUntyped } from "graphql";

/** One field at the root of a mutation, by its schema name (an alias proves nothing), with its arguments read
 * through the request's variables so a literal and a `$input` judge the same. */
export interface RootField {
  name: string;
  args: Record<string, unknown>;
}

export type GraphqlVerdict = { kind: "read" } | { kind: "write"; fields: RootField[] } | { kind: "refused"; said: string };

const malformed = (why: string): GraphqlVerdict => ({ kind: "refused", said: `wsp: ${why}, so a cloud thread cannot send it through gh.` });

/** What one GraphQL request body asks GitHub to do, read by parsing the document: gh's documents often open with a
 * fragment and GitHub runs fragment spreads at the root, so neither the first word nor the operation name says
 * whether a request writes. */
export function judgeGraphql(body: string): GraphqlVerdict {
  let request: unknown;
  try {
    request = JSON.parse(body);
  } catch {
    return malformed("this GraphQL request is not JSON");
  }
  if (Array.isArray(request)) return malformed("a batch of GraphQL requests is one request per write to judge");
  if (typeof request !== "object" || request === null || typeof (request as { query?: unknown }).query !== "string") return malformed("this GraphQL request carries no document");
  const { query, variables } = request as { query: string; variables?: unknown };
  let document;
  try {
    document = parse(query);
  } catch {
    return malformed("this GraphQL document does not parse");
  }
  const operations = document.definitions.filter((d): d is OperationDefinitionNode => d.kind === "OperationDefinition");
  const fragments = new Map(document.definitions.filter((d): d is FragmentDefinitionNode => d.kind === "FragmentDefinition").map(d => [d.name.value, d]));
  if (operations.length !== 1) return malformed(`this GraphQL document holds ${operations.length} operations, and only a document of one is judged`);
  const operation = operations[0]!;
  const vars = typeof variables === "object" && variables !== null && !Array.isArray(variables) ? (variables as Record<string, unknown>) : {};
  const roots: RootField[] = [];
  const onMutation: string[] = [];
  // A fragment spread again adds the same fields, and walking it again doubles per level: 24 levels in 1 KB held 16 s.
  const expanded = new Set<string>();
  const expand = (set: SelectionSetNode, seen: ReadonlySet<string>): string | undefined => {
    for (const selection of set.selections) {
      if (selection.kind === "Field") {
        const args: Record<string, unknown> = {};
        for (const arg of selection.arguments ?? []) args[arg.name.value] = valueFromASTUntyped(arg.value, vars);
        roots.push({ name: selection.name.value, args });
        continue;
      }
      const fragment = selection.kind === "FragmentSpread" ? fragments.get(selection.name.value) : selection;
      if (fragment === undefined) return `the fragment ${selection.kind === "FragmentSpread" ? selection.name.value : ""} is spread but never defined`;
      if (fragment.typeCondition?.name.value === "Mutation") onMutation.push(fragment.kind === "FragmentDefinition" ? fragment.name.value : "an inline fragment");
      const name = fragment.kind === "FragmentDefinition" ? fragment.name.value : undefined;
      if (name !== undefined && seen.has(name)) return `the fragment ${name} spreads itself`;
      if (name !== undefined && expanded.has(name)) continue;
      if (name !== undefined) expanded.add(name);
      const why = expand(fragment.selectionSet, name === undefined ? seen : new Set([...seen, name]));
      if (why !== undefined) return why;
    }
    return undefined;
  };
  const why = expand(operation.selectionSet, new Set());
  if (why !== undefined) return malformed(why);
  if (operation.operation === "query") {
    if (onMutation.length > 0) return malformed(`this query spreads ${onMutation[0]}, a fragment on Mutation`);
    return { kind: "read" };
  }
  if (operation.operation === "subscription") return malformed("GitHub serves no subscriptions");
  return { kind: "write", fields: roots.filter(field => field.name !== "__typename") };
}
