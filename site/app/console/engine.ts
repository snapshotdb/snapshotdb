// @ts-nocheck
/* eslint-disable */
// Console engine — ported verbatim from the standalone console (imperative DOM).
// Wrapped so React can initialize it on mount and clear its polling interval on unmount.
export function initConsole() {
  const __timers = [];
  const setInterval = (...a) => { const id = globalThis.setInterval(...a); __timers.push(id); return id; };
const $=(s,r=document)=>r.querySelector(s); const $$=(s,r=document)=>[...r.querySelectorAll(s)];
const el=(t,a={},...k)=>{const n=document.createElement(t);for(const[x,v]of Object.entries(a)){
  if(x==='class')n.className=v;else if(x==='html')n.innerHTML=v;else if(x.startsWith('on'))n.addEventListener(x.slice(2),v);else n.setAttribute(x,v);}
  for(const c of k.flat())if(c!=null)n.append(c.nodeType?c:document.createTextNode(c));return n;};
const esc=s=>String(s??'').replace(/[&<>]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));

/* ---------- config ---------- */
const cfg={
  get base(){return localStorage.getItem('ab_base')||''},   // '' = same origin
  set base(v){localStorage.setItem('ab_base',v)},
  get token(){return localStorage.getItem('ab_token')||''},
  set token(v){localStorage.setItem('ab_token',v)},
};
const apiURL=p=>(cfg.base.replace(/\/$/,''))+p;

/* ---------- API: POST a command, poll the job ---------- */
async function raw(path,opts={}){
  const r=await fetch(apiURL(path),{...opts,headers:{'Authorization':'Bearer '+cfg.token,...(opts.headers||{})}});
  return r;
}
async function health(){
  const r=await raw('/v1/health'); if(!r.ok)throw new Error('HTTP '+r.status);
  return r.json();
}
async function cmd(args){
  const r=await raw('/v1/commands',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(args)});
  if(r.status===401)throw new Error('unauthorized — check the access token');
  if(!r.ok){let m='HTTP '+r.status;try{m=(await r.json()).error||m}catch{}throw new Error(m);}
  const {id}=await r.json();
  for(let i=0;i<600;i++){
    await new Promise(z=>setTimeout(z,i<10?150:400));
    const jr=await raw('/v1/jobs/'+id); if(!jr.ok)continue;
    const j=await jr.json();
    if(j.state==='done'){
      if(j.exit_code!==0)throw new Error((j.stderr||j.stdout||'command failed').trim());
      return (j.stdout||'').trim();
    }
  }
  throw new Error('timed out waiting for the server job');
}
const cmdJSON=async a=>{const s=await cmd([...a,'--format','json']);return JSON.parse(s||'null');};

/* ---------- toast ---------- */
function toast(kind,title,msg){
  const t=el('div',{class:'toast '+kind},el('div',{class:'t'},title),msg?el('div',{class:'m'},msg):null);
  $('#toasts').append(t); setTimeout(()=>{t.style.transition='.3s';t.style.opacity='0';setTimeout(()=>t.remove(),300)},kind==='err'?7000:3800);
}

/* ---------- state ---------- */
let branches=[]; let statusCache={}; let pollTimer=null;

async function load(){
  try{
    const h=await health();
    $('#ver').textContent='v'+(h.version||'?');
    $('#healthDot').className='dot ok'; $('#healthText').textContent=(cfg.base||'same origin')+' · online';
  }catch(e){
    $('#healthDot').className='dot bad'; $('#healthText').textContent='offline';
    return renderDisconnected(e.message);
  }
  try{
    branches=await cmdJSON(['list'])||[];
  }catch(e){ return renderDisconnected(e.message); }
  render();
  // fetch replication status for synced sources
  for(const b of branches.filter(x=>x.synced)){
    cmdJSON(['status',b.name]).then(s=>{statusCache[b.name]=s;patchSource(b.name);}).catch(()=>{});
  }
}

function renderDisconnected(msg){
  $('#stage').replaceChildren(el('div',{class:'empty'},
    el('h3',{},'Not connected'),
    el('p',{},msg||'Set the server address and access token.'),
    el('div',{style:'margin-top:14px'},el('button',{class:'btn primary',onclick:openConn},'Open connection settings'))));
}

