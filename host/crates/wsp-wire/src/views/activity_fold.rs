// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/activity-fold.ts at a3b870a6e: one fold walk of a group's processes, which the
//! Activity page and `wsp activity` both draw. See spec/activity.md.

use crate::views::activity::ActivityProc;
use std::collections::HashMap;
use wsp_frames::ProcEntry;

/// A row with a pid and a parent pid, which is all the walk reads.
pub trait PidRow {
    fn pid(&self) -> i64;
    fn ppid(&self) -> i64;
}

impl PidRow for ActivityProc {
    fn pid(&self) -> i64 {
        self.pid
    }
    fn ppid(&self) -> i64 {
        self.ppid
    }
}

impl PidRow for ProcEntry {
    fn pid(&self) -> i64 {
        i64::from(self.pid)
    }
    fn ppid(&self) -> i64 {
        i64::from(self.ppid)
    }
}

// views/activity-fold.ts:7 at a3b870a6e
// export interface Folded<P> {
//   readonly p: P;
//   /** The rows this one holds, whose figures join its own. */
//   readonly held: readonly P[];
//   readonly kids: readonly Folded<P>[];
// }
#[derive(Clone, Debug, PartialEq)]
pub struct Folded<P> {
    pub p: P,
    /// The rows this one holds, whose figures join its own, in the list's order.
    pub held: Vec<P>,
    pub kids: Vec<Folded<P>>,
}

// views/activity-fold.ts:16 at a3b870a6e
// export function foldTree<P extends { readonly pid: number; readonly ppid: number }>(list: readonly P[], foldsInto: (pid: number) => number | undefined): Folded<P>[] {
//   const byPid = new Map(list.map(p => [p.pid, p]));
//   const into = (pid: number): number | undefined => {
//     const at = foldsInto(pid);
//     return at !== undefined && at !== pid && byPid.has(at) ? at : undefined;
//   };
//   const up = (p: P): P | undefined => (p.ppid !== p.pid ? byPid.get(p.ppid) : undefined);
//   /** The first parent that stands as a row: a folded parent stands as the row it folds into, unless that is this row. */
//   const parentOf = (p: P): number | undefined => {
//     let at = up(p);
//     for (let n = 0; at !== undefined && n <= list.length; n++) {
//       const target = into(at.pid);
//       if (target === undefined) return at.pid;
//       if (target !== p.pid) return target;
//       at = up(at);
//     }
//     return undefined;
//   };
//   const kids = new Map<number, P[]>();
//   const held = new Map<number, P[]>();
//   const heads: P[] = [];
//   for (const p of list) {
//     const target = into(p.pid);
//     if (target !== undefined) (held.get(target) ?? held.set(target, []).get(target)!).push(p);
//     else {
//       const parent = parentOf(p);
//       if (parent === undefined) heads.push(p);
//       else (kids.get(parent) ?? kids.set(parent, []).get(parent)!).push(p);
//     }
//   }
//   const build = (p: P): Folded<P> => ({ p, held: held.get(p.pid) ?? [], kids: (kids.get(p.pid) ?? []).map(build) });
//   return heads.map(build);
// }
/// The list's tree, in the list's order. `folds_into` names the row a process folds into; a target outside the list,
/// or the pid itself, folds nothing.
pub fn fold_tree<P: PidRow + Clone>(list: &[P], folds_into: impl Fn(i64) -> Option<i64>) -> Vec<Folded<P>> {
    let by_pid: HashMap<i64, &P> = list.iter().map(|p| (p.pid(), p)).collect();
    let into = |pid: i64| folds_into(pid).filter(|&at| at != pid && by_pid.contains_key(&at));
    let up = |p: &P| if p.ppid() != p.pid() { by_pid.get(&p.ppid()).copied() } else { None };
    let parent_of = |p: &P| -> Option<i64> {
        let mut at = up(p);
        for _ in 0..=list.len() {
            let a = at?;
            match into(a.pid()) {
                None => return Some(a.pid()),
                Some(target) if target != p.pid() => return Some(target),
                Some(_) => at = up(a),
            }
        }
        None
    };
    let mut kids: HashMap<i64, Vec<&P>> = HashMap::new();
    let mut held: HashMap<i64, Vec<&P>> = HashMap::new();
    let mut heads: Vec<&P> = Vec::new();
    for p in list {
        match into(p.pid()) {
            Some(target) => held.entry(target).or_default().push(p),
            None => match parent_of(p) {
                None => heads.push(p),
                Some(parent) => kids.entry(parent).or_default().push(p),
            },
        }
    }
    fn build<P: PidRow + Clone>(p: &P, held: &HashMap<i64, Vec<&P>>, kids: &HashMap<i64, Vec<&P>>) -> Folded<P> {
        Folded {
            p: p.clone(),
            held: held.get(&p.pid()).map(|rows| rows.iter().map(|r| (*r).clone()).collect()).unwrap_or_default(),
            kids: kids.get(&p.pid()).map(|rows| rows.iter().map(|r| build(*r, held, kids)).collect()).unwrap_or_default(),
        }
    }
    heads.iter().map(|p| build(*p, &held, &kids)).collect()
}
