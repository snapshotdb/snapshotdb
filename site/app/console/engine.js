/* eslint-disable */
// Browser-only imperative console. React owns mounting; this module owns its DOM and requests.
export function initConsole() {
  const __timers = [];
  let disposed = false;
  let connection = new AbortController();
  const lifecycle = new AbortController();
  const setInterval = (...a) => { const id = globalThis.setInterval(...a); __timers.push(id); return id; };
const $=(s,r=document)=>r.querySelector(s); const $$=(s,r=document)=>[...r.querySelectorAll(s)];
const el=(t,a={},...k)=>{const n=document.createElement(t);for(const[x,v]of Object.entries(a)){
  if(x==='class')n.className=v;else if(x==='html')n.innerHTML=v;else if(x.startsWith('on'))n.addEventListener(x.slice(2),v);else n.setAttribute(x,v);}
  for(const c of k.flat())if(c!=null)n.append(c.nodeType?c:document.createTextNode(c));return n;};
const esc=s=>String(s??'').replace(/[&<>]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]));

/* ---------- config ---------- */
let plan=null;
const cfg={
  get byoc(){return localStorage.getItem('ab_mode')==='byoc'},
  set byoc(v){localStorage.setItem('ab_mode',v?'byoc':'hosted')},
  get base(){return localStorage.getItem('ab_base')||''},   // '' = same origin
  set base(v){localStorage.setItem('ab_base',v)},
  get token(){return localStorage.getItem('ab_token')||''},
  set token(v){localStorage.setItem('ab_token',v)},
};
const apiURL=p=>cfg.byoc ? (cfg.base.replace(/\/$/,''))+p : '/api/hosted'+p;

/* ---------- API: POST a command, poll the job ---------- */
async function raw(path,opts={}){
  const r=await fetch(apiURL(path),{...opts,redirect:'error',signal:AbortSignal.any([lifecycle.signal,connection.signal,AbortSignal.timeout(15000)]),headers:{...(cfg.byoc?{'Authorization':'Bearer '+cfg.token}:{}),...(opts.headers||{})}});
  return r;
}
async function health(){
  const r=await raw('/v1/health'); if(!r.ok)throw new Error('HTTP '+r.status);
  return r.json();
}
async function cmd(args,allowedExitCodes=[0]){
  const requestConnection=connection;
  const r=await raw('/v1/commands',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(args)});
  if(r.status===401)throw new Error('unauthorized — check the access token');
  if(!r.ok){let m='HTTP '+r.status;try{m=(await r.json()).error||m}catch{}if(r.status===402)openBilling(m);throw new Error(m);}
  const {id}=await r.json();
  for(let i=0;i<600;i++){
    await new Promise(z=>setTimeout(z,i<10?150:400));
    if(disposed||requestConnection!==connection)throw new Error('Connection changed');
    const jr=await raw('/v1/jobs/'+id); if(!jr.ok)throw new Error(jr.status===401?'unauthorized — check the access token':'Job lookup failed: HTTP '+jr.status);
    const j=await jr.json();
    if(j.state==='done'){
      if(!allowedExitCodes.includes(j.exit_code)){const message=(j.stderr||j.stdout||'command failed').trim();if(message.includes('PLAN_LIMIT:')&&!cfg.byoc)openBilling(message.replace('PLAN_LIMIT:',''));throw new Error(message);}
      return (j.stdout||'').trim();
    }
  }
  throw new Error('timed out waiting for the server job');
}
const cmdJSON=async (a,allowedExitCodes)=>{const s=await cmd([...a,'--format','json'],allowedExitCodes);return JSON.parse(s||'null');};

/* ---------- toast ---------- */
function toast(kind,title,msg){
  if(disposed)return;
  const t=el('div',{class:'toast '+kind},el('div',{class:'t'},title),msg?el('div',{class:'m'},msg):null);
  $('#toasts').append(t); setTimeout(()=>{t.style.transition='.3s';t.style.opacity='0';setTimeout(()=>t.remove(),300)},kind==='err'?7000:3800);
}

/* ---------- state ---------- */
let branches=[]; let statusCache={}; let loadId=0;

