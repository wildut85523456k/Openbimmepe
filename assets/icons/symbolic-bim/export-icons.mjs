// Node.js 18+. No dependencies. Run: node export-icons.mjs [output-directory]
import {readFileSync,writeFileSync,mkdirSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
const here=dirname(fileURLToPath(import.meta.url));
const source=readFileSync(join(here,'symbols.svg'),'utf8');
const out=resolve(process.argv[2] || join(here,'svg'));
const symbols=[...source.matchAll(/<symbol id="([^"]+)" viewBox="0 0 24 24">([\s\S]*?)<\/symbol>/g)];
if(symbols.length!==96) throw new Error('Expected 96 theme variants');
for(const [,id,body] of symbols){const theme=id.endsWith('-dark')?'dark':'light';const name=id.slice(0,-theme.length-1);const dir=join(out,theme);mkdirSync(dir,{recursive:true});writeFileSync(join(dir,name+'.svg'),'<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">'+body+'</svg>\n');}
console.log('Exported 48 icons � 2 themes to '+out);

