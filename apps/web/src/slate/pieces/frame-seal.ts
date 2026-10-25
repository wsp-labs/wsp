// SPDX-License-Identifier: AGPL-3.0-only
// An agent's HTML page made into a document that reaches nothing. It is drawn two frames deep, both sandboxed to an
// opaque origin with scripts and nothing else: the outer one's policy refuses every frame load, which is the only rule
// that stops the page navigating itself to an address (measured in Chromium 151: a lone sandboxed frame can), and the
// inner one holds the page under SLATE_FRAME_POLICY, so it fetches, connects, posts and loads nothing. What a policy
// cannot refuse is closed by a script that runs before the page's own: WebRTC reached a STUN server through every
// policy, so its constructors go, and a nested page, the one road to a fresh realm that still has them, is cut at the
// string sinks (Trusted Types), at the DOM (an observer that drops any frame inserted) and at XSLT. The observer alone
// cannot see a shadow root the parser makes, so an engine without Trusted Types draws no page: the panel says why, and
// a page sealed there anyway stops before its first script. The same seal draws the GPUI app's frame in WebKit,
// bundled by apps/desktop-gpui/app/frame-seal.mjs.
import { SLATE_FRAME_POLICY } from "@wsp/protocol/slate";

/** What a page may not hold, cut from its own markup before it is drawn and from any HTML it writes later. */
const NESTED = "iframe,frame,frameset,object,embed,portal,fencedframe";

/** The theme's tokens a page is handed as its own :root variables, read off this window as they are now. */
const TOKENS = ["background", "foreground", "card", "card-foreground", "muted", "muted-foreground", "accent", "accent-foreground", "primary", "primary-foreground", "border", "input", "ring", "success", "warning", "success-foreground", "warning-foreground", "error-foreground", "info-foreground", "slate-accent", "font-sans", "font-mono"] as const;

export interface FrameTheme {
  scheme: "dark" | "light";
  vars: Record<string, string>;
}

export function frameTheme(root: HTMLElement = document.documentElement): FrameTheme {
  const style = getComputedStyle(root);
  const vars: Record<string, string> = {};
  for (const name of TOKENS) {
    const value = style.getPropertyValue(`--${name}`).trim();
    if (value !== "") vars[name] = value;
  }
  return { scheme: style.colorScheme.includes("dark") ? "dark" : "light", vars };
}

/** Said in a frame's place where this window's engine cannot seal a page. */
export const UNSEALABLE = "This window's web engine cannot draw a page sealed; Safari 26 or later on macOS 14 or later can.";

/** Whether this window's engine has what the seal stands on. */
export const sealable = (): boolean => typeof (globalThis as { trustedTypes?: unknown }).trustedTypes === "object";