async function load(){
  const id=++loadId;
  try{
    const h=await health();
    if(disposed||id!==loadId)return;
    $('#ver').textContent='v'+(h.version||'?');
    $('#healthDot').className='dot ok'; $('#healthText').textContent=(cfg.byoc?'BYOC':'SnapshotDB Cloud')+' · online';
    $('#deploymentBtn').textContent='Deployment · '+(cfg.byoc?'BYOC':'hosted');
    if(!cfg.byoc){const pr=await raw('/v1/account');if(!pr.ok)throw new Error('Could not load plan usage');plan=await pr.json();renderUsage();}else{plan=null;renderUsage();}
  }catch(e){
    if(disposed||id!==loadId)return;
    $('#healthDot').className='dot bad'; $('#healthText').textContent='offline';
    return renderDisconnected(e.message);
  }
  try{
    const next=await cmdJSON(['list'])||[];
    if(disposed||id!==loadId)return;
    branches=next;
  }catch(e){ if(disposed||id!==loadId)return; return renderDisconnected(e.message); }
  render();
  // fetch replication status for synced sources
  for(const b of branches.filter(x=>x.synced)){
    cmdJSON(['status',b.name]).then(s=>{if(disposed)return;statusCache[b.name]=s;patchSource(b.name);}).catch(()=>{});
  }
}

function renderDisconnected(msg){
  if(disposed)return;
  branches=[];statusCache={};
  $('#healthDot').className='dot bad'; $('#healthText').textContent='not connected';
  $('#stage').replaceChildren(el('div',{class:'empty'},
    el('h3',{},'Not connected'),
    el('p',{},msg||'Set the server address and access token.'),
    el('div',{style:'margin-top:14px'},el('button',{class:'btn primary',onclick:cfg.byoc?openConn:load},cfg.byoc?'BYOC connection settings':'Retry hosted connection'))));
}

function groupBranches(){
  const roots=branches.filter(b=>!b.parent||b.parent==='-'||!branches.some(p=>p.name===b.parent));
  const kids=(p,seen=new Set([p]))=>branches.filter(b=>b.parent===p&&!seen.has(b.name)).flatMap(b=>[b,...kids(b.name,new Set([...seen,b.name]))]);
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
    el('span',{class:'badge state '+st,title:st==='suspended'?'The local copy is idle. Connecting wakes it when quota is available; your original database is unaffected.':''},st==='suspended'?'Idle · auto-resumes':st)];
  if(r.locked)badges.push(el('span',{class:'badge lock'},'🔒 locked'));
  const actions=el('div',{class:'cactions'});
  if(r.status==='suspended')card.title='This local copy is idle. Connecting resumes it when quota is available; your original remote database is unaffected.';
  if(r.synced){
    actions.append(btn('sm','repair',()=>run(['repair',r.name],'Repairing '+r.name)));
    if(r.engine==='postgres')actions.append(btn('sm','reconcile',()=>run(['reconcile',r.name],'Reconciling '+r.name)));
  }
  if(cfg.byoc||r.engine==='sqlite')actions.append(btn('sm','open',()=>revealURL(r.name)));
  const sourceActive=r.status==='running'||r.status==='syncing';
  actions.append(btn('sm',sourceActive?'stop':'start',()=>run([sourceActive?'stop':'start',r.name],'Updating '+r.name)));
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
  if(!r.synced)return [metaItem('kind','local copy'),...(r.status==='suspended'?[metaItem('compute','Paused after inactivity; connections resume it when quota is available.')]:[]),metaItem('branch from','create --from '+r.name)];
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
    el('span',{class:'tag'},(k.status==='suspended'?'Idle · auto-resumes':k.status)+' · from '+k.parent),
    el('span',{class:'spacer'}));
  if(k.status!=='snapshot'&&(k.engine!=='sqlite'||k.url))row.append(btn('sm','copy url',()=>copyURL(k.name)));
  if(k.status!=='snapshot')row.append(btn('sm','open',()=>revealURL(k.name)));
  if(k.status!=='snapshot')row.append(btn('sm ghost',k.status==='running'?'stop':'start',()=>run([k.status==='running'?'stop':'start',k.name],'Updating '+k.name)));
  row.append(btn('sm ghost','+ branch',()=>openNewBranch(k.name)));
  row.append(btn('sm ghost',k.locked?'unlock':'lock',()=>run([k.locked?'unlock':'lock',k.name],'Updating lock '+k.name)));
  if(k.status!=='snapshot'&&!branches.some(b=>b.name===k.parent&&b.status==='snapshot'))row.append(btn('sm ghost','reset',()=>run(['reset',k.name],'Resetting '+k.name)));
  row.append(btn('sm ghost danger','rm',()=>confirmDelete(k.name,false)));
  return row;
}