function groupBranches(){
  const roots=branches.filter(b=>!b.parent||b.parent==='-');
  const kids=p=>branches.filter(b=>b.parent===p);
  return {roots,kids};
}

function render(){
  const {roots,kids}=groupBranches();
  const stage=el('div');
  // Sources
  const srcSec=el('section',{class:'sec'});
  srcSec.append(sectionHead('Sources',roots.length,'synced replicas & local roots'));
  if(!roots.length)srcSec.append(el('div',{class:'empty'},el('h3',{},'No sources yet'),
    el('p',{},'Connect a production database to keep a live replica you can branch from.'),
    el('div',{style:'margin-top:14px'},el('button',{class:'btn primary',onclick:openNewSource},'+ New source'))));
  for(const r of roots)srcSec.append(sourceCard(r,kids(r.name)));
  stage.append(srcSec);
  $('#stage').replaceChildren(stage);
}

function sectionHead(title,count,note){
  return el('div',{class:'sec-h'},el('h2',{},title),el('span',{class:'count'},count+' total'),
    el('span',{class:'spacer'}),el('span',{class:'count'},note));
}

function stateClass(b){
  if(b.engine==='sqlite')return 'file';
  return b.status;
}

function sourceCard(r,kids){
  const st=r.status;
  const card=el('div',{class:'card '+st,id:'src-'+cssid(r.name)});
  const badges=[el('span',{class:'badge engine'},r.engine),
    el('span',{class:'badge state '+st},st)];
  if(r.locked)badges.push(el('span',{class:'badge lock'},'🔒 locked'));
  const actions=el('div',{class:'cactions'});
  if(r.synced){
    actions.append(btn('sm','repair',()=>run(['repair',r.name],'Repairing '+r.name)));
    if(r.engine==='postgres')actions.append(btn('sm','reconcile',()=>run(['reconcile',r.name],'Reconciling '+r.name)));
  }
  actions.append(btn('sm','settings',()=>openSettings(r.name)));
  actions.append(btn('sm','+ branch',()=>openNewBranch(r.name),'primary'));
  actions.append(btn('sm',r.locked?'unlock':'lock',()=>run([r.locked?'unlock':'lock',r.name],(r.locked?'Unlocking ':'Locking ')+r.name)));
  actions.append(btn('sm danger','delete',()=>confirmDelete(r.name,true)));
  card.append(el('div',{class:'crow'},el('span',{class:'cname'},r.name),...badges,actions));
  // meta (status) placeholder, filled by patchSource
  card.append(el('dl',{class:'meta',id:'meta-'+cssid(r.name)},...sourceMeta(r)));
  // branches rail
  if(kids.length){
    const rail=el('div',{class:'branches'});
    for(const k of kids)rail.append(branchRow(k));
    card.append(rail);
  }
  return card;
}

function sourceMeta(r){
  const s=statusCache[r.name];
  if(r.synced && !s)return [metaItem('replication',el('span',{},el('span',{class:'spin'}),' reading…'))];
  if(!r.synced)return [metaItem('kind','local root (import)'),metaItem('branch from','create --from '+r.name)];
  const out=[];
  if(s.initial_copy)out.push(metaItem('initial copy',progress(s.initial_copy)));
  out.push(metaItem('stream',streamCell(s.stream)));
  if(s.tables)out.push(metaItem('tables',s.tables));
  if(s.slot)out.push(metaItem('wal slot',s.slot,/retain/i.test(s.slot)?'warn':''));
  if(s.schema_changes)out.push(metaItem('schema changes',s.schema_changes,/fail|not tracked/i.test(s.schema_changes)?'warn':''));
  return out.length?out:[metaItem('state',s.state||r.status)];
}
function streamCell(v){ if(!v)return '—';
  if(/PAUSED/.test(v))return el('span',{class:'bad'},v);
  if(/caught up|0 bytes|connected/.test(v))return el('span',{},v);
  return v;
}
function progress(v){
  const m=/\((\d+)%\)/.exec(v); const pct=m?+m[1]:null;
  const box=el('div',{},el('span',{},v));
  if(pct!=null)box.append(el('div',{class:'bar'},el('i',{style:'width:'+pct+'%'})));
  return box;
}
function metaItem(k,v,cls=''){return el('div',{},el('dt',{},k),el('dd',{class:cls},v));}

