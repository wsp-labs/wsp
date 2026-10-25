// SPDX-License-Identifier: AGPL-3.0-only
//! The servers a turn of an agent gets in a folder, read off one Host by the catalog's resolver, keyed by the folder's
//! checkout as git answers it on that Host. Port of packages/collect/src/detect/turn-servers.ts at a3b870a6e; the
//! spec is spec/detect_turn_servers.md.
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::mcp_launch::{Checkout, TurnServer};

use crate::host::Host;

/// The TS options object `{ checkout?, own? }` (turn-servers.ts:15).
#[derive(Debug, Clone, Default)]
pub struct TurnRead {
    /// The folder's checkout, read off the Host where it is not handed in.
    pub checkout: Option<Checkout>,
    /// Read the agent's own file alone, keyed by the folder itself.
    pub own: bool,
}

/// The folder's checkout on that Host: `git <checkout_args(folder)>` read by `checkout_of`.
// detect/turn-servers.ts:8
// /** The folder's checkout on that Host: its top, and the main checkout both agents key a turn's servers by. */
// export const readCheckout = async (host: Pick<Host, "exec">, folder: string): Promise<Checkout> => checkoutOf(await host.exec.run("git", checkoutArgs(folder)), folder);
pub async fn read_checkout(host: &Host, folder: &str) -> Checkout {
    todo!()
}

/// Every server a turn of the agent in `folder` gets there, by scope, read off the files that Host holds. The agent is
/// TS `McpAgent` (an entry with `mcp` set); the resolver's TurnAgent is its id and its `mcp` format, files and
/// project files. An entry with no `mcp` gets no servers.
// detect/turn-servers.ts:11
// /** Every server a turn of the agent in `folder` gets there, by scope, read off the files that Host holds. `checkout` is
//  * the folder's, read off the Host where it is not handed in; `own` reads the agent's own file alone, keyed by the folder
//  * itself. The first of the own file's candidates that is there is the one, as the agent reads it, so none after it is
//  * read. */
// export async function readTurnServers(host: Pick<Host, "home" | "stores" | "fs" | "exec">, agent: McpAgent, folder: string, o: { checkout?: Checkout; own?: true } = {}): Promise<TurnServer[]> {
//   const checkout = o.checkout ?? (o.own === true ? { key: folder } : await readCheckout(host, folder));
//   const files = turnServerFiles(agent, folder, host.home, host.stores?.[agent.id], checkout.top);
//   const read = async (path: string): Promise<TurnFile | undefined> => {
//     const text = await host.fs.readText(path);
//     return text === undefined ? undefined : { path, text };
//   };
//   let user: TurnFile | undefined;
//   for (const path of files.user) if ((user = await read(path)) !== undefined) break;
//   const projects = o.own === true ? [] : (await Promise.all(files.projects.map(read))).filter(f => f !== undefined);
//   return turnServers(agent, { ...(user !== undefined ? { user } : {}), projects, folder, key: checkout.key });
// }
pub async fn read_turn_servers(host: &Host, agent: &AgentEntry, folder: &str, o: TurnRead) -> Vec<TurnServer> {
    todo!()
}
