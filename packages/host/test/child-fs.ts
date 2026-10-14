// SPDX-License-Identifier: AGPL-3.0-only
// The folder reads run in a node child of their own, which no vi.mock reaches, so a test changes that child's fs
// through a preload in NODE_OPTIONS, which the child inherits. `refuse` refuses one folder's listing as a Mac refuses
// a folder it will not let wsp read; `guard` holds forever on any read of what a folder holds, its listing or a path
// inside it, as macOS does on a guarded folder while its permission prompt waits; `print` writes a line of its own to
// stdout as the first asks come in.

export interface ChildFs {
  refuse?: string;
  guard?: string;
  print?: string;
}

const preload = ({ refuse, guard, print }: ChildFs): string => `
const fs = process.getBuiltinModule("node:fs");
const refuse = ${JSON.stringify(refuse ?? null)}, guard = ${JSON.stringify(guard ?? null)};
for (const name of ["readdirSync", "statSync", "lstatSync", "existsSync", "realpathSync", "readFileSync"]) {
  const real = fs[name];
  fs[name] = (p, ...rest) => {
    const at = String(p);
    if (guard !== null && (at.startsWith(guard + "/") || (name === "readdirSync" && at === guard))) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0);
    if (name === "readdirSync" && at === refuse) throw Object.assign(new Error("EACCES: permission denied, scandir '" + at + "'"), { code: "EACCES" });
    return real(p, ...rest);
  };
}
const print = ${JSON.stringify(print ?? null)};
if (print !== null) process.stdin.once("data", () => process.stdout.write(print + "\\n"));`;

export async function withChildFs<T>(how: ChildFs, fn: () => Promise<T>): Promise<T> {
  const was = process.env.NODE_OPTIONS;
  process.env.NODE_OPTIONS = `--import data:text/javascript,${encodeURIComponent(preload(how))}`;
  try {
    return await fn();
  } finally {
    if (was === undefined) delete process.env.NODE_OPTIONS;
    else process.env.NODE_OPTIONS = was;
  }
}
