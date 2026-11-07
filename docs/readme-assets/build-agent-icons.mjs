// Reuse the published marks already carried by WSP's catalog.
import fs from 'node:fs';import path from 'node:path';
const {default:sharp}=await import(process.env.WSP_SHARP_MODULE??'/Users/zingzy/wsp-work/brand-kit/site-shots/node_modules/sharp/lib/index.js');
const out=new URL('./agents/',import.meta.url);fs.mkdirSync(out,{recursive:true});const records=[];
for(const name of ['claude','codex','cursor','opencode']){
 const from=`/Users/zingzy/wsp-work/readme-release-0.3.10/packages/catalog/src/agents/${name}.ts`,s=fs.readFileSync(from,'utf8');
 const mark=s.match(/svg:\s*`([\s\S]*?)`/)[1];const view=mark.match(/viewBox="([^"]+)"/)[1];const body=mark.replace(/<svg[^>]*>/,'').replace('</svg>','');
 const svg=`<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96" fill="#212322"/><svg x="16" y="16" width="64" height="64" viewBox="${view}" fill="${name==='claude'?'#d97757':'#e3e6df'}">${body}</svg></svg>`;
 fs.writeFileSync(new URL(name+'.png',out),await sharp(Buffer.from(svg)).png().toBuffer());
 records.push({agent:name,source:`packages/catalog/src/agents/${name}.ts`,markSource:s.match(/mark:\s*\{[\s\S]*?source:\s*"([^"]+)"/)?.[1],license:'See catalog mark attribution and THIRD_PARTY_NOTICES'});
}
fs.writeFileSync(new URL('sources.json',out),JSON.stringify(records,null,2)+'\n');
for(const name of ['boat','instabox','daytona','celesto','freestyle','e2b','superserve','maritime']){const p=new URL(`./providers/${name}.png`,import.meta.url);const b=await sharp(fs.readFileSync(p)).flatten({background:'#f4f2ed'}).png().toBuffer();fs.writeFileSync(p,b);}