function patchSource(name){
  const card=$('#src-'+cssid(name)); if(!card)return;
  const r=branches.find(b=>b.name===name); if(!r)return;
  const meta=$('#meta-'+cssid(name)); if(meta)meta.replaceChildren(...sourceMeta(r));
  const s=statusCache[name];
  if(s&&s.stream&&/PAUSED/.test(s.stream)){card.className='card paused';}
}

function branchRow(k){
  const led=el('span',{class:'stateled '+stateClass(k)});
  const row=el('div',{class:'brow'});
  row.append(led,el('span',{class:'bname'},k.name),
    el('span',{class:'tag'},k.engine==='sqlite'?'file':k.status),
    el('span',{class:'spacer'}));
  if(k.engine!=='sqlite'||k.url)row.append(btn('sm','copy url',()=>copyURL(k.name)));
  row.append(btn('sm','open',()=>revealURL(k.name)));
  if(k.status==='running')row.append(btn('sm ghost','stop',()=>run(['stop',k.name],'Suspending '+k.name)));
  row.append(btn('sm ghost','reset',()=>run(['reset',k.name],'Resetting '+k.name)));
  row.append(btn('sm ghost danger','rm',()=>confirmDelete(k.name,false)));
  return row;
}

function btn(cls,label,fn,extra=''){return el('button',{class:'btn '+cls+(extra?' '+extra:''),onclick:fn},label);}
const cssid=s=>s.replace(/[^a-z0-9]/gi,'_');

/* ---------- actions ---------- */
async function run(args,verb){
  toast('ok',verb+'…');
  try{ const out=await cmd(args); toast('ok',verb.replace(/…$/,'')+' — done',out&&out.length<400?out:''); await load(); }
  catch(e){ toast('err',verb.replace(/…$/,'')+' failed',e.message); }
}
async function copyURL(name){
  try{ const u=await cmd(['url',name]); await navigator.clipboard.writeText(u); toast('ok','URL copied',u); }
  catch(e){ toast('err','Could not get URL',e.message); }
}

/* ---------- modals ---------- */
function modal(node){ const scrim=el('div',{class:'scrim',onclick:e=>{if(e.target===scrim)scrim.remove();}},node);
  $('#modalHost').append(scrim); return scrim; }
function foot(...b){return el('div',{class:'foot'},...b);}

function openConn(){
  const base=el('input',{value:cfg.base,placeholder:'https://anybranch.example.com  (blank = same origin)'});
  const tok=el('input',{value:cfg.token,placeholder:'server access token (ANYBRANCH_TOKEN)',type:'password'});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'Connection'),
    el('div',{class:'sub'},'The console calls the anybranch server. Same-origin needs no address; a remote address needs the server to allow this origin.'),
    el('div',{class:'body'},el('label',{},'Server address'),base,
      el('label',{},'Access token'),tok,
      el('div',{class:'hint'},'Stored only in this browser (localStorage). Never sent anywhere but your server.')),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Cancel'),
      el('button',{class:'btn primary',onclick:()=>{cfg.base=base.value.trim();cfg.token=tok.value.trim();m.remove();load();}},'Save & connect'))));
}

