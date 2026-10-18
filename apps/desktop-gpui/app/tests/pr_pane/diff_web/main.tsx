// SPDX-License-Identifier: AGPL-3.0-only
// Today's code view over tests/pr_pane/diff_sample.patch, as the Changes pane draws it: diff_web.sh builds and shoots it.
import "./app.css";
import "@web/themes/index.js";
import { createRoot } from "react-dom/client";
import { StyledDiffCodeView } from "@web/components/diffs/StyledDiffCodeView";
import { toDiffModel } from "@web/diffs/model";
import { diffPanelOptions } from "@web/diffs/DiffSurface";
import { getDiffCollapseIconClassName } from "@web/lib/diffRendering";
import { ChevronDownIcon } from "lucide-react";
import patch from "@sample?raw";

const params = new URLSearchParams(location.search);
const theme = params.get("theme") ?? "paper";
const side = theme === "paper" || theme === "linen" || theme === "frost" ? "light" : "dark";
const mode = params.get("mode") === "split" ? "split" : "stacked";
document.documentElement.dataset.theme = theme;
if (side === "dark") document.documentElement.classList.add("dark");

const files = patch.split(/(?=^diff --git )/m).filter(Boolean).map(text => {
  const path = /^diff --git a\/(\S+) b\/(\S+)/.exec(text)![2];
  return { path, kind: "modified", additions: 0, deletions: 0, patch: text };
});
const model = toDiffModel({ base: null, files, truncated: false, moved: [] } as never, "shot");
const items = model.files.map(f => ({ id: f.fileKey, type: "diff" as const, fileDiff: f.fileDiff, collapsed: false, version: f.fileVersion }));

function App() {
  return (
    <div style={{ position: "fixed", inset: 0, display: "flex", flexDirection: "column" }} className="bg-background text-foreground">
      <div className="min-h-0 flex-1">
        <StyledDiffCodeView
          className="h-full min-h-0 overflow-auto"
          items={items as never}
          renderHeaderPrefix={(item: any) => (
            <span className="flex items-center gap-1">
              <span className={`-ms-0.5 inline-flex size-5 items-center justify-center ${getDiffCollapseIconClassName(item.fileDiff)}`}><ChevronDownIcon className="size-4" /></span>
            </span>
          )}
          options={diffPanelOptions(side, mode) as never}
        />
      </div>
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
(window as any).__ready = true;
