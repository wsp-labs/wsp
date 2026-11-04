// SPDX-License-Identifier: AGPL-3.0-only
// A checkout travels through its own computer, including its index; a worktree's .git pointer never travels.
import { SEED_ROWS } from "@wsp/catalog";
import { landBytes, type Machine } from "@wsp/engine";
import { shellQuote as q, usageRefusal } from "@wsp/protocol";

const DEADLINE = 180_000;
const CHUNK = 256 * 1024;
const excludes = (kinds: readonly string[]): string => SEED_ROWS.flatMap(row => Object.entries(row.patterns).filter(([kind]) => kinds.includes(kind)).flatMap(([, patterns]) => patterns)).map(pattern => `--exclude=${q(pattern.replace(/\/$/, ""))}`).join(" ");
const NEVER = excludes(["never"]);
const MAX_BYTES = 1024 * 1024 * 1024;
type Options = { sourceFolder: string; destinationFolder: string; branch: string; picked?: readonly string[]; sourceScratch?: string; destinationScratch?: string; onProgress?: (bytes: number, total: number) => void };
export type TransferredCheckout = { folder: string; root: string; head: string; sourceBranch: string; branch: string; git: boolean; bytes: number; sent: string[] };

async function checked(machine: Machine, command: string, long = false): Promise<string> {
  const read = long ? await machine.run(command, { deadlineMs: DEADLINE }) : await machine.exec(command, { timeoutMs: 20_000 });
  if (read.exitCode !== 0) throw new Error(`Project transfer failed: ${(read.stderr || read.stdout).trim().slice(-1000) || `exit ${read.exitCode}`}. Check the computer and retry.`);
  return read.stdout;
}
const remove = async (machine: Machine, path: string) => { await machine.exec(`rm -rf -- ${q(path)}`).catch(() => {}); };
async function temporary(machine: Machine, parent = "/tmp", prefix = "wsp-transfer."): Promise<string> {
  return (await checked(machine, `umask 077\nmkdir -p -- ${q(parent)}\nmktemp -d ${q(`${parent}/${prefix}XXXXXX`)}`)).trim();
}
const hashCommand = (file: string) => `if command -v sha256sum >/dev/null 2>&1; then sha256sum ${q(file)}; else shasum -a 256 ${q(file)}; fi`;

/** Bounded frames rather than one base64 stdout: a project's history can exceed the daemon's single-answer cap. */
async function transferFile(source: Machine, destination: Machine, from: string, to: string, progress?: (bytes: number, total: number) => void): Promise<number> {
  const size = Number((await checked(source, `wc -c < ${q(from)}`)).trim());
  if (!Number.isSafeInteger(size) || size < 0 || size > MAX_BYTES) throw usageRefusal("The project transfer is larger than 1 GB, or its size could not be read.", "Exclude generated files from the project and try again.");
  const hash = (await checked(source, hashCommand(from))).trim().split(/\s+/)[0];
  await checked(destination, `umask 077\n: > ${q(to)}`);
  const piece = `${to}.part`;
  progress?.(0, size);
  try {
    for (let offset = 0; offset < size; offset += CHUNK) {
      const encoded = await checked(source, `set -o pipefail\ndd if=${q(from)} bs=${CHUNK} skip=${offset / CHUNK} count=1 2>/dev/null | base64`);
      const bytes = Buffer.from(encoded.replace(/\s/g, ""), "base64");
      if (bytes.length !== Math.min(CHUNK, size - offset)) throw new Error("Project transfer returned fewer bytes than the source held. Retry the transfer.");
      await landBytes(destination, piece, bytes, { timeoutMs: DEADLINE });
      await checked(destination, `cat ${q(piece)} >> ${q(to)}\nrm -f -- ${q(piece)}`);
      progress?.(Math.min(offset + CHUNK, size), size);
    }
    const got = (await checked(destination, hashCommand(to))).trim().split(/\s+/)[0];
    if (!hash || hash !== got) throw new Error("Project transfer hash did not match. Retry the transfer.");
    return size;
  } finally { await remove(destination, piece); }
}