function openNewSource(){
  const engine=el('select',{},...['postgres','mysql','mongodb'].map(e=>el('option',{value:e},e)));
  const name=el('input',{placeholder:'source name, e.g. prod'});
  const url=el('input',{placeholder:'postgresql://user:pass@host:5432/db'});
  const schemas=el('input',{placeholder:'public  (postgres only, comma-separated)',value:'public'});
  const result=el('div');
  const preBtn=el('button',{class:'btn',onclick:doPreflight},'Run preflight');
  const cloneBtn=el('button',{class:'btn primary',disabled:'',onclick:doClone},'Clone source');
  async function doPreflight(){
    result.replaceChildren(el('div',{style:'color:var(--muted)'},el('span',{class:'spin'}),' preflighting…'));
    try{
      const rep=await cmdJSON(['preflight',engine.value,url.value.trim(),...(engine.value==='postgres'?['--schemas',schemas.value.trim()||'public']:[])]);
      renderChecklist(result,rep); cloneBtn.disabled=!rep.passed;
    }catch(e){ result.replaceChildren(el('div',{class:'check fail'},el('span',{class:'m'},'✗'),el('div',{},el('div',{class:'n'},'preflight error'),el('div',{class:'fix'},e.message)))); }
  }
  async function doClone(){
    if(!name.value.trim())return toast('err','Name required');
    m.remove(); toast('ok','Cloning '+name.value+'…','initial copy runs in the background');
    try{ const out=await cmd(['clone',name.value.trim(),url.value.trim()]); toast('ok','Source '+name.value+' created',out); await load(); }
    catch(e){ toast('err','Clone failed',e.message); }
  }
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'New source'),
    el('div',{class:'sub'},'Preflight checks the database read-only and prints exactly what to fix. Nothing is created until you clone.'),
    el('div',{class:'body'},
      el('div',{class:'row2'},el('div',{},el('label',{},'Engine'),engine),el('div',{},el('label',{},'Name'),name)),
      el('label',{style:'margin-top:14px'},'Connection string'),url,
      el('label',{style:'margin-top:14px'},'Schemas'),schemas,
      result),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Cancel'),preBtn,cloneBtn)));
}
function renderChecklist(host,rep){
  const box=el('div',{style:'margin-top:16px'});
  for(const c of rep.checks||[]){
    box.append(el('div',{class:'check '+c.state},
      el('span',{class:'m'},c.state==='pass'?'✓':c.state==='warn'?'!':'✗'),
      el('div',{},el('span',{class:'n'},c.name),c.detail?el('span',{class:'d'},' — '+c.detail):null,
        c.fix&&c.state!=='pass'?el('div',{class:'fix'},c.fix):null)));
  }
  if(rep.grant_script&&rep.grant_script.trim())box.append(el('div',{class:'grant'},rep.grant_script.trim()));
  box.append(el('div',{style:'margin-top:12px;color:'+(rep.passed?'var(--green)':'var(--red)')},
    rep.passed?'✓ Preflight passed — ready to clone.':'✗ Preflight did not pass — fix the above, then re-run.'));
  host.replaceChildren(box);
}

function openNewBranch(from){
  const name=el('input',{placeholder:'branch name, e.g. fix-orders'});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'New branch'),
    el('div',{class:'sub'},'Copy-on-write clone of '+esc(from)+' with its own server and credentials. Ready in seconds regardless of size.'),
    el('div',{class:'body'},el('label',{},'Branch name'),name,
      el('div',{class:'hint'},'1–40 chars: letters, digits, - _ . — not starting with . or _')),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Cancel'),
      el('button',{class:'btn primary',onclick:async()=>{
        if(!name.value.trim())return; m.remove(); toast('ok','Branching '+from+'…');
        try{ const j=await cmdJSON(['create',name.value.trim(),'--from',from]); toast('ok','Branch '+j.name+' ready',j.url||''); await load(); }
        catch(e){ toast('err','Branch failed',e.message); }
      }},'Create branch'))));
  setTimeout(()=>name.focus(),50);
}

async function openSettings(root){
  let text='';
  try{ text=await cmd(['settings',root]); }catch(e){ text=e.message; }
  const dbInput=el('input',{placeholder:'default database name'});
  const hookName=el('input',{placeholder:'hook name, e.g. 10-anonymize'});
  const ta='font-family:var(--mono);font-size:12px;color:var(--ink);background:#0a0908;border:1px solid var(--line);border-radius:0;padding:9px 11px;width:100%;min-height:76px;resize:vertical';
  const hookSql=el('textarea',{placeholder:"SQL to run once per new branch — e.g. update users set email='redacted';",style:ta});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'Settings · '+esc(root)),
    el('div',{class:'sub'},'Per-source defaults applied to every new branch.'),
    el('div',{class:'body'},
      el('label',{},'Current'),el('div',{class:'grant'},text||'(none)'),
      el('label',{style:'margin-top:16px'},'Default database'),dbInput,
      el('div',{style:'margin-top:8px'},el('button',{class:'btn sm',onclick:async()=>{
        if(!dbInput.value.trim())return; m.remove(); await run(['settings',root,'set','default_db',dbInput.value.trim()],'Setting default_db');
      }},'Set default_db')),
      el('label',{style:'margin-top:20px'},'branch_sql hook'),hookName,el('div',{style:'height:8px'}),hookSql,
      el('div',{class:'hint'},'Runs once on every new branch, right after creation — never on the source. Hooks run in name order; use for seeding or anonymization.')),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Close'),
      el('button',{class:'btn primary',onclick:async()=>{
        if(!hookName.value.trim()||!hookSql.value.trim())return toast('err','A hook needs a name and SQL');
        m.remove(); await run(['settings',root,'set','branch_sql',hookSql.value,'--hook',hookName.value.trim()],'Adding branch_sql hook');
      }},'Add hook'))));
  setTimeout(()=>dbInput.focus(),50);
}

