// Remove the desktop pixels baked outside the captured native window outline.
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';
const {default:sharp}=await import(process.env.WSP_SHARP_MODULE??'/Users/zingzy/wsp-work/brand-kit/site-shots/node_modules/sharp/lib/index.js');
export function windowMask(width,height){
 const file=new URL(`./window-masks/native-${width}x${height}.png`,import.meta.url);
 return fs.readFileSync(file);
}
export async function maskWindow(input){
 const {width,height}=await sharp(input).metadata();
 return sharp(input).ensureAlpha().composite([{input:windowMask(width,height),blend:'dest-in'}]).png().toBuffer();
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const [width,height,out]=process.argv.slice(2);
 if(!Number.isInteger(+width)||!Number.isInteger(+height)||+width<100||+height<100||!out)throw Error('usage: window-mask.mjs <width> <height> <output.png>');
 await sharp(windowMask(+width,+height)).png().toFile(out);
}