function btn(cls,label,fn,extra=''){return el('button',{class:'btn '+cls+(extra?' '+extra:''),onclick:fn},label);}
const cssid=s=>Array.from(s,c=>c.codePointAt(0).toString(16)).join('-');

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
function modal(node){ if(disposed)return document.createElement('div');
  node.setAttribute('role','dialog');node.setAttribute('aria-modal','true');
  const title=node.querySelector('h3');if(title)node.setAttribute('aria-label',title.textContent);
  node.querySelectorAll('label').forEach((label,i)=>{const field=label.nextElementSibling;if(field&&/INPUT|SELECT|TEXTAREA/.test(field.tagName)){field.id='modal-field-'+i;label.htmlFor=field.id;}});
  const prior=document.activeElement;
  const scrim=el('div',{class:'scrim',onclick:e=>{if(e.target===scrim)scrim.remove();}},node);
  scrim.addEventListener('keydown',e=>{if(e.key==='Escape'){scrim.remove();prior?.focus();}if(e.key==='Tab'){const fields=[...node.querySelectorAll('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled)')];const first=fields[0],last=fields.at(-1);if(e.shiftKey&&document.activeElement===first){e.preventDefault();last?.focus();}else if(!e.shiftKey&&document.activeElement===last){e.preventDefault();first?.focus();}}});
  $('#modalHost').replaceChildren(scrim); node.querySelector('input,select,button')?.focus(); return scrim; }
function foot(...b){return el('div',{class:'foot'},...b);}

function renderUsage(){
  const node=$('#usageSummary');
  if(cfg.byoc){node.textContent='BYOC · compute and storage run on your infrastructure';return;}
  if(!plan){node.textContent='';return;}
  const hours=(plan.remaining_seconds/3600).toFixed(2);
  node.replaceChildren(el('span',{},(plan.plan==='pro'?'Pro':'Free trial')+' · '+hours+' branch-hours left · '+plan.sources+' source'+(plan.sources===1?'':'s')),btn('sm ghost','View plan',()=>openBilling()));
}
function openDeployment(){
  const mode=el('select',{},el('option',{value:'hosted'},'SnapshotDB Cloud — hosted for you'),el('option',{value:'byoc'},'BYOC — use your own app server'));
  mode.value=cfg.byoc?'byoc':'hosted';
  const m=modal(el('div',{class:'modal'},el('h3',{},'Deployment'),
    el('div',{class:'sub'},'Hosted is the default. Choose BYOC only when you operate a separate SnapshotDB app server.'),
    el('div',{class:'body'},el('label',{},'Deployment mode'),mode),
    foot(btn('ghost','Cancel',()=>m.remove()),btn('primary','Save',()=>{cfg.byoc=mode.value==='byoc';connection.abort();connection=new AbortController();branches=[];statusCache={};plan=null;m.remove();renderUsage();if(cfg.byoc)openConn();else load();}),...(cfg.byoc?[btn('ghost','BYOC server settings',()=>{m.remove();openConn();})]:[]))));
}
async function checkout(button){
  button.disabled=true;
  try{const r=await fetch('/api/billing/checkout',{method:'POST'});const value=await r.json();if(!r.ok)throw new Error(value.error);window.location.assign(value.url);}
  catch(e){toast('err','Checkout unavailable',e.message);button.disabled=false;}
}
function openBilling(reason=''){
  const upgrade=el('button',{class:'btn primary',onclick:e=>checkout(e.currentTarget)},'Upgrade · $150/month');
  const pro=plan?.plan==='pro';
  const manage=el('button',{class:'btn',onclick:async e=>{
    const button=e.currentTarget;button.disabled=true;
    try{const r=await fetch('/api/billing/portal',{method:'POST'});const value=await r.json();if(!r.ok)throw new Error(value.error);window.location.assign(value.url);}
    catch(error){toast('err','Billing portal unavailable',error.message);button.disabled=false;}
  }},'Manage billing');
  const m=modal(el('div',{class:'modal'},el('h3',{},'Plan & usage'),
    el('div',{class:'sub'},reason||'Only running compute uses branch-hours. Idle copies retain their data.'),
    el('div',{class:'body'},
      el('div',{class:'plan-grid'},
        el('div',{class:'plan-card'},el('h4',{},'Free · $0'),el('p',{},'1 source database'),el('p',{},'2 total trial branch-hours'),el('p',{},'1 GiB storage · 1 GiB transfer'),el('p',{},'2 running databases')),
        el('div',{class:'plan-card'},el('h4',{},'Pro · $150/month'),el('p',{},'Unlimited source databases'),el('p',{},'300 branch-hours per billing month'),el('p',{},'50 GiB shared storage · 50 GiB transfer'),el('p',{},'4 running databases'))),
      el('p',{class:'hint'},'One database running for one hour uses one branch-hour. Running source replicas also count. Concurrent databases add together. Allowances stop compute at the limit; there are no automatic overage charges.'),
      plan?el('p',{},'Used: '+(plan.used_seconds/3600).toFixed(2)+' / '+(plan.limit_seconds/3600)+' hours'+(plan.period_end?' · Paid period ends '+new Date(plan.period_end*1000).toLocaleDateString():'')):null,
      !cfg.byoc&&plan?.customer?el('p',{class:'hint'},'Manage billing to view subscription status, update your payment method, or recover a failed payment.'):null,
      cfg.byoc?el('p',{},'BYOC uses your infrastructure and is outside hosted usage billing.'):null),
    foot(btn('ghost','Close',()=>m.remove()),...(!cfg.byoc&&!pro?[upgrade]:[]),...(!cfg.byoc&&plan?.customer?[manage]:[]),...(!cfg.byoc&&plan?.subscription?[btn('ghost','Cancel renewal',async()=>{
      if(!window.confirm('Cancel Pro renewal? You keep access until the paid period ends.'))return;
      try{const r=await fetch('/api/billing/cancel',{method:'POST'});const v=await r.json();if(!r.ok)throw new Error(v.error);toast('ok','Subscription',v.message);m.remove();}catch(e){toast('err','Cancellation failed',e.message);}
    })]:[]))));
}

