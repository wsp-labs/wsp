// SPDX-License-Identifier: AGPL-3.0-only
// The frame piece over a thread folder of HTML pages: the anatomy dial indicator as its repo commits it and as an
// agent hands it to a slate, a hand-written explainer, and a page that tries every forbidden thing.
import type { SlateJson } from "@wsp/protocol/slate";

export const FRAME_FILES = {
  anatomy: "figures/dial-indicator.html",
  anatomyAsCommitted: "figures/dial-indicator-committed.html",
  explainer: "pages/explainer.html",
  hostile: "pages/hostile.html",
} as const;

export const FRAME_CASES: Readonly<Record<string, { text: string; values?: Record<string, SlateJson> }>> = {
  "frame-anatomy": {
    text: `<slate><column><section title="Dial indicator" note="drawn with the anatomy skill"><frame id="dial" src="${FRAME_FILES.anatomy}" caption="Hover to slide the carriage; the needle reads each block's height" /></section><section title="The blocks"><facts><fact label="Heights" value="18, 14 and 10" mono /><fact label="One turn" value="20 px" mono /></facts></section></column></slate>`,
  },
  "frame-anatomy-committed": {
    text: `<slate><column><section title="Dial indicator" note="the page as its repo commits it"><frame id="dial" src="${FRAME_FILES.anatomyAsCommitted}" caption="Its own ground and palette, since the page paints them" /></section></column></slate>`,
  },
  "frame-explainer": {
    text: `<slate><run name="bench" cmd="echo" /><column><section title="What happens on a click"><frame id="explain" src="${FRAME_FILES.explainer}" /></section><section title="Last hour"><row><number label="Redirects" value={18240} format="integer" note="per hour" /><number label="p95" value={22} unit="ms" note="last hour" /></row><button label="Run the bench" onPress={start($bench)} /></section></column></slate>`,
  },
  "frame-hostile": {
    text: `<slate><column><section title="A page that tries everything"><frame id="hostile" src="${FRAME_FILES.hostile}" /></section></column></slate>`,
  },
  "frame-states": {
    text: `<slate><column>
  <section title="Loading"><frame id="loading" src="loading/explainer.html" /></section>
  <section title="Missing file"><frame id="missing" src="pages/gone.html" /></section>
  <section title="Too big to show"><frame id="big" src="too-big/trace.html" /></section>
</column></slate>`,
  },
};
