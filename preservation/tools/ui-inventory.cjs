// Extract every authored JSX interaction and bridge call with the pinned TS parser.
const fs = require('node:fs');
const path = require('node:path');
const { createRequire } = require('node:module');
const root = process.argv[2];
const ts = createRequire(path.join(root, 'frontend/package.json'))('typescript');
const rows=[];
function walk(dir) { for(const e of fs.readdirSync(dir,{withFileTypes:true})) {
 const p=path.join(dir,e.name);
 if(e.isDirectory()) {walk(p);continue;}
 if(!/\.tsx?$/.test(e.name)||/\.test\./.test(e.name))continue;
 const body=fs.readFileSync(p,'utf8'), file=ts.createSourceFile(p,body,ts.ScriptTarget.Latest,true,e.name.endsWith('tsx')?ts.ScriptKind.TSX:ts.ScriptKind.TS);
 function add(kind,name,n) {const pos=file.getLineAndCharacterOfPosition(n.getStart(file));rows.push({path:path.relative(root,p),line:pos.line+1,kind,name,expression:n.getText(file)});}
 function visit(n){
  if(ts.isJsxAttribute(n)&&/^on[A-Z]/.test(n.name.getText(file))) add('ui-action',n.name.getText(file),n);
  if(ts.isCallExpression(n)&&/\b(?:localStorage|sessionStorage|history)\./.test(n.expression.getText(file))) add('browser-state',n.expression.getText(file),n);
  if(ts.isImportDeclaration(n)&&/wailsjs/.test(n.moduleSpecifier.getText(file)))add('bridge-import',n.moduleSpecifier.getText(file),n);
  ts.forEachChild(n,visit);
 }
 visit(file);
}}
walk(path.join(root,'frontend/src'));
process.stdout.write(JSON.stringify(rows,null,2)+'\n');
