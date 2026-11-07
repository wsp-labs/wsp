// Reframe original classic Electron captures; never reconstruct product UI.
import fs from 'node:fs';import path from 'node:path';import {createHash} from 'node:crypto';
const {default:sharp}=await import(process.env.WSP_SHARP_MODULE??'/Users/zingzy/wsp-work/brand-kit/site-shots/node_modules/sharp/lib/index.js');
const root=path.dirname(new URL(import.meta.url).pathname),lab=process.env.WSP_LAB_SHOTS??'/Users/zingzy/wsp-work/brand-kit/site-shots';
const wall='/Users/zingzy/Library/Application Support/so.cap.desktop/bg-1791190531541-wallhaven-zpoy6g.png';
const W=1600,H=1104;
const backgroundFor=async height=>{
 const bw=Math.round(W*1.6),bh=Math.round(height*1.6);
 return sharp(wall).resize(bw,bh,{fit:'cover'}).extract({left:Math.round((bw-W)/2),top:0,width:W,height}).png().toBuffer();
};
const shots={
 hero:{contain:true,pad:64,height:1048},
 orchestration:{scene:'sidebar',region:true,rect:[24,930,510,780]},
 computers:{scale:1.65,focus:[1400,650],target:[800,550]},
 tools:{scene:'agents',scale:2.1,at:'tr'},
 activity:{contain:true,pad:64},usage:{contain:true,pad:64},
 'resume-reset':{region:true,useFocus:false,rect:[900,1480,1600,300]},
 terminal:{scale:2.1,at:'tr'},browser:{scale:2.1,at:'tr'},
 changes:{scale:2.1,at:'tr'},pr:{scale:2.1,at:'tr'},
 editor:{region:true,rect:[1460,0,360,400]},btw:{scale:2.1,at:'tr'},
};
const themes=process.env.WSP_EXPORT_THEME?.split(',')??['graphite','moss'];const only=process.env.WSP_EXPORT_SHOTS?.split(',');const records=[];
for(const theme of themes){
 const folder=path.join(lab,'out',`try-${theme}-own-glass`),out=path.join(root,'features',theme);fs.mkdirSync(out,{recursive:true});
 for(const [name,shot]of Object.entries(shots)){
  if(only&&!only.includes(name))continue;
  const raw=path.join(folder,(shot.scene??name)+'.png'),meta=await sharp(raw).metadata(),height=shot.height??H;
  const background=shot.height?await sharp(wall).resize(W,height,{fit:'cover'}).png().toBuffer():await backgroundFor(height);
  let input=await sharp(raw).png().toBuffer(),left,top,width,sh,rect;
  if(shot.region){
   const focusPath=raw.replace('.png','.focus.json');rect=shot.useFocus!==false&&fs.existsSync(focusPath)?JSON.parse(fs.readFileSync(focusPath)):shot.rect;
   if(rect.pixelRect)rect=rect.pixelRect;
   if(!Array.isArray(rect))rect=[rect.left??rect.x,rect.top??rect.y,rect.width,rect.height];
   const [x,y,w,h]=rect;input=await sharp(input).extract({left:Math.round(x),top:Math.round(y),width:Math.round(w),height:Math.round(h)}).png().toBuffer();
   const m=await sharp(input).metadata(),factor=Math.min((W-144)/m.width,(height-112)/m.height);width=Math.round(m.width*factor);sh=Math.round(m.height*factor);left=Math.round((W-width)/2);top=Math.round((height-sh)/2);
  }else if(shot.contain){
   const factor=Math.min((W-2*shot.pad)/meta.width,(height-2*shot.pad)/meta.height);width=Math.round(meta.width*factor);sh=Math.round(meta.height*factor);left=Math.round((W-width)/2);top=Math.round((height-sh)/2);
  }else{
   width=Math.round(W*shot.scale);sh=Math.round(width*meta.height/meta.width);const factor=width/meta.width;
   left=shot.focus?Math.round(shot.target[0]-shot.focus[0]*factor):shot.at==='tr'?Math.round(W*.94-width):Math.round((W-width)/2);
   top=shot.focus?Math.round(shot.target[1]-shot.focus[1]*factor):Math.round(height*.08);
  }
  const resized=await sharp(input).resize(width,sh).png().toBuffer(),x=Math.max(0,left),y=Math.max(0,top),clipLeft=Math.max(0,-left),clipTop=Math.max(0,-top),cw=Math.min(W-x,width-clipLeft),ch=Math.min(height-y,sh-clipTop);
  const visible=await sharp(resized).extract({left:clipLeft,top:clipTop,width:cw,height:ch}).png().toBuffer();
  await sharp(background).composite([{input:visible,left:x,top:y}]).webp({quality:94}).toFile(path.join(out,name+'.webp'));
  records.push({theme,name,source:path.relative(path.join(lab,'out'),raw),sourceSha256:createHash('sha256').update(fs.readFileSync(raw)).digest('hex'),frame:{width:W,height},region:rect,position:{left,top,width,height:sh},roundedCorners:false,shadow:false});
 }
 if(!only){
  const layers=[],panels=['slate-host','slate-release','slate-activity'];
  for(const [i,scene]of panels.entries()){
   const raw=path.join(folder,scene+'.png'),meta=await sharp(raw).metadata();
   const panel=await sharp(raw).extract({left:2044,top:0,width:meta.width-2044,height:meta.height}).resize({height:960}).png().toBuffer();
   const pm=await sharp(panel).metadata();layers.push({input:panel,left:80+i*497,top:24+i*40});
   records.push({theme,name:'slate-stack',panel:scene,source:path.relative(path.join(lab,'out'),raw),sourceSha256:createHash('sha256').update(fs.readFileSync(raw)).digest('hex'),region:{left:2044,top:0,width:meta.width-2044,height:meta.height},position:{left:80+i*497,top:24+i*40,width:pm.width,height:960},roundedCorners:false,shadow:false});
  }
  await sharp(await backgroundFor(H)).composite(layers).webp({quality:94}).toFile(path.join(out,'slate-stack.webp'));
 }
}
if(!only)fs.writeFileSync(path.join(root,'features/crops.json'),JSON.stringify(records,null,2)+'\n');
console.log(`Exported ${records.length} source views in ${themes.join(', ')}.`);