export async function transferCheckout(source: Machine, destination: Machine, o: Options): Promise<TransferredCheckout> {
  for (const path of o.picked ?? []) if (path === "" || path.startsWith("/") || path.split("/").some(p => p === ".." || p === ".git" || p === "")) throw usageRefusal("A selected path is outside the project folder.", "Select files inside the project folder and retry.");
  const meta = (await checked(source, [
    `cd -P ${q(o.sourceFolder)} || exit 1`,
    `root=$(git rev-parse --show-toplevel 2>/dev/null) || root="$PWD"`,
    `head=$(git rev-parse --verify HEAD 2>/dev/null) || head=`,
    `branch=$(git symbolic-ref --quiet --short HEAD 2>/dev/null) || branch=`,
    `printf '%s\\0' "$root" "$head" "$branch" "$PWD"`,
  ].join("\n"))).split("\0");
  const [root, head, sourceBranch, folder] = meta as [string, string, string, string];
  if (meta.length !== 5 || !root.startsWith("/") || !(folder === root || folder.startsWith(`${root}/`))) throw new Error("The source computer returned invalid project metadata. Retry the transfer.");
  const prefix = folder.slice(root.length).replace(/^\//, "");
  const git = head !== "";
  const sent = [...(o.picked ?? [])];
  const left = await temporary(source, o.sourceScratch);
  let right: string | undefined;
  let reserved = false;
  try {
    right = await temporary(destination, o.destinationScratch);
    const paths = `${left}/files.list`;
    const lines = [`set -e -o pipefail`, `umask 077`];
    if (git) lines.push(
      `git -C ${q(root)} rev-list --objects --no-walk ${q(head)} | git -C ${q(root)} pack-objects --stdout > ${q(`${left}/checkout.pack`)}`,
      `git -C ${q(root)} diff --cached --binary --full-index > ${q(`${left}/index.patch`)}`,
      `git -C ${q(root)} diff --binary --full-index > ${q(`${left}/work.patch`)}`,
      `git -C ${q(root)} ls-files --others --exclude-standard -z > ${q(paths)}`,
    );
    else lines.push(`cd ${q(root)}`, `find . -type d \\( -name .git -o -name node_modules -o -name .venv -o -name target -o -name .ssh -o -name .codex -o -name .claude \\) -prune -o ! -type d -print0 > ${q(paths)}`);
    for (const path of sent) {
      const relative = prefix === "" ? path : `${prefix}/${path}`;
      lines.push(`if [ -e ${q(`${root}/${relative}`)} ] || [ -L ${q(`${root}/${relative}`)} ]; then printf '%s\\0' ${q(relative)} >> ${q(paths)}; fi`);
    }
    lines.push(`tar ${NEVER} --exclude=.git --exclude='*/.git' -C ${q(root)} --null -T ${q(paths)} -czf ${q(`${left}/files.tgz`)}`, `rm -f -- ${q(paths)}`);
    if (git) lines.push(
      `git -C ${q(root)} diff --cached --binary --full-index > ${q(`${left}/check.index`)}`,
      `git -C ${q(root)} diff --binary --full-index > ${q(`${left}/check.work`)}`,
      `cmp ${q(`${left}/index.patch`)} ${q(`${left}/check.index`)} && cmp ${q(`${left}/work.patch`)} ${q(`${left}/check.work`)}`,
      `test "$(git -C ${q(root)} rev-parse HEAD)" = ${q(head)}`,
      `rm -f -- ${q(`${left}/check.index`)} ${q(`${left}/check.work`)}`,
    );
    lines.push(`tar -C ${q(left)} -czf ${q(`${left}/payload.tgz`)} ${git ? "checkout.pack index.patch work.patch " : ""}files.tgz`);
    await checked(source, lines.join("\n"), true);
    const bytes = await transferFile(source, destination, `${left}/payload.tgz`, `${right}/payload.tgz`, o.onProgress);
    await checked(destination, `set -e\numask 077\nmkdir -p -- ${q(o.destinationFolder.slice(0, o.destinationFolder.lastIndexOf("/")) || "/")}\nmkdir -- ${q(o.destinationFolder)}`);
    reserved = true;
    const land = [`set -e`, `tar -C ${q(right)} -xzf ${q(`${right}/payload.tgz`)}`];
    if (git) land.push(
      `git init -q ${head.length === 64 ? "--object-format=sha256 " : ""}-- ${q(o.destinationFolder)}`,
      `git -C ${q(o.destinationFolder)} index-pack --stdin < ${q(`${right}/checkout.pack`)}`,
      `printf '%s\\n' ${q(head)} > ${q(`${o.destinationFolder}/.git/shallow`)}`,
      `git -C ${q(o.destinationFolder)} -c core.hooksPath=/dev/null checkout -qb ${q(o.branch)} ${q(head)}`,
      `[ ! -s ${q(`${right}/index.patch`)} ] || git -C ${q(o.destinationFolder)} apply --index ${q(`${right}/index.patch`)}`,
      `[ ! -s ${q(`${right}/work.patch`)} ] || git -C ${q(o.destinationFolder)} apply ${q(`${right}/work.patch`)}`,
    );
    land.push(`tar -C ${q(o.destinationFolder)} -xzf ${q(`${right}/files.tgz`)}`);
    // Empty project subfolders are not in Git or the untracked list, but still remain the thread's cwd.
    land.push(`mkdir -p -- ${q(prefix === "" ? o.destinationFolder : `${o.destinationFolder}/${prefix}`)}`);
    await checked(destination, land.join("\n"), true);
    return { folder: prefix === "" ? o.destinationFolder : `${o.destinationFolder}/${prefix}`, root: o.destinationFolder, head, sourceBranch, branch: o.branch, git, bytes, sent };
  } catch (e) {
    if (reserved) await remove(destination, o.destinationFolder);
    throw e;
  } finally {
    await remove(source, left);
    if (right !== undefined) await remove(destination, right);
  }
}

export async function transferReturnedBranch(source: Machine, destination: Machine, o: Omit<Options, "picked"> & { base: string }): Promise<{ branch: string; head: string; from: string; cleanup: () => Promise<void> }> {
  const left = await temporary(source, o.sourceScratch);
  let right: string | undefined;
  try {
    // The complete branch also handles a child with no new commits and a lead whose branch moved meanwhile.
    await checked(source, `set -e\nhead=$(git -C ${q(o.sourceFolder)} rev-parse --verify ${q(`refs/heads/${o.branch}`)})\nif [ "$head" = ${q(o.base)} ]; then git -C ${q(o.sourceFolder)} bundle create ${q(`${left}/result.bundle`)} ${q(o.branch)}; else git -C ${q(o.sourceFolder)} bundle create ${q(`${left}/result.bundle`)} ${q(o.branch)} ${q(`^${o.base}`)}; fi`, true);
    const head = (await checked(source, `git bundle list-heads ${q(`${left}/result.bundle`)}`)).trim().split(/\s+/)[0]!;
    if (!/^[0-9a-f]{40,64}$/.test(head)) throw new Error("The result bundle returned no commit. Check the child's branch and retry.");
    right = await temporary(destination, o.destinationFolder, ".wsp-return-");
    const from = `${right}/result.bundle`;
    await transferFile(source, destination, `${left}/result.bundle`, from);
    const kept = right;
    return { branch: o.branch, head, from: `./${kept.slice(kept.lastIndexOf("/") + 1)}/result.bundle`, cleanup: () => remove(destination, kept) };
  } catch (e) {
    if (right !== undefined) await remove(destination, right);
    throw e;
  } finally { await remove(source, left); }
}

/** Ignore generated output, but retain changed ignored project data before removing shared compute. */
export async function checkoutIgnoredHash(machine: Machine, folder: string): Promise<string> {
  const output = await checked(machine, `set -e -o pipefail
cd ${q(folder)}
git ls-files --others --ignored --exclude-standard --directory -z | tar ${excludes(["never", "junk", "rebuilt"])} --exclude=.git --null -T - -cf - | if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi`);
  const hash = output.trim().split(/\s+/)[0]!;
  if (!/^[0-9a-f]{64}$/.test(hash)) throw new Error("The child's ignored files could not be checked. Keep the checkout and retry cleanup.");
  return hash;
}