function openConn(){
  if(!cfg.byoc)return openDeployment();
  const base=el('input',{value:cfg.base,placeholder:'https://your-server.example.com'});
  const tok=el('input',{value:cfg.token,placeholder:'server access token (SNAPSHOTDB_TOKEN)',type:'password'});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'BYOC connection'),
    el('div',{class:'sub'},'Connect to the SnapshotDB app server you operate. Allow this website’s origin on your server.'),
    el('div',{class:'body'},el('label',{},'Server address'),base,
      el('label',{},'Access token'),tok,
      el('div',{class:'hint'},'Stored only in this browser (localStorage). Never sent anywhere but your server.')),
    foot(el('button',{class:'btn ghost',onclick:()=>m.remove()},'Cancel'),
      el('button',{class:'btn primary',onclick:()=>{try{const value=base.value.trim();if(!value)throw new Error('Enter your BYOC server address');if(value){const u=new URL(value);if(!['http:','https:'].includes(u.protocol)||u.username||u.password||u.search||u.hash)throw new Error('Use an http(s) server address without credentials, query, or fragment');if(location.protocol==='https:'&&u.protocol!=='https:'&&!['localhost','127.0.0.1','[::1]'].includes(u.hostname))throw new Error('An HTTPS console needs an HTTPS server');}cfg.base=value;cfg.token=tok.value.trim();connection.abort();connection=new AbortController();statusCache={};branches=[];m.remove();load();}catch(e){toast('err','Could not save connection',e.message);}}},'Save & connect'))));
}

