// SPDX-License-Identifier: AGPL-3.0-only
// Written by frame-seal.mjs from apps/web/src/slate/pieces/frame-seal.ts; never edited by hand.
"use strict";
var wspSeal = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // apps/web/src/slate/pieces/frame-seal.ts
  var frame_seal_exports = {};
  __export(frame_seal_exports, {
    UNSEALABLE: () => UNSEALABLE,
    frameShell: () => frameShell,
    frameTheme: () => frameTheme,
    sealPage: () => sealPage,
    sealable: () => sealable
  });

  // packages/protocol/src/slate/frame-policy.ts
  var SLATE_FRAME_POLICY = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; form-action 'none'; base-uri 'none'; frame-src 'none'; worker-src 'none'; require-trusted-types-for 'script'; trusted-types default wsp-raw";

  // apps/web/src/slate/pieces/frame-seal.ts
  var NESTED = "iframe,frame,frameset,object,embed,portal,fencedframe";
  var TOKENS = ["background", "foreground", "card", "card-foreground", "muted", "muted-foreground", "accent", "accent-foreground", "primary", "primary-foreground", "border", "input", "ring", "success", "warning", "success-foreground", "warning-foreground", "error-foreground", "info-foreground", "slate-accent", "font-sans", "font-mono"];
  function frameTheme(root = document.documentElement) {
    const style = getComputedStyle(root);
    const vars = {};
    for (const name of TOKENS) {
      const value = style.getPropertyValue(`--${name}`).trim();
      if (value !== "") vars[name] = value;
    }
    return { scheme: style.colorScheme.includes("dark") ? "dark" : "light", vars };
  }
  var UNSEALABLE = "This window's web engine cannot draw a page sealed; Safari 26 or later on macOS 14 or later can.";
  var sealable = () => typeof globalThis.trustedTypes === "object";
  var GUARD = `(()=>{
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
  var attr = (s) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
  function sealPage(html, theme) {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const clean = (root) => {
      for (const n of root.querySelectorAll(`${NESTED},meta[http-equiv],base`)) n.remove();
      for (const t of root.querySelectorAll("template")) {
        t.removeAttribute("shadowrootmode");
        t.removeAttribute("shadowroot");
        clean(t.content);
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
  function frameShell(sealed, scheme) {
    return `<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; frame-src 'none'"><style>:root{color-scheme:${scheme}}html,body{margin:0;height:100%;overflow:hidden;background:transparent}iframe{display:block;border:0;width:100%;height:100%;background:transparent;color-scheme:${scheme}}</style><iframe sandbox="allow-scripts" title="page" srcdoc="${attr(sealed)}"></iframe><script>const f=document.querySelector("iframe");addEventListener("message",e=>{const d=e.data;if(e.source===f.contentWindow&&d&&d.wspFrame==="height")parent.postMessage({wspFrame:"height",height:Number(d.height)||0},"*")})<\/script>`;
  }
  return __toCommonJS(frame_seal_exports);
})();
