// SPDX-License-Identifier: AGPL-3.0-only
// A group's processes as Activity draws them: each under its parent, the rows
// an agent's own row holds (its run script, the shells on the way down, the
// input pump) held by that row, and what they started hung under it. The page
// and `wsp activity` walk one tree; each says which rows fold into which.

export interface Folded<P> {
  readonly p: P;
  /** The rows this one holds, whose figures join its own. */
  readonly held: readonly P[];
  readonly kids: readonly Folded<P>[];
}

/** The list's tree, in the list's order. `foldsInto` names the row a process folds into; a target outside the list
 * folds nothing. */
export function foldTree<P extends { readonly pid: number; readonly ppid: number }>(list: readonly P[], foldsInto: (pid: number) => number | undefined): Folded<P>[] {
  const byPid = new Map(list.map(p => [p.pid, p]));
  const into = (pid: number): number | undefined => {
    const at = foldsInto(pid);
    return at !== undefined && at !== pid && byPid.has(at) ? at : undefined;
  };
  const up = (p: P): P | undefined => (p.ppid !== p.pid ? byPid.get(p.ppid) : undefined);
  /** The first parent that stands as a row: a folded parent stands as the row it folds into, unless that is this row. */
  const parentOf = (p: P): number | undefined => {
    let at = up(p);
    for (let n = 0; at !== undefined && n <= list.length; n++) {
      const target = into(at.pid);
      if (target === undefined) return at.pid;
      if (target !== p.pid) return target;
      at = up(at);
    }
    return undefined;
  };
  const kids = new Map<number, P[]>();
  const held = new Map<number, P[]>();
  const heads: P[] = [];
  for (const p of list) {
    const target = into(p.pid);
    if (target !== undefined) (held.get(target) ?? held.set(target, []).get(target)!).push(p);
    else {
      const parent = parentOf(p);
      if (parent === undefined) heads.push(p);
      else (kids.get(parent) ?? kids.set(parent, []).get(parent)!).push(p);
    }
  }
  const build = (p: P): Folded<P> => ({ p, held: held.get(p.pid) ?? [], kids: (kids.get(p.pid) ?? []).map(build) });
  return heads.map(build);
}
