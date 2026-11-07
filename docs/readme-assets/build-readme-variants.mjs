// README.md is the Graphite edition; keep the Moss edition text identical.
import fs from 'node:fs';import path from 'node:path';
const root=path.resolve(path.dirname(new URL(import.meta.url).pathname),'../..');
const main=fs.readFileSync(path.join(root,'README.md'),'utf8');
fs.writeFileSync(path.join(root,'README.moss.md'),main.replaceAll('/graphite/','/moss/').replaceAll('.graphite.png','.moss.png').replaceAll('setup-graphite.gif','setup-moss.gif'));
console.log('Wrote README.moss.md');