function confirmDelete(name,isRoot){
  const conf=el('input',{placeholder:'type '+name+' to confirm'});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'Delete '+esc(name)),
    el('div',{class:'sub'},isRoot
      ?'This removes the replica and releases its slot, publication, and triggers on the source database. The production data is untouched.'
      :'This deletes the branch and everything written on it. Cannot be undone.'),
    el('div',{class:'body'},el('label',{},'Confirm name'),conf),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Cancel'),
      el('button',{class:'btn danger',onclick:async()=>{
        if(conf.value.trim()!==name)return toast('err','Name does not match');
        m.remove(); await run(['rm',name],'Deleting '+name);
      }},'Delete'))));
  setTimeout(()=>conf.focus(),50);
}

async function revealURL(name){
  let u=''; try{ u=await cmd(['url',name]); }catch(e){ return toast('err','No URL',e.message); }
  const inp=el('input',{value:u,readonly:''});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'Connection URL · '+esc(name)),
    el('div',{class:'sub'},'Use exactly as shown. This branch URL has no access to production.'),
    el('div',{class:'body'},inp),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Close'),
      el('button',{class:'btn primary',onclick:()=>{navigator.clipboard.writeText(u);toast('ok','Copied');}},'Copy'))));
  setTimeout(()=>inp.select(),50);
}

/* ---------- wire up ---------- */
$('#connBtn').onclick=openConn;
$('#newSourceBtn').onclick=openNewSource;
$('#refreshBtn').onclick=load;
window.addEventListener('keydown',e=>{if(e.key==='r'&&!/input|select|textarea/i.test(document.activeElement.tagName))load();});
function openNewSourceGuard(){ if(!cfg.token&&!cfg.base){openConn();}else openNewSource(); }
$('#newSourceBtn').onclick=openNewSourceGuard;

/* ---------- stat cards + sidebar views ---------- */
function setStat(id,v){const n=$(id);if(n)n.textContent=v;}
function renderStats(disc){
  if(disc||!$('#healthDot').classList.contains('ok')){setStat('#stSources','—');setStat('#stBranches','—');setStat('#stEngines','—');return;}
  const roots=branches.filter(b=>!b.parent||b.parent==='-');
  setStat('#stSources',roots.length);
  setStat('#stBranches',branches.length-roots.length);
  setStat('#stEngines',new Set(branches.map(b=>b.engine)).size||0);
}
const _render=render; render=function(){const r=_render.apply(this,arguments);try{renderStats();}catch(e){}return r;};
const _rdisc=renderDisconnected; renderDisconnected=function(){const r=_rdisc.apply(this,arguments);try{renderStats(true);}catch(e){}return r;};
function setView(v){
  const c=$('.content'); if(c)c.className='content v-'+v;
  $$('.nav-i[data-view]').forEach(b=>b.classList.toggle('active',b.dataset.view===v));
  const t=$('#pageTitle'); if(t)t.textContent=v[0].toUpperCase()+v.slice(1);
}
$$('.nav-i[data-view]').forEach(b=>b.onclick=()=>setView(b.dataset.view));
$('#navSettings').onclick=()=>{const roots=branches.filter(b=>!b.parent||b.parent==='-');if(!roots.length)return toast('err','No source yet','Connect a source to edit its branch settings.');openSettings(roots[0].name);};

// live refresh of statuses every 6s (only status calls, cheap)
setInterval(()=>{ if($('#healthDot').classList.contains('ok'))
  for(const b of branches.filter(x=>x.synced))
    cmdJSON(['status',b.name]).then(s=>{statusCache[b.name]=s;patchSource(b.name);}).catch(()=>{});
},6000);

load();
  return () => { __timers.forEach((id) => clearInterval(id)); };
}
