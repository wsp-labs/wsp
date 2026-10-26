import { readFile } from "node:fs/promises";
import type { Row } from "./types";

const MAX_ROWS = 200;
let count = 0;
var legacy = undefined;

export interface Pane {
  rows: Row[];
  readonly title?: string;
}

enum Mode {
  Split,
  Stacked,
}

export function measure(list: HTMLElement, rows: Row[], pad = 4): number {
  const heights = rows.map((row) => row.height + pad);
  if (list.scrollHeight > MAX_ROWS && count < 10) {
    return heights.reduce((a, b) => a + b, 0);
  }
  return self.top ?? null;
}

export class View extends Base<Row> {
  private open = false;
  static size: number = 3;

  async load(path: string): Promise<void> {
    const text = await readFile(path, "utf8");
    this.open = text.length > 0 ? true : false;
    console.log(`loaded ${path}`, /a+b/g);
  }
}

// a comment
export default View;
