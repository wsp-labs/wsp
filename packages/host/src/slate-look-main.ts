// SPDX-License-Identifier: AGPL-3.0-only
// The app's own Electron, started by a host that runs inside it to draw one slate and quit: a hidden window on the
// host's page in its slate-look mode, a temporary profile so it shares nothing with the open app and takes no lock of
// it, and one line of JSON each way on stdin and stdout. The page asks for the slate and for each image and page it
// names, the host answers, and once the page says its height the window is captured and the PNG is the last line.
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

interface Electron {
  app: { setPath(name: string, path: string): void; whenReady(): Promise<void>; quit(): void; dock?: { hide(): void }; on(event: string, f: () => void): void };
  BrowserWindow: new (o: Record<string, unknown>) => { loadURL(url: string): Promise<void>; setContentSize(w: number, h: number): void; webContents: { capturePage(rect?: unknown, opts?: Record<string, unknown>): Promise<{ toPNG(): Buffer; getSize(): { width: number; height: number } }> }; destroy(): void };
  ipcMain: { handle(channel: string, f: (event: unknown, ...args: unknown[]) => unknown): void };
}

const PRELOAD = `const { contextBridge, ipcRenderer } = require("electron");
contextBridge.exposeInMainWorld("wspLook", { slate: () => ipcRenderer.invoke("look:slate"), read: (kind, src) => ipcRenderer.invoke("look:read", kind, src), drawn: height => ipcRenderer.invoke("look:drawn", height) });`;

/** The tallest picture a look takes; a slate past it is cut there. */
const MOST_HEIGHT = 4000;

/** Resolves once the line is handed to the pipe: a PNG is megabytes, and a quit before then cut it short. */
const say = (line: Record<string, unknown>): Promise<void> => new Promise(sent => void process.stdout.write(`${JSON.stringify(line)}\n`, () => sent()));

async function main(): Promise<void> {
  // A name held in a variable, so neither the bundler nor the type checker looks for a package this host does not carry.
  const electron = "electron";
  const { app, BrowserWindow, ipcMain } = (await import(electron)) as Electron;
  const profile = mkdtempSync(join(tmpdir(), "wsp-slate-look-"));
  app.setPath("userData", profile);
  app.dock?.hide();
  app.on("window-all-closed", () => {});
  const answers = new Map<number, (answer: unknown) => void>();
  let asked = 0;
  let given: { base: string; view: unknown; theme: string; width: number } | undefined;
  const first = new Promise<void>(ready => {
    createInterface({ input: process.stdin }).on("line", line => {
      const msg = JSON.parse(line) as { type: string; id?: number; answer?: unknown } & Record<string, unknown>;
      if (msg.type === "slate") {
        given = msg as unknown as NonNullable<typeof given>;
        ready();
      } else if (msg.type === "answer" && msg.id !== undefined) answers.get(msg.id)?.(msg.answer), answers.delete(msg.id);
    });
  });
  await Promise.all([app.whenReady(), first]);
  const look = given!;
  const preload = join(profile, "preload.cjs");
  writeFileSync(preload, PRELOAD);
  const win = new BrowserWindow({ show: false, width: look.width, height: 800, useContentSize: true, paintWhenInitiallyHidden: true, backgroundColor: "#00000000", webPreferences: { preload, sandbox: true, contextIsolation: true } });
  const done = new Promise<void>((finish, fail) => {
    ipcMain.handle("look:slate", () => ({ view: look.view, theme: look.theme, width: look.width }));
    ipcMain.handle("look:read", (_e, kind, src) => new Promise(answer => { const id = ++asked; answers.set(id, answer); void say({ type: "read", id, kind, src }); }));
    ipcMain.handle("look:drawn", async (_e, height) => {
      try {
        const h = Math.max(40, Math.min(MOST_HEIGHT, Number(height) || 0));
        win.setContentSize(look.width, h);
        await new Promise(r => setTimeout(r, 400));
        const shot = await win.webContents.capturePage(undefined, { stayHidden: true });
        const size = shot.getSize();
        await say({ type: "done", png: shot.toPNG().toString("base64"), width: size.width, height: size.height });
        finish();
      } catch (e) {
        fail(e);
      }
    });
  });
  await win.loadURL(`${look.base}/?slate-look=1`);
  await done;
  win.destroy();
  rmSync(profile, { recursive: true, force: true });
  app.quit();
}

main().catch(async e => {
  await say({ type: "failed", message: e instanceof Error ? e.message : String(e) });
  process.exit(1);
});