function openNewSource(){
  if(!cfg.byoc&&plan&&(plan.remaining_seconds<=0||(plan.source_limit!==null&&plan.sources>=plan.source_limit)))return openBilling('Upgrade to add another source or get more branch-hours.');
  let revision=0;let approved=-1;let busy=false;
  const engine=el('select',{},...['postgres','mysql','mongodb','sqlite'].map(e=>el('option',{value:e},e)));
  const name=el('input',{placeholder:'source name, e.g. prod'});
  const url=el('input',{placeholder:'postgresql://user:pass@host:5432/db'});
  const schemas=el('input',{placeholder:'public',value:'public'});
  const schemaLabel=el('label',{},'Schemas');
  const result=el('div');
  const preBtn=el('button',{class:'btn',onclick:doPreflight},'Run preflight');
  const cloneBtn=el('button',{class:'btn primary',disabled:'',onclick:doClone},'Clone source');
  const invalidate=()=>{revision++;approved=-1;cloneBtn.disabled=engine.value!=='sqlite';preBtn.hidden=engine.value==='sqlite';url.disabled=engine.value==='sqlite';schemas.disabled=engine.value!=='postgres';schemas.hidden=engine.value!=='postgres';schemaLabel.hidden=engine.value!=='postgres';result.replaceChildren();};
  [engine,name,url,schemas].forEach(input=>input.addEventListener('input',invalidate));
  async function doPreflight(){
    if(busy)return;busy=true;preBtn.disabled=true;cloneBtn.disabled=true;approved=-1;
    const current=revision;
    result.replaceChildren(el('div',{style:'color:var(--muted)'},el('span',{class:'spin'}),' preflighting…'));
    try{
      const rep=await cmdJSON(['preflight',engine.value,url.value.trim(),...(engine.value==='postgres'?['--schemas',schemas.value.trim()||'public']:[])],[0,2]);
      if(current!==revision)return;
      renderChecklist(result,rep);approved=rep.passed?current:-1; cloneBtn.disabled=!rep.passed;
    }catch(e){ result.replaceChildren(el('div',{class:'check fail'},el('span',{class:'m'},'✗'),el('div',{},el('div',{class:'n'},'preflight error'),el('div',{class:'fix'},e.message)))); }finally{busy=false;preBtn.disabled=false;}
  }
  async function doClone(){
    if(busy)return;
    if(engine.value!=='sqlite'&&approved!==revision)return toast('err','Run preflight again','Source settings have changed.');
    if(!name.value.trim())return toast('err','Name required');
    busy=true;m.remove(); toast('ok','Creating '+name.value+'…');
    try{ const args=engine.value==='sqlite'?['import','sqlite',name.value.trim(),'--new']:['sync',engine.value,name.value.trim(),url.value.trim(),...(engine.value==='postgres'?['--schemas',schemas.value.trim()||'public']:[])];const out=await cmd(args); toast('ok','Source '+name.value+' created',out); await load(); }
    catch(e){ toast('err','Clone failed',e.message); }
  }
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'New source'),
    el('div',{class:'sub'},'Preflight checks a live database before cloning. SQLite creates an empty source you can populate using its connection URL.'),
    el('div',{class:'body'},
      el('div',{class:'row2'},el('div',{},el('label',{},'Engine'),engine),el('div',{},el('label',{},'Name'),name)),
      el('label',{style:'margin-top:14px'},'Connection string'),url,
      schemaLabel,schemas,
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
  if(!cfg.byoc&&plan&&plan.remaining_seconds<=0)return openBilling('Your branch-hour allowance is exhausted.');
  const name=el('input',{placeholder:'branch name, e.g. fix-orders'});
  const m=modal(el('div',{class:'modal'},
    el('h3',{},'New branch'),
    el('div',{class:'sub'},'Copy-on-write clone of '+esc(from)+' with its own server and credentials. Prepared pools avoid waiting for a cold database startup.'),
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
  if(!cfg.byoc){modal(el('div',{class:'modal'},el('h3',{},'Hosted source settings'),el('div',{class:'body'},el('p',{},'Hosted databases use managed defaults: 1 vCPU, 2 GiB memory per running database. Advanced engine hooks are available in BYOC mode.')),foot(btn('ghost','Close',()=>$('#modalHost').replaceChildren()))));return;}
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
      el('button',{class:'btn primary',onclick:async()=>{try{await navigator.clipboard.writeText(u);toast('ok','Copied');}catch{inp.select();toast('err','Copy failed','Select and copy the URL manually.');}}},'Copy'))));
  setTimeout(()=>inp.select(),50);
}

/* ---------- wire up ---------- */
$('#deploymentBtn').onclick=openDeployment;
$('#billingBtn').onclick=()=>openBilling();
$('#newSourceBtn').onclick=openNewSource;
$('#refreshBtn').onclick=load;
const onKey=e=>{if(e.key==='r'&&!e.metaKey&&!e.ctrlKey&&!e.altKey&&!document.activeElement.isContentEditable&&!/input|select|textarea/i.test(document.activeElement.tagName))load();};
window.addEventListener('keydown',onKey);
function openNewSourceGuard(){ if(cfg.byoc&&(!cfg.token||!cfg.base)){openConn();}else openNewSource(); }
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

setInterval(async()=>{if(disposed||document.hidden)return;try{
  const next=await cmdJSON(['list']);if(disposed)return;branches=next||[];render();
  if(!cfg.byoc){const response=await raw('/v1/account');if(response.ok){plan=await response.json();if(!disposed)renderUsage();}}
}catch{}},15000);
// live refresh of statuses every 6s (only status calls, cheap)
setInterval(()=>{ if($('#healthDot').classList.contains('ok'))
  for(const b of branches.filter(x=>x.synced))
    cmdJSON(['status',b.name]).then(s=>{if(disposed)return;statusCache[b.name]=s;patchSource(b.name);}).catch(()=>{});
},6000);

// Back from Dodo checkout. Access comes from the webhook; the 15 s refresh above shows it.
{const q=new URLSearchParams(location.search);if(q.get('billing')==='return'){history.replaceState(null,'',location.pathname);
  const ok=q.get('status')==='active';toast(ok?'ok':'err',ok?'Payment received':'Checkout not completed',ok?'Pro activates within a few seconds.':'No subscription was started. You can retry from Plan & usage.');}}
load();
  return () => { disposed=true;lifecycle.abort();connection.abort();window.removeEventListener('keydown',onKey);__timers.forEach((id) => clearInterval(id));$('#modalHost')?.replaceChildren(); };
}