/** The first script in the page: it runs before any of the page's own and holds what it captured in its closure. */
const GUARD = `(()=>{
for(const k of ["RTCPeerConnection","webkitRTCPeerConnection","RTCDataChannel","RTCSessionDescription","RTCIceCandidate","RTCRtpSender","RTCRtpReceiver","RTCRtpTransceiver","XSLTProcessor"])try{delete window[k]}catch(e){}
if(typeof self.trustedTypes!=="object"){stop();document.documentElement.remove();throw new Error("no Trusted Types")}
const NESTED=${JSON.stringify(NESTED)};
const qsa=Function.prototype.call.bind(DocumentFragment.prototype.querySelectorAll),dqsa=Function.prototype.call.bind(Document.prototype.querySelectorAll),rm=Function.prototype.call.bind(Element.prototype.remove),unattr=Function.prototype.call.bind(Element.prototype.removeAttribute),content=Object.getOwnPropertyDescriptor(HTMLTemplateElement.prototype,"content").get;
const clean=f=>{for(const n of qsa(f,NESTED))rm(n);for(const t of qsa(f,"template")){unattr(t,"shadowrootmode");unattr(t,"shadowroot");clean(content.call(t))}};
const T=document.createElement("template"),raw=trustedTypes.createPolicy("wsp-raw",{createHTML:s=>s});
trustedTypes.createPolicy("default",{createHTML:(s,_t,sink)=>{if(/srcdoc/i.test(String(sink)))return "";T.innerHTML=raw.createHTML(s);clean(content.call(T));return T.innerHTML},createScript:()=>null,createScriptURL:()=>null});
const drop=root=>{for(const n of (root===document?dqsa(document,NESTED):qsa(root,NESTED)))rm(n)};
const roots=[document],watch=new MutationObserver(()=>{for(const r of roots)drop(r)});
watch.observe(document,{childList:true,subtree:true});
const attach=Element.prototype.attachShadow;
Element.prototype.attachShadow=function(o){const r=attach.call(this,o);roots.push(r);watch.observe(r,{childList:true,subtree:true});return r};
addEventListener("click",e=>{const a=e.target instanceof Element?e.target.closest("a[href]"):null;if(!a)return;e.preventDefault();const h=a.getAttribute("href")||"";if(h.startsWith("#")&&h.length>1)document.getElementById(decodeURIComponent(h.slice(1)))?.scrollIntoView()},true);
const root=document.documentElement,say=()=>parent.postMessage({wspFrame:"height",height:Math.max(Math.ceil(root.getBoundingClientRect().height),root.scrollHeight>root.clientHeight?root.scrollHeight:0)},"*");
new ResizeObserver(say).observe(root);addEventListener("load",say);
})();`;

const attr = (s: string): string => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");

/** The page as the inner frame draws it: its own frames and declarative shadow roots cut, its own event handlers on html and
 * head dropped (they are parsed before the guard runs), the policy, the guard and the theme first in its head,
 * and no ground of its own unless it paints one. */
export function sealPage(html: string, theme: FrameTheme): string {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const clean = (root: ParentNode): void => {
    for (const n of root.querySelectorAll(`${NESTED},meta[http-equiv],base`)) n.remove();
    for (const t of root.querySelectorAll("template")) {
      t.removeAttribute("shadowrootmode");
      t.removeAttribute("shadowroot");
      clean((t as HTMLTemplateElement).content);
    }
  };
  clean(doc);
  for (const el of [doc.documentElement, doc.head])
    for (const a of [...el.attributes]) if (/^on/i.test(a.name)) el.removeAttribute(a.name);
  const policy = doc.createElement("meta");
  policy.httpEquiv = "Content-Security-Policy";
  policy.content = SLATE_FRAME_POLICY;
  const guard = doc.createElement("script");
  guard.textContent = GUARD;
  const look = doc.createElement("style");
  const vars = Object.entries(theme.vars).map(([k, v]) => `--${k}:${v};`).join("");
  look.textContent = `:root{color-scheme:${theme.scheme};${vars}}:where(html,body){background:transparent;margin:0;color:var(--foreground);font-family:var(--font-sans)}`;
  doc.head.prepend(policy, guard, look);
  doc.documentElement.dataset["scheme"] = theme.scheme;
  return `<!doctype html>${doc.documentElement.outerHTML}`;
}

/** The outer frame around a sealed page: it loads no frame but the page, and passes on the page's height alone. */
export function frameShell(sealed: string, scheme: FrameTheme["scheme"]): string {
  return `<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; frame-src 'none'"><style>:root{color-scheme:${scheme}}html,body{margin:0;height:100%;overflow:hidden;background:transparent}iframe{display:block;border:0;width:100%;height:100%;background:transparent;color-scheme:${scheme}}</style><iframe sandbox="allow-scripts" title="page" srcdoc="${attr(sealed)}"></iframe><script>const f=document.querySelector("iframe");addEventListener("message",e=>{const d=e.data;if(e.source===f.contentWindow&&d&&d.wspFrame==="height")parent.postMessage({wspFrame:"height",height:Number(d.height)||0},"*")})</script>`;
}
