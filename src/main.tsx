import React,{createElement,useCallback,useEffect,useMemo,useRef,useState} from 'react';
import {createPortal} from 'react-dom';
import {createRoot} from 'react-dom/client';
import {AnimatePresence,motion} from 'motion/react';
import {Activity,ArrowRight,BookMarked,Check,ChevronLeft,ChevronRight,ClipboardCopy,Clock,Code2,Copyright,Download,FileText,Gauge,Globe2,History,Languages,LayoutGrid,Loader2,Mail,Moon,Pause,Play,Radio,RefreshCw,Search,Settings2,Sparkles,Star,Sun,Table2,Terminal,Trash2,Volume2,X,Scale,Info,Power} from 'lucide-react';
import {call,clearHistory,confirmClose,deleteHistory,emitToHud,history as readHistory,inTauri,listenEvent,metrics as readMetrics,models as readModels,readSettings,saveSettings,showMain,speak,stopSpeak,translate as requestTranslate,vocab as readVocab} from './api';
import {Logo} from './components/Logo';
import type {AppSettings,CandidateSnapshot,HistoryRow,Metrics,ModelRecord,ProviderProfile,Translation,VocabRow} from './types';
import {LANGUAGES,MODEL_PRESETS} from './types';
import {EDGE_VOICES,edgeVoiceFor,speakSample} from './tts';
import './styles/main.css';
import './styles/hud.css';

type Page='home'|'history'|'vocab'|'settings'|'about';
type SettingsTab='basic'|'models'|'diagnostics';
type HistoryView='cards'|'table';
type IconType=typeof Activity;

const NAV:[Page,string,IconType][]=[['home','首页',Sparkles],['history','输入记录',History],['vocab','生词本',BookMarked],['settings','设置',Settings2],['about','关于',Globe2]];
const SHORT:Record<string,string>={zh:'中文',en:'英语',ja:'日语',ko:'韩语',fr:'法语',de:'德语',es:'西班牙语',ru:'俄语',it:'意大利语',pt:'葡萄牙语',ar:'阿拉伯语',th:'泰语'};
const DEMO_MODE=!inTauri()&&new URLSearchParams(window.location.search).get('demo')==='1';
const demoTime=(minutes:number)=>new Date(Date.now()-minutes*60000).toISOString();
const DEMO_HISTORY:HistoryRow[]=[
 {id:101,original:'你好',translated:'Hello',source_lang:'zh',target_lang:'en',engine:'ollama · translategemma:4b',elapsed_ms:42,created_at:demoTime(2)},
 {id:102,original:'谢谢',translated:'Thank you',source_lang:'zh',target_lang:'en',engine:'ollama · translategemma:4b',elapsed_ms:38,created_at:demoTime(8)},
 {id:103,original:'今天天气很好',translated:'The weather is lovely today.',source_lang:'zh',target_lang:'en',engine:'local · cedict',elapsed_ms:61,created_at:demoTime(64)},
 {id:104,original:'再见',translated:'Goodbye',source_lang:'zh',target_lang:'en',engine:'ollama · translategemma:4b',elapsed_ms:27,created_at:demoTime(142)},
 {id:105,original:'我们一起学习吧',translated:"Let's study together.",source_lang:'zh',target_lang:'en',engine:'local · cedict',elapsed_ms:54,created_at:demoTime(1490)},
 {id:106,original:'没关系',translated:"It's all right.",source_lang:'zh',target_lang:'en',engine:'ollama · translategemma:4b',elapsed_ms:35,created_at:demoTime(2880)}
];
const DEMO_VOCAB:VocabRow[]=[
 {source:'你好',target_lang:'en',translated:'Hello',repetitions:3,interval_days:4,due_at:demoTime(-1440)},
 {source:'谢谢',target_lang:'en',translated:'Thank you',repetitions:2,interval_days:2,due_at:demoTime(-720)},
 {source:'今天天气很好',target_lang:'en',translated:'The weather is lovely today.',repetitions:1,interval_days:1,due_at:demoTime(-60)},
 {source:'再见',target_lang:'ja',translated:'さようなら',repetitions:1,interval_days:1,due_at:demoTime(240)}
];
const THEMES=[
 {id:'blossom',name:'花瓣',accent:'#ff5fa8',deep:'#5032c6',darkAccent:'#ff7ab8',darkDeep:'#9b85ff'},
 {id:'ocean',name:'海盐',accent:'#4f8cff',deep:'#2847c9',darkAccent:'#6fa4ff',darkDeep:'#7f9dff'},
 {id:'mint',name:'薄荷',accent:'#2fbf9b',deep:'#0f7a62',darkAccent:'#4dd6b0',darkDeep:'#69e0c0'},
 {id:'sunset',name:'琥珀',accent:'#f79a3e',deep:'#b2550b',darkAccent:'#ffb25e',darkDeep:'#ffd08a'},
];
const DENSITIES=[{id:'compact',name:'紧凑'},{id:'cozy',name:'舒适'},{id:'roomy',name:'宽松'}];
const clock=(iso:string)=>{const d=new Date(iso);return Number.isNaN(d.getTime())?'':d.toLocaleString('zh-CN',{month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit'});};
const dir=(s:string,t:string)=>`${LANGUAGES[s]??s} → ${LANGUAGES[t]??t}`;

/* ---------------------------------------------------------------- HUD view */
function HudView(){
 useEffect(()=>{const stop=(e:Event)=>e.preventDefault();document.addEventListener('contextmenu',stop);return()=>document.removeEventListener('contextmenu',stop)},[]);
 const [candidate,setCandidate]=useState<CandidateSnapshot|null>(null);
 const [translation,setTranslation]=useState<Translation|null>(null);
 const [error,setError]=useState('');
 const [settings,setSettings]=useState<AppSettings|null>(null);
 const speechEnabled=useRef(false);
 const latestTranslation=useRef<Translation|null>(null);
 const settingsRef=useRef<AppSettings|null>(null);
 useEffect(()=>{
  let alive=true;const unsubs:Array<()=>void>=[];
  const init=async()=>{
   const s=await readSettings();
   if(alive){speechEnabled.current=s.auto_speech;settingsRef.current=s;setSettings(s)}
   for(const [name,cb] of [
    ['rino:candidate',(v:unknown)=>{setCandidate(v as CandidateSnapshot);setTranslation(null);latestTranslation.current=null;setError('')}],
    ['rino:translation',(v:unknown)=>{const t=v as Translation;latestTranslation.current=t;setTranslation(t);setError('');if(speechEnabled.current)void speak(t.translated,t.target,settingsRef.current??undefined)}],
    ['rino:translation-error',(v:unknown)=>setError(String(v))],
    ['rino:settings',(v:unknown)=>{const next=v as AppSettings;const was=speechEnabled.current;speechEnabled.current=next.auto_speech;settingsRef.current=next;if(!next.auto_speech)stopSpeak();else if(!was&&latestTranslation.current)void speak(latestTranslation.current.translated,latestTranslation.current.target,next);setSettings(next)}],
    ['rino:hide',()=>{setCandidate(null);setTranslation(null);latestTranslation.current=null;setError('')}]
   ] as Array<[string,(v:unknown)=>void]>){unsubs.push(await listenEvent(name,cb));}
   try{const snapshot=await call<CandidateSnapshot|null>('current_candidate');if(alive&&snapshot)setCandidate(snapshot)}catch{ /* preview */ }
  };init().catch(()=>{});
  return()=>{alive=false;unsubs.forEach(u=>u());stopSpeak()};
 },[]);
 // Only the translation: the source already sits in the IME bar right above.
 const shown=translation?.translated??(error?'—':'…');
 const fontPx=settings?.font_size??16;
 const measured=useMemo(()=>{
  try{
   const ctx=document.createElement('canvas').getContext('2d');
   if(!ctx) return {width:180,height:46};
   ctx.font=`750 ${fontPx}px 'DM Sans','Segoe UI','Microsoft YaHei UI',sans-serif`;
   return {width:Math.min(520,Math.max(88,Math.ceil(ctx.measureText(shown).width)+30)),height:Math.max(36,Math.round(fontPx*1.42)+16)};
  }catch{return {width:180,height:46}}
 },[shown,fontPx]);
 useEffect(()=>{if(!candidate)return;Promise.resolve(call('hud_resize',{width:measured.width,height:measured.height,chars:shown.length})).catch(()=>{});},[candidate,shown,measured.width,measured.height]);
 return <div className="hud-root" style={{fontSize:fontPx}} aria-live="polite">
  {candidate&&<div className="hud-shell hud-solo" style={{width:measured.width,height:measured.height}}><strong className="hud-translated">{shown}</strong></div>}
 </div>
}

/* ------------------------------------------------------------ UI primitives */
function PageHead({title,sub,right}:{title:string;sub?:string;right?:React.ReactNode}){
 return <div className="page-head"><div className="page-title"><h1>{title}</h1>{sub&&<p>{sub}</p>}</div>{right&&<div className="page-head-right">{right}</div>}</div>
}
function Panel({title,note,right,children,className=''}:{title?:string;note?:string;right?:React.ReactNode;children:React.ReactNode;className?:string}){
 return <section className={'panel '+className}>
  {(title||right)&&<div className="panel-head"><div className="panel-copy">{title&&<h2>{title}</h2>}{note&&<p>{note}</p>}</div>{right}</div>}
  {children}
 </section>
}
function Row({label,hint,children,noLine=false}:{label:string;hint?:string;children:React.ReactNode;noLine?:boolean}){
 return <div className={noLine?'row row-plain':'row'}><div className="row-copy"><strong>{label}</strong>{hint&&<small>{hint}</small>}</div><div className="row-control">{children}</div></div>
}
function Switch({checked,onChange,label}:{checked:boolean;onChange:(v:boolean)=>void;label:string}){
 const ref=useRef<HTMLElement|null>(null);
 useEffect(()=>{
  const el=ref.current; if(!el)return;
  const handle=()=>onChange(Boolean((el as HTMLElement&{checked?:boolean}).checked));
  el.addEventListener('change',handle);
  return()=>el.removeEventListener('change',handle);
 },[onChange]);
 useEffect(()=>{const el=ref.current as (HTMLElement&{checked?:boolean})|null;if(el&&el.checked!==checked)el.checked=checked;},[checked]);
 return <span className="vfx-switch">{createElement('sakuga-toggle',{ref,label,checked:checked?'':undefined})}</span>;
}
function SwitchRow({title,desc,checked,onChange,noLine=false}:{title:string;desc?:string;checked:boolean;onChange:(v:boolean)=>void;noLine?:boolean}){
 return <Row label={title} hint={desc} noLine={noLine}><Switch checked={checked} onChange={onChange} label={title}/></Row>
}
function Segmented<T extends string>({items,value,onChange}:{items:Array<{id:T;name:string}>;value:T;onChange:(v:T)=>void}){
 return <div className="segmented">{items.map(it=><button key={it.id} className={it.id===value?'seg on':'seg'} onClick={()=>onChange(it.id)}>{it.name}</button>)}</div>
}
function IconButtons<T extends string>({items,value,onChange}:{items:Array<{id:T;icon:IconType;title:string}>;value:T;onChange:(v:T)=>void}){
 return <div className="icon-seg">{items.map(({id,icon:Icon,title})=><button key={id} className={id===value?'iseg on':'iseg'} title={title} aria-label={title} onClick={()=>onChange(id)}><Icon size={15}/></button>)}</div>
}
function IconBtn({icon:Icon,title,onClick,className=''}:{icon:IconType;title:string;onClick:()=>void;className?:string}){
 return <button className={'icon-btn '+className} title={title} aria-label={title} onClick={onClick}><Icon size={15}/></button>
}
type ContextMenuItem={divider:true}|{divider?:false;label:string;icon?:IconType;onClick:()=>void;checked?:boolean;danger?:boolean;disabled?:boolean};
function ContextMenu({x,y,items,scale,onClose}:{x:number;y:number;items:ContextMenuItem[];scale:number;onClose:()=>void}){
 const ref=useRef<HTMLDivElement|null>(null);
 useEffect(()=>{
  const esc=(e:KeyboardEvent)=>{if(e.key==='Escape')onClose()};
  window.addEventListener('keydown',esc);
  requestAnimationFrame(()=>ref.current?.focus());
  return()=>window.removeEventListener('keydown',esc);
 },[onClose]);
 return createPortal(
  <div className="ctx-layer" onPointerDown={onClose} onContextMenu={e=>{e.preventDefault();onClose()}}>
   <div ref={ref} className="ctx-menu" role="menu" tabIndex={-1}
    style={{left:x,top:y,transform:`scale(${scale})`,maxHeight:Math.max(220,(window.innerHeight-16)/scale)}}
    onPointerDown={e=>e.stopPropagation()}>
    {items.map((item,i)=>{
     if(item.divider)return <div className="ctx-sep" key={'sep-'+i}/>;
     const Icon=item.icon;
     return <button key={item.label+'-'+i} type="button" role="menuitem" aria-checked={item.checked} disabled={item.disabled}
      className={'ctx-item'+(item.danger?' danger':'')} onClick={()=>{onClose();item.onClick()}}>
      <span className="ctx-icon">{Icon&&<Icon size={14}/>}</span><span>{item.label}</span>{item.checked&&<Check size={13} className="ctx-check"/>}
     </button>;
    })}
   </div>
  </div>,document.body);
}
function Stat({icon:Icon,value,label,tone=''}:{icon:IconType;value:string|number;label:string;tone?:string}){
 return <div className={'metric '+tone} title={label}><span className="metric-icon"><Icon size={13}/></span><b>{value}</b></div>
}
function Empty({icon:Icon,title,text}:{icon:IconType;title:string;text:string}){
 return <div className="empty"><Icon size={24}/><strong>{title}</strong><p>{text}</p></div>
}
function StarButton({on,onClick,titled='收藏到生词本'}:{on:boolean;onClick:()=>void;titled?:string}){
 return <button className={on?'star-btn on':'star-btn'} title={on?'已收藏，点击取消':titled} aria-label={titled} onClick={onClick}>
  <motion.span animate={on?{scale:[1,1.34,1],rotate:[0,-16,0]}:{scale:1}} transition={{duration:.32,ease:[.2,1,.4,1]}}><Star size={14}/></motion.span>
  {on&&<motion.span className="star-ring" initial={{scale:.5,opacity:.55}} animate={{scale:1.85,opacity:0}} transition={{duration:.45}}/>}
 </button>
}

/* ------------------------------------------------------------- Application */
const fitUiScale=()=>Math.max(1,Math.min(window.innerWidth/720,window.innerHeight/600));
function AboutParticles(){
 const ref=useRef<HTMLCanvasElement|null>(null);
 useEffect(()=>{
  const canvas=ref.current; if(!canvas)return;
  const ctx=canvas.getContext('2d'); if(!ctx)return;
  const parent=canvas.parentElement; if(!parent)return;
  let raf=0,w=0,h=0,dots:Array<{x:number;y:number;r:number;vx:number;vy:number;phase:number;spin:number;kind:number}>=[];
  let accent='#ff5fa8',deep='#5032c6';
  const palette=()=>{const s=getComputedStyle(document.documentElement);accent=s.getPropertyValue('--accent').trim()||accent;deep=s.getPropertyValue('--accent-deep').trim()||deep;};
  const resize=()=>{
   const rect=parent.getBoundingClientRect(); const dpr=Math.min(2,window.devicePixelRatio||1);
   w=Math.max(1,rect.width); h=Math.max(1,rect.height);
   canvas.width=Math.round(w*dpr); canvas.height=Math.round(h*dpr);
   canvas.style.width=w+'px'; canvas.style.height=h+'px';
   ctx.setTransform(dpr,0,0,dpr,0,0);
   const count=Math.max(18,Math.min(42,Math.round(w*h/17000)));
   dots=Array.from({length:count},(_,i)=>({x:Math.random()*w,y:Math.random()*h,r:1+Math.random()*2.4,vx:(Math.random()-.5)*8,vy:-(3+Math.random()*12),phase:Math.random()*Math.PI*2,spin:.4+Math.random()*1.8,kind:i%11===0?1:0}));
  };
  palette(); resize();
  const ro=new ResizeObserver(resize); ro.observe(parent);
  const mo=new MutationObserver(palette); mo.observe(document.documentElement,{attributes:true,attributeFilter:['data-mode','data-theme']});
  const t0=performance.now(); let last=t0;
  const frame=(now:number)=>{
   const dt=Math.min(.05,(now-last)/1000); last=now;
   ctx.clearRect(0,0,w,h);
   for(const d of dots){
    d.x+=d.vx*dt; d.y+=d.vy*dt; d.phase+=d.spin*dt;
    if(d.y<-12){d.y=h+10;d.x=Math.random()*w}
    if(d.x<-12)d.x=w+10; if(d.x>w+12)d.x=-10;
    const pulse=.22+.36*(.5+.5*Math.sin(d.phase));
    ctx.globalAlpha=pulse;
    ctx.fillStyle=d.kind?deep:accent;
    if(d.kind){
     const len=3+d.r*1.7;
     ctx.fillRect(d.x-len,d.y-.7,len*2,1.4); ctx.fillRect(d.x-.7,d.y-len,1.4,len*2);
    }else{
     ctx.beginPath();ctx.arc(d.x,d.y,d.r,0,Math.PI*2);ctx.fill();
    }
   }
   ctx.globalAlpha=.07; ctx.strokeStyle=accent; ctx.lineWidth=.7;
   for(let i=0;i<dots.length;i++)for(let j=i+1;j<dots.length;j++){
    const a=dots[i],b=dots[j],dx=a.x-b.x,dy=a.y-b.y,dist=Math.hypot(dx,dy);
    if(dist<76){ctx.beginPath();ctx.moveTo(a.x,a.y);ctx.lineTo(b.x,b.y);ctx.stroke()}
   }
   ctx.globalAlpha=1;
   raf=requestAnimationFrame(frame);
  };
  raf=requestAnimationFrame(frame);
  return()=>{cancelAnimationFrame(raf);ro.disconnect();mo.disconnect()};
 },[]);
 return <canvas ref={ref} className="about-particles" aria-hidden="true"/>;
}
function Application(){
 const [page,setPage]=useState<Page>(()=>{const p=new URLSearchParams(window.location.search).get('page');return p==='history'||p==='vocab'||p==='settings'||p==='about'?p:'home'});
 const [uiScale,setUiScale]=useState(()=>fitUiScale());
 const [settings,setSettings]=useState<AppSettings|null>(null);
 const [settingsTab,setSettingsTab]=useState<SettingsTab>(()=>{const t=new URLSearchParams(window.location.search).get('tab');return t==='models'||t==='diagnostics'?t:'basic'});
 const [live,setLive]=useState<CandidateSnapshot|null>(null);
 const [translation,setTranslation]=useState<Translation|null>(null);
 const [metric,setMetric]=useState<Metrics>({snapshots:0,translations:0,cache_hits:0,errors:0,last_reader_status:'等待连接',last_translation_ms:0});
 const [historyRows,setHistoryRows]=useState<HistoryRow[]>([]);
 const [historyView,setHistoryView]=useState<HistoryView>('cards');
 const [historyQuery,setHistoryQuery]=useState('');
 const [pageSize,setPageSize]=useState(20);
 const [pageIndex,setPageIndex]=useState(0);
 const [confirmClear,setConfirmClear]=useState(false);
 const [vocabulary,setVocabulary]=useState<VocabRow[]>([]);
 const [dueOnly,setDueOnly]=useState(false);
 const [models,setModels]=useState<ModelRecord[]>([]);
 const [modelError,setModelError]=useState('');
 const [providerSelected,setProviderSelected]=useState('ollama');
 const [secret,setSecret]=useState('');
 const [hasSecret,setHasSecret]=useState(false);
 const [manualText,setManualText]=useState('');
 const [manualResult,setManualResult]=useState<Translation|null>(null);
 const [busy,setBusy]=useState(false);
 const [report,setReport]=useState('');
 const [closeAsk,setCloseAsk]=useState(false);
 const [recOpen,setRecOpen]=useState(false);
 const [starredKeys,setStarredKeys]=useState<Set<string>>(()=>new Set());
 const [booted,setBooted]=useState(false);
 const [ctx,setCtx]=useState<{x:number;y:number;items:ContextMenuItem[]}|null>(null);
 const toastHost=useRef<HTMLElement|null>(null);
 const savedRef=useRef<AppSettings|null>(null);
 const loadedRef=useRef(false);
 const modelBefore=useRef('');

 useEffect(()=>{
  const apply=()=>setUiScale(fitUiScale());
  window.addEventListener('resize',apply);
  window.visualViewport?.addEventListener('resize',apply);
  return()=>{window.removeEventListener('resize',apply);window.visualViewport?.removeEventListener('resize',apply)};
 },[]);

 const tell=useCallback((text:string)=>{
  const el=toastHost.current as unknown as {show?:(t:string)=>void}|null;
  if(el?.show)el.show(text);
 },[]);
 const change=useCallback((patch:Partial<AppSettings>)=>setSettings(s=>s?{...s,...patch}:s),[]);
 const setTts=useCallback((patch:Partial<AppSettings>)=>{
  if(!settings)return;
  const next={...settings,...patch};
  setSettings(next);
  void emitToHud('rino:settings',next).catch(()=>{});
 },[settings]);
 const setAutoSpeech=useCallback((value:boolean)=>setTts({auto_speech:value}),[setTts]);
 const patchProvider=useCallback((id:string,patch:Partial<ProviderProfile>)=>setSettings(s=>s?{...s,providers:s.providers.map(p=>p.id===id?{...p,...patch}:p)}:s),[]);
 const selectedProvider=settings?.providers.find(p=>p.id===providerSelected);
 const refreshHistory=useCallback(async()=>{if(DEMO_MODE){setHistoryRows(DEMO_HISTORY);return}try{setHistoryRows(await readHistory(800))}catch{/* preview */}},[]);
 const refreshVocab=useCallback(async()=>{if(DEMO_MODE){setVocabulary(DEMO_VOCAB);return}try{setVocabulary(await readVocab(dueOnly))}catch(e){tell(String(e))}},[dueOnly,tell]);
 const refreshStars=useCallback(async()=>{if(DEMO_MODE){setStarredKeys(new Set(DEMO_VOCAB.map(v=>v.source+'|'+v.target_lang)));return}try{const rows=await readVocab(false);setStarredKeys(new Set(rows.map(v=>v.source+'|'+v.target_lang)))}catch{/* preview */}},[]);

 const copyText=useCallback(async(text:string,msg:string)=>{
  try{await navigator.clipboard.writeText(text);tell(msg);return}catch{/* fall through */}
  try{const ta=document.createElement('textarea');ta.value=text;ta.style.position='fixed';ta.style.opacity='0';document.body.appendChild(ta);ta.select();document.execCommand('copy');ta.remove();tell(msg)}catch{tell('复制失败')}
 },[tell]);
 const refreshStatus=useCallback(()=>{void readMetrics().then(setMetric).catch(()=>{});if(page==='history')void refreshHistory();if(page==='vocab')void refreshVocab()},[page,refreshHistory,refreshVocab]);
 const removeHistoryRow=useCallback(async(h:HistoryRow)=>{try{await deleteHistory(h.id);await refreshHistory()}catch(e){tell(String(e))}},[refreshHistory,tell]);
 const reviewVocabRow=useCallback(async(v:VocabRow)=>{try{await call('review_vocab',{text:v.source,target:v.target_lang,quality:4});await refreshVocab()}catch(e){tell(String(e))}},[refreshVocab,tell]);
 const dropVocabRow=useCallback(async(v:VocabRow)=>{try{await call('remove_vocab',{text:v.source,target:v.target_lang});await refreshVocab();await refreshStars()}catch(e){tell(String(e))}},[refreshVocab,refreshStars,tell]);
 useEffect(()=>{
  let alive=true;const unsub:Array<()=>void>=[];
  const boot=async()=>{
   const cfg=await readSettings();
   if(!alive) return;
   savedRef.current=structuredClone(cfg);loadedRef.current=true;
   setSettings(cfg);setBooted(true);
   setProviderSelected(cfg.provider_id==='auto'||cfg.provider_id==='offline'?'ollama':cfg.provider_id);
   if(!cfg.start_minimized) requestAnimationFrame(()=>requestAnimationFrame(()=>void showMain()));
   for(const [name,handler] of [
    ['rino:candidate',(v:unknown)=>{setLive(v as CandidateSnapshot);setTranslation(null)}],
    ['rino:translation',(v:unknown)=>setTranslation(v as Translation)],
    ['rino:hide',()=>{setLive(null);setTranslation(null)}],
    ['rino:settings',(v:unknown)=>{const next=v as AppSettings;savedRef.current=structuredClone(next);setSettings(next)}],
    ['rino:navigate',(v:unknown)=>{setPage((v as string)==='settings'?'settings':'home');if((v as string)==='settings')setSettingsTab('basic')}],
    ['rino:close-request',()=>setCloseAsk(true)]
   ] as Array<[string,(v:unknown)=>void]>){unsub.push(await listenEvent(name,handler));}
   if(inTauri()){try{const c=await call<CandidateSnapshot|null>('current_candidate');if(alive)setLive(c)}catch{/* nothing */}}
   void refreshHistory();void refreshStars();
  };
  boot().catch(e=>tell(String(e)));
  const timer=setInterval(()=>readMetrics().then(v=>alive&&setMetric(v)).catch(()=>{}),1500);
  return()=>{alive=false;clearInterval(timer);unsub.forEach(fn=>fn())};
 },[tell,refreshHistory,refreshStars]);

 // Theme + dark mode + density live on <html> so every panel inherits them.
 useEffect(()=>{
  if(!settings)return;
  const t=THEMES.find(x=>x.id===settings.theme)??THEMES[0];
  document.documentElement.style.setProperty('--accent',settings.dark?t.darkAccent:t.accent);
  document.documentElement.style.setProperty('--accent-deep',settings.dark?t.darkDeep:t.deep);
  document.documentElement.dataset.mode=settings.dark?'dark':'light';
  document.documentElement.dataset.density=settings.density;
 },[settings?.theme,settings?.density,settings?.dark]);

 // Instant save: no confirmation, no save bar. Debounced so typing stays fluent.
 useEffect(()=>{
  if(!settings||!loadedRef.current)return;
  if(savedRef.current&&JSON.stringify(settings)===JSON.stringify(savedRef.current))return;
  const timer=setTimeout(async()=>{
   try{await saveSettings(settings);savedRef.current=structuredClone(settings)}catch(e){tell(String(e))}
  },650);
  return()=>clearTimeout(timer);
 },[settings,tell]);

 useEffect(()=>{if(page==='history')void refreshHistory()},[page,refreshHistory]);
 useEffect(()=>{if(page==='vocab')void refreshVocab()},[page,refreshVocab]);
 useEffect(()=>{if(translation)void refreshHistory()},[translation,refreshHistory]);
 useEffect(()=>{setPageIndex(0)},[historyQuery,pageSize,historyView]);
 useEffect(()=>{
  if(!selectedProvider){setHasSecret(false);return;}
  setSecret('');
  if(inTauri())void call<boolean>('has_api_key',{profileId:selectedProvider.id}).then(setHasSecret).catch(()=>setHasSecret(false));
 },[selectedProvider?.id]);

 // API key is stored as soon as the field settles.
 useEffect(()=>{
  if(!loadedRef.current||!selectedProvider)return;
  const key=secret.trim();if(!key)return;
  const timer=setTimeout(async()=>{
   try{await call('set_api_key',{profileId:selectedProvider.id,key});setHasSecret(true);tell('密钥已保存')}catch(e){tell(String(e))}
  },800);
  return()=>clearTimeout(timer);
 },[secret,selectedProvider?.id,hasSecret,loadedRef.current]);

 const toggleMonitor=useCallback(()=>{if(!settings)return;change({enabled:!settings.enabled});},[settings,change]);
 const testTranslate=useCallback(async()=>{if(!manualText.trim())return;setBusy(true);setManualResult(null);try{setManualResult(await requestTranslate(manualText))}catch(e){tell(String(e))}finally{setBusy(false)}},[manualText,tell]);
 const toggleStar=useCallback(async(row:HistoryRow)=>{const key=row.original+'|'+row.target_lang;const on=starredKeys.has(key);try{
  if(on){await call('remove_vocab',{text:row.original,target:row.target_lang});setStarredKeys(prev=>{const n=new Set(prev);n.delete(key);return n})}
  else{await call('save_vocab',{text:row.original,translation:row.translated,target:row.target_lang});setStarredKeys(prev=>new Set(prev).add(key))}
 }catch(e){tell(String(e))}},[starredKeys,tell]);
 const saveTranslation=useCallback(async(t:Translation)=>{const key=t.original+'|'+t.target;try{await call('save_vocab',{text:t.original,translation:t.translated,target:t.target});setStarredKeys(prev=>new Set(prev).add(key))}catch(e){tell(String(e))}},[tell]);
 const applyClose=useCallback(async(action:'tray'|'quit',remember:boolean)=>{setCloseAsk(false);try{await confirmClose(action,remember);if(remember&&settings){const next={...settings,close_prompt:false,close_action:action};savedRef.current=structuredClone(next);setSettings(next)}}catch(e){tell(String(e))}},[settings,tell]);
 const loadModels=useCallback(async()=>{setModelError('');try{const list=await readModels();setModels(list)}catch(e){setModelError(String(e))}},[tell]);
 useEffect(()=>{if(page==='settings'&&settingsTab==='models'&&models.length===0&&inTauri()){readModels().then(setModels).catch(e=>setModelError(String(e)))}},[page,settingsTab,models.length]);

 const openContextMenu=(e:React.MouseEvent<HTMLDivElement>)=>{
  e.preventDefault();e.stopPropagation();
  if(!settings)return;
  const hit=e.target as HTMLElement;
  const historyEl=hit.closest<HTMLElement>('[data-history-id]');
  const vocabEl=hit.closest<HTMLElement>('[data-vocab-key]');
  const h=historyEl?historyRows.find(r=>String(r.id)===historyEl.dataset.historyId):undefined;
  const v=vocabEl?vocabulary.find(r=>r.source+'|'+r.target_lang===vocabEl.dataset.vocabKey):undefined;
  const items:ContextMenuItem[]=[];
  if(h){
   items.push(
    {label:'复制原文',icon:ClipboardCopy,onClick:()=>void copyText(h.original,'原文已复制')},
    {label:'复制译文',icon:ClipboardCopy,onClick:()=>void copyText(h.translated,'译文已复制')},
    {label:'朗读译文',icon:Volume2,onClick:()=>void speak(h.translated,h.target_lang,settings)},
    {label:starredKeys.has(h.original+'|'+h.target_lang)?'取消收藏':'收藏到生词本',icon:Star,onClick:()=>void toggleStar(h)},
    {label:'删除这条记录',icon:Trash2,danger:true,onClick:()=>void removeHistoryRow(h)},
    {divider:true}
   );
  }else if(v){
   items.push(
    {label:'复制词汇',icon:ClipboardCopy,onClick:()=>void copyText(v.source,'词汇已复制')},
    {label:'复制译文',icon:ClipboardCopy,onClick:()=>void copyText(v.translated,'译文已复制')},
    {label:'朗读译文',icon:Volume2,onClick:()=>void speak(v.translated,v.target_lang,settings)},
    {label:'标记已掌握',icon:Check,onClick:()=>void reviewVocabRow(v)},
    {label:'移出生词本',icon:Trash2,danger:true,onClick:()=>void dropVocabRow(v)},
    {divider:true}
   );
  }else{
   if(translation){
    const key=translation.original+'|'+translation.target;
    items.push(
     {label:'复制原文',icon:ClipboardCopy,onClick:()=>void copyText(translation.original,'原文已复制')},
     {label:'复制译文',icon:ClipboardCopy,onClick:()=>void copyText(translation.translated,'译文已复制')},
     {label:'朗读译文',icon:Volume2,onClick:()=>void speak(translation.translated,translation.target,settings)},
     starredKeys.has(key)?{label:'已收藏到生词本',icon:Star,checked:true,disabled:true,onClick:()=>{}}
      :{label:'收藏当前译文',icon:Star,onClick:()=>void saveTranslation(translation),
        },
     {divider:true}
    );
   }else if(live){
    items.push(
     {label:'复制当前输入',icon:ClipboardCopy,onClick:()=>void copyText(live.text,'输入已复制')},
     {divider:true}
    );
   }
  }
  const common:ContextMenuItem[]=[
   {label:'刷新状态',icon:RefreshCw,onClick:refreshStatus},
   {label:settings.enabled?'暂停监测':'启动监测',icon:settings.enabled?Pause:Play,onClick:toggleMonitor},
   {label:'自动朗读译文',icon:Volume2,checked:settings.auto_speech,onClick:()=>setAutoSpeech(!settings.auto_speech)},
   {label:'夜间模式',icon:settings.dark?Sun:Moon,checked:settings.dark,onClick:()=>change({dark:!settings.dark})},
  ];
  items.push(...((h||v)?common:common.concat([
   {label:'打开设置',icon:Settings2,onClick:()=>{setPage('settings');setSettingsTab('basic')}},
   {label:'关于 RINO',icon:Info,onClick:()=>setPage('about')},
   {label:'退出 RINO',icon:Power,danger:true,onClick:()=>void confirmClose('quit',false).catch(err=>tell(String(err)))}
  ])));
  const menuW=236*uiScale;
  const estH=Math.min(420,items.reduce((n,it)=>n+(it.divider?9:34),18))*uiScale;
  const x=Math.max(8,Math.min(e.clientX,window.innerWidth-menuW-8));
  const y=Math.max(8,Math.min(e.clientY,window.innerHeight-estH-8));
  setCtx({x,y,items});
 };
 if(!settings) return <div className="boot"><Logo size={42}/><span>正在启动…</span></div>;
 if(!booted) return <div className="boot"><Logo size={42}/><span>正在启动…</span></div>;

 const filteredHistory=historyQuery.trim()?historyRows.filter(h=>h.original.toLowerCase().includes(historyQuery.trim().toLowerCase())||h.translated.toLowerCase().includes(historyQuery.trim().toLowerCase())):historyRows;
 const pageCount=Math.max(1,Math.ceil(filteredHistory.length/pageSize));
 const safeIndex=Math.min(pageIndex,pageCount-1);
 const pagedHistory=filteredHistory.slice(safeIndex*pageSize,safeIndex*pageSize+pageSize);
 const closeMode=settings.close_prompt?'ask':(settings.close_action==='quit'?'quit':'tray');
 const fixedScroll=page==='history'||page==='vocab';

 return <div className="app" onContextMenu={openContextMenu} style={{width:`calc(100vw / ${uiScale})`,height:`calc(100vh / ${uiScale})`,transform:`scale(${uiScale})`,transformOrigin:"top left"}}>
  <aside className="rail">
   <div className="brand"><Logo size={32}/><div className="brand-copy"><strong>rino<span>.</span></strong><div className="brand-sub"><small>随字而译</small><em>v0.1.0</em></div></div></div>
   <nav className="nav">{NAV.map(([id,label,Icon])=><button key={id} className={page===id?'nav-item on':'nav-item'} onClick={()=>setPage(id)}>{page===id&&<motion.span className="nav-bg" layoutId="nav-bg" transition={{type:'spring',stiffness:430,damping:38}}/>}<Icon size={16}/><span>{label}</span>{page===id&&<ChevronRight size={13} className="nav-arrow"/>}</button>)}</nav>
   <div className="rail-spacer"/>
   <div className="rail-status">
    <div className="rail-status-top">
     <div className="rail-state"><span className={settings.enabled?'dot on':'dot'}/><div><strong>{settings.enabled?'正在伴随输入':'已暂停'}</strong><small>{dir(settings.source,settings.target)}</small></div></div>
     <div className="rail-tools">
      <IconBtn icon={RefreshCw} title="刷新状态" onClick={()=>readMetrics().then(setMetric).catch(()=>{})}/>
      <IconBtn icon={settings.enabled?Pause:Play} title={settings.enabled?'暂停监测':'启动监测'} onClick={()=>void toggleMonitor()} className={settings.enabled?'':'accent'}/>
     </div>
    </div>
   </div>
  </aside>

  <div className="stage">
   <header className="topbar">
    <div className="crumbs"><span>RINO</span><ChevronRight size={11}/><strong>{NAV.find(v=>v[0]===page)?.[1]}</strong></div>
    <div className="top-actions">
     <IconBtn icon={settings.dark?Sun:Moon} title={settings.dark?'切换到浅色':'切换到夜间模式'} onClick={()=>change({dark:!settings.dark})}/>
    </div>
   </header>

   <div className={'scroll'+(fixedScroll?' scroll-fixed':'')}>
    <AnimatePresence mode="wait">
    <motion.div key={page} className={'page page-'+page+(fixedScroll?' page-fill':'')} initial={{opacity:0,y:8}} animate={{opacity:1,y:0}} exit={{opacity:0,y:-5}} transition={{duration:.17,ease:[.22,1,.36,1]}}>

    {page==='home'&&<>
     <PageHead title="随字而译" sub="输入之间，译文轻盈呈现，让语言学习自然融入每一次表达。" right={<span className={settings.enabled?'status-pill on':'status-pill'}><i/>{settings.enabled?'监听中':'已暂停'}</span>}/>
     <div className="metrics">
      <Stat icon={Activity} value={metric.snapshots} label="候选识别次数"/>
      <Stat icon={Globe2} value={metric.translations} label="完成翻译次数"/>
      <Stat icon={Gauge} value={metric.cache_hits} label="缓存命中次数"/>
      <Stat icon={Clock} value={metric.last_translation_ms+'ms'} label="最近一次翻译耗时"/>
     </div>
     <div className="home-stack">
      <Panel className="live-panel">
       <div className="live-head"><span className="lang-pill"><Languages size={12}/>{dir(settings.source,settings.target)}</span><span className="src-tag">{live?.source??'UIA'}</span></div>
       <div className="live-stage">{live
        ?<>
         <div className="live-word">{live.text}</div>
         <div className="live-result">{translation?.original===live.text?translation.translated:'正在翻译…'}</div>
         <div className="live-choices">{live.candidates.slice(0,6).map((c,i)=><span key={i} className={i===live.selected_index?'choice on':'choice'}>{c.text}</span>)}</div>
         <div className="live-foot"><span className="muted">{translation&&translation.original===live.text?`${translation.engine} · ${translation.elapsed_ms} ms`:'读取候选'}</span>{translation&&translation.original===live.text&&<StarButton on={starredKeys.has(translation.original+'|'+translation.target)} onClick={()=>void saveTranslation(translation)}/>}</div>
        </>
        :<Empty icon={Sparkles} title="等待一次输入" text="在浏览器、编辑器或聊天框里使用微软拼音，候选与译文会同时出现在这里。"/>}
       </div>
      </Panel>
      <Panel title="最近输入" right={<button className="link-btn" onClick={()=>setPage('history')}>全部 <ArrowRight size={12}/></button>}>
       {historyRows.length?<div className="recent">{historyRows.slice(0,5).map(h=><div className="recent-row" key={h.id} data-history-id={h.id}><div className="recent-words"><strong>{h.original}</strong><ArrowRight size={11}/><em>{h.translated}</em></div><small>{clock(h.created_at)}</small></div>)}</div>:<Empty icon={History} title="还没有记录" text="开始输入后，这里会出现最近的字词与译文。"/>}
      </Panel>
     </div>

    </>}

    {page==='history'&&<>
     <PageHead title="输入记录" sub="本机留存，按时间排序。" right={<IconButtons items={[{id:'cards' as HistoryView,icon:LayoutGrid,title:'卡片视图'},{id:'table' as HistoryView,icon:Table2,title:'表格视图'}]} value={historyView} onChange={setHistoryView}/>}/>
     <div className="history-bar">
      <select className="ctl mini" value={pageSize} onChange={e=>setPageSize(Number(e.target.value))} title="每页条数"><option value={20}>20</option><option value={50}>50</option><option value={100}>100</option></select>
      <label className="search"><Search size={13}/><input value={historyQuery} onChange={e=>setHistoryQuery(e.target.value)} placeholder="搜索原文或译文"/></label>
      <span className="muted count">{filteredHistory.length} 条</span>
      <IconBtn icon={Trash2} title={confirmClear?'再次点击确认清空':'清空记录'} className={confirmClear?'danger on':''} onClick={async()=>{if(!confirmClear){setConfirmClear(true);setTimeout(()=>setConfirmClear(false),3000);return;}try{await clearHistory();setConfirmClear(false);await refreshHistory();}catch(e){tell(String(e))}}}/>
     </div>
     <div className="history-body">
      {filteredHistory.length===0?<Panel className="fill-center"><Empty icon={History} title="没有匹配的记录" text="换一个关键词，或先去输入几个词。"/></Panel>
       :historyView==='cards'
       ?<div className="cards">{pagedHistory.map((h,i)=><motion.div key={h.id} className="hcard" data-history-id={h.id} initial={{opacity:0,y:6}} animate={{opacity:1,y:0}} transition={{delay:Math.min(i,8)*.025,duration:.18}} whileHover={{y:-2}}>
         <div className="hcard-top"><span>{dir(h.source_lang,h.target_lang)}</span><small>{clock(h.created_at)}</small></div>
         <div className="hcard-body"><strong>{h.original}</strong><ArrowRight size={13}/><em>{h.translated}</em></div>
         <div className="hcard-foot"><span>{h.engine} · {h.elapsed_ms} ms</span><div className="row-actions"><StarButton on={starredKeys.has(h.original+'|'+h.target_lang)} onClick={()=>void toggleStar(h)}/><IconBtn icon={Volume2} title="朗读" onClick={()=>void speak(h.translated,h.target_lang,settings)}/><IconBtn icon={Trash2} title="删除这条记录" onClick={()=>void removeHistoryRow(h)}/></div></div>
        </motion.div>)}</div>
       :<div className="table-wrap"><table><thead><tr><th>时间</th><th>原文</th><th>译文</th><th>引擎</th><th>耗时</th><th className="act">操作</th></tr></thead><tbody>{pagedHistory.map(h=><tr key={h.id} data-history-id={h.id}><td className="muted">{clock(h.created_at)}</td><td><strong>{h.original}</strong></td><td>{h.translated}</td><td className="muted">{h.engine}</td><td className="muted">{h.elapsed_ms} ms</td><td><div className="row-actions"><StarButton on={starredKeys.has(h.original+'|'+h.target_lang)} onClick={()=>void toggleStar(h)}/><IconBtn icon={Volume2} title="朗读" onClick={()=>void speak(h.translated,h.target_lang,settings)}/><IconBtn icon={Trash2} title="删除这条记录" onClick={()=>void removeHistoryRow(h)}/></div></td></tr>)}</tbody></table></div>}
     </div>
     <div className="pager"><span className="muted">共 {filteredHistory.length} 条</span><div className="pager-buttons"><IconBtn icon={ChevronLeft} title="上一页" onClick={()=>setPageIndex(Math.max(0,safeIndex-1))}/><span>第 {safeIndex+1} / {pageCount} 页</span><IconBtn icon={ChevronRight} title="下一页" onClick={()=>setPageIndex(Math.min(pageCount-1,safeIndex+1))}/></div></div>
    </>}

    {page==='vocab'&&<>
     <PageHead title="生词本" sub="收藏词语，科学复习。" right={<div className="head-tools"><Segmented items={[{id:'all',name:'全部'},{id:'due',name:'待复习'}]} value={dueOnly?'due':'all'} onChange={v=>setDueOnly(v==='due')}/><IconBtn icon={Download} title="导出 CSV" onClick={async()=>{try{const csv=await call<string>('export_vocab');const blob=new Blob([csv],{type:'text/csv;charset=utf-8'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download='rino-vocabulary.csv';a.click();URL.revokeObjectURL(url);}catch(e){tell(String(e))}}}/></div>}/>
     <div className="vocab-body">{vocabulary.length
      ?<div className="table-wrap vocab-table"><table><thead><tr><th>词汇</th><th>译文</th><th>复习</th><th>下次</th><th className="act">操作</th></tr></thead><tbody>{vocabulary.map(v=><tr key={v.source+v.target_lang} data-vocab-key={v.source+'|'+v.target_lang}><td><strong>{v.source}</strong><small className="muted"> {SHORT[v.target_lang]??v.target_lang}</small></td><td>{v.translated}</td><td className="muted">{v.repetitions} 次 · {v.interval_days} 天</td><td className="muted">{new Date(v.due_at).toLocaleDateString('zh-CN')}</td><td><div className="row-actions"><IconBtn icon={Volume2} title="朗读" onClick={()=>void speak(v.translated,v.target_lang,settings)}/><IconBtn icon={Check} title="标记已掌握" onClick={()=>void reviewVocabRow(v)}/><IconBtn icon={Trash2} title="移出生词本" onClick={()=>void dropVocabRow(v)}/></div></td></tr>)}</tbody></table></div>
      :<Panel className="fill-center"><Empty icon={BookMarked} title="还没有收藏" text="在输入记录里点一下星标，词语就会出现在这里。"/></Panel>}</div>
    </>}

    {page==='settings'&&<>
     <PageHead title="设置" sub="随心调整，自动保存。"/><div className="settings-tabs"><Segmented items={[{id:'basic' as SettingsTab,name:'基本设置'},{id:'models' as SettingsTab,name:'模型'},{id:'diagnostics' as SettingsTab,name:'诊断'}]} value={settingsTab} onChange={setSettingsTab}/></div>
     <AnimatePresence mode="wait">
      <motion.div key={settingsTab} className="settings-pane" initial={{opacity:0,y:7}} animate={{opacity:1,y:0}} exit={{opacity:0,y:-5}} transition={{duration:.15,ease:[.22,1,.36,1]}}>
      {settingsTab==='basic'&&<>
       <Panel title="语言">
        <Row label="目标语言"><select className="ctl" value={settings.target} onChange={e=>{const v=e.target.value;if(v!==settings.target){change({target:v});tell('目标语言已切换为 '+LANGUAGES[v])}}}>{Object.entries(LANGUAGES).filter(([k])=>k!==settings.source).map(([k,v])=><option key={k} value={k}>{v}</option>)}</select></Row>
       </Panel>
       <Panel title="浮窗">
        <Row label="字号"><select className="ctl" value={settings.font_size} onChange={e=>change({font_size:Number(e.target.value)})}>{[13,14,15,16,17,18,20,22,24].map(v=><option key={v} value={v}>{v} px</option>)}</select></Row>
        <Row label="与候选栏的距离" hint="0–50 px"><input className="ctl num" type="number" min={0} max={50} value={settings.hud_gap} onChange={e=>change({hud_gap:Number(e.target.value)})}/></Row>
        <div className="field-note">浮窗显示译文时，按 Tab 可直接填入译文，不再上屏中文候选。</div>

       </Panel>
       <Panel title="语音">
        <SwitchRow title="自动朗读译文" desc="译文出现时朗读，可随时关闭" checked={settings.auto_speech} onChange={v=>setAutoSpeech(v)}/>
        <Row label="发音引擎">
         <select className="ctl" value={settings.tts_provider} onChange={e=>{const v=e.target.value;setTts({tts_provider:v,tts_voice:v==='edge'?edgeVoiceFor(settings.target,settings.tts_voice):settings.tts_voice});if(v==='edge')tell('在线音色需开启授权后生效')}}>
          <option value="system">本机语音</option>
          <option value="edge">在线神经语音</option>
         </select>
        </Row>
        {settings.tts_provider==='edge'?<>
         <Row label="音色">
          <select className="ctl voice-select" value={edgeVoiceFor(settings.target,settings.tts_voice)} onChange={e=>setTts({tts_voice:e.target.value})}>
           {EDGE_VOICES.filter(v=>v.lang===settings.target).map(v=><option key={v.id} value={v.id}>{v.label}</option>)}
          </select>
         </Row>
         {EDGE_VOICES.some(v=>v.lang===settings.target)
          ?null
          :<div className="field-note">当前目标语言暂无在线音色，仍使用本机语音。</div>}
         <SwitchRow title="允许在线语音合成" desc="译文会发送给 Microsoft，仅用于生成语音" checked={settings.tts_cloud_consent} onChange={v=>{setTts({tts_cloud_consent:v});tell(v?'已允许在线语音合成':'已关闭在线语音合成')}}/>
         {settings.tts_cloud_consent?null:<div className="field-note">未授权时自动回退本机语音。</div>}
        </>:null}
        <Row label="试听"><button className="ghost-btn" disabled={settings.tts_provider==='edge'&&!settings.tts_cloud_consent} onClick={()=>void speak(speakSample(settings.target),settings.target,settings)}>播放示例</button></Row>
       </Panel>
       <Panel title="外观">
        <div className="swatches">{THEMES.map(t=><button key={t.id} className={settings.theme===t.id?'swatch on':'swatch'} onClick={()=>change({theme:t.id})}><span style={{background:settings.dark?t.darkAccent:t.accent}}/>{t.name}</button>)}</div>
        <Row label="界面密度"><Segmented items={DENSITIES.map(d=>({id:d.id,name:d.name}))} value={settings.density} onChange={v=>change({density:v})}/></Row>
       </Panel>
       <Panel title="行为">
        <SwitchRow title="启动时隐藏主窗口" desc="只留托盘图标" checked={settings.start_minimized} onChange={v=>change({start_minimized:v})} noLine/>
        <Row label="关闭主窗口" hint="下次不再询问可在弹窗里设置" noLine>
         <select className="ctl" value={closeMode} onChange={e=>{const v=e.target.value;if(v==='ask')change({close_prompt:true});else change({close_prompt:false,close_action:v})}}>
          <option value="ask">每次询问</option><option value="tray">最小化到托盘</option><option value="quit">直接退出</option>
         </select></Row>
        <SwitchRow title="允许在线翻译" desc="关闭时只用本地词典与本地模型" checked={settings.cloud_consent} onChange={v=>change({cloud_consent:v})}/>
       </Panel>
       <Panel title="监测节奏">
        <Row label="活跃轮询" hint="30–2000 ms"><input className="ctl num" type="number" min={30} max={2000} value={settings.poll_active_ms} onChange={e=>change({poll_active_ms:Number(e.target.value)})}/></Row>
        <Row label="空闲轮询" hint="80–5000 ms"><input className="ctl num" type="number" min={80} max={5000} value={settings.poll_idle_ms} onChange={e=>change({poll_idle_ms:Number(e.target.value)})}/></Row>
       </Panel>
      </>}

      {settingsTab==='models'&&<>
       <Panel title="服务商" note="密钥保存在 Windows 凭据管理器。">
        <div className="fields">
         <div className="field"><label>翻译策略</label><select className="ctl wide" value={settings.provider_id} onChange={e=>{const id=e.target.value;change({provider_id:id,providers:settings.providers.map(p=>p.id===id?{...p,enabled:true}:p)});const label=id==='auto'?'智能路由':id==='offline'?'纯离线词典':(settings.providers.find(p=>p.id===id)?.label??id);tell('翻译策略已切换为 '+label)}}>
          <option value="auto">智能（按延迟自动选择）</option><option value="offline">纯离线词典</option>
          {settings.providers.map(p=><option key={p.id} value={p.id}>{p.label}</option>)}
         </select></div>
         <div className="field"><label>服务商</label><select className="ctl wide" value={providerSelected} onChange={e=>setProviderSelected(e.target.value)}>{settings.providers.map(p=><option key={p.id} value={p.id}>{p.label}</option>)}</select></div>
         {selectedProvider&&<>
          <div className="field"><label>调用协议</label><select className="ctl wide" value={selectedProvider.protocol} onChange={e=>patchProvider(selectedProvider.id,{protocol:e.target.value})}><option value="ollama">Ollama Chat</option><option value="openai">OpenAI 兼容</option><option value="responses">OpenAI Responses</option><option value="anthropic">Anthropic Messages</option><option value="gemini">Gemini</option><option value="deepl">DeepL</option><option value="libre">LibreTranslate</option><option value="baidu">百度翻译</option></select></div>
          <div className="field"><label>Base URL</label><input className="ctl wide" spellCheck={false} value={selectedProvider.base_url} onChange={e=>patchProvider(selectedProvider.id,{base_url:e.target.value})} placeholder="https://api.example.com/v1"/></div>
          {!['deepl','baidu','libre'].includes(selectedProvider.protocol)&&<div className="field"><label>模型 ID</label><input className="ctl wide" spellCheck={false} value={selectedProvider.model} onFocus={e=>{modelBefore.current=e.target.value}} onBlur={e=>{if(modelBefore.current!==e.target.value)tell('模型已更新')}} onChange={e=>patchProvider(selectedProvider.id,{model:e.target.value})} placeholder="完整模型 ID"/></div>}
          {selectedProvider.protocol==='baidu'&&<div className="field"><label>APPID</label><input className="ctl wide" spellCheck={false} value={selectedProvider.app_id??''} onChange={e=>patchProvider(selectedProvider.id,{app_id:e.target.value})} placeholder="百度翻译应用 APPID"/></div>}
          <div className="field"><label>API Key</label><input className="ctl wide" type="password" autoComplete="off" value={secret} onChange={e=>setSecret(e.target.value)} placeholder={hasSecret?'已保存，输入新密钥可覆盖':'本地模型无需填写'}/><small className="field-note">{hasSecret?'已保存':'输入后自动保存'}</small></div>
         </>}
        </div>
       </Panel>
       <Panel title="本地模型" right={<IconBtn icon={RefreshCw} title="扫描本地模型" onClick={()=>void loadModels()}/>}>
        {models.length?<div className="model-cards">{models.map(m=><button key={m.name} className="model-card" onClick={()=>{patchProvider('ollama',{model:m.name});setProviderSelected('ollama');tell('已选择 '+m.name)}}><span className="model-badge"><Terminal size={14}/></span><strong>{m.name}</strong><small>{m.family} · {(m.size/1073741824).toFixed(2)} GiB</small><ChevronRight size={13}/></button>)}</div>
        :<div className="model-empty">{modelError||'还没有扫描结果。'}</div>}
       </Panel>
       <Panel title="推荐模型" note="适合本地部署的翻译模型">
        <button className="rec-entry" onClick={()=>setRecOpen(true)}><span className="rec-icon"><Sparkles size={16}/></span><div><strong>查看模型建议</strong><small>下载方式、体积与适用场景</small></div><ChevronRight size={14}/></button>
       </Panel>
      </>}

      {settingsTab==='diagnostics'&&<>
       <div className="metrics">
        <Stat icon={Activity} value={metric.snapshots} label="候选采样"/>
        <Stat icon={Globe2} value={metric.translations} label="翻译成功"/>
        <Stat icon={Gauge} value={metric.cache_hits} label="缓存命中"/>
        <Stat icon={Clock} value={metric.errors} label="失败次数" tone={metric.errors?'warn':''}/>
       </div>
       <Panel title="连通性测试" right={<span className="lang-mini">{dir(settings.source,settings.target)}</span>}>
         <div className="test-grid">
          <label className="test-label">原文</label><span/>
          <label className="test-label">译文</label>
          <input className="ctl test-input" maxLength={500} value={manualText} onChange={e=>setManualText(e.target.value)} placeholder="输入一句话测试翻译"/>
          <button className="send-btn" title={busy?'翻译中':'开始翻译'} disabled={busy} onClick={()=>void testTranslate()}>{busy?<Loader2 size={16} className="spin"/>:<ArrowRight size={16}/>}</button>
          <div className="test-result-box" title={manualResult?.translated}>{manualResult?<><strong>{manualResult.translated}</strong><small>{manualResult.engine} · {manualResult.elapsed_ms} ms</small></>:<span className="muted">等待测试</span>}</div>
         </div>
       </Panel>
       <Panel title="输入法读取">
        <div className="reader-card"><div className="reader-radar"><Radio size={20}/><span/><span/></div><div className="reader-copy"><strong>{metric.last_reader_status}</strong><small>{inTauri()?'Windows UI Automation':'浏览器预览'} · 活跃 {settings.poll_active_ms} ms · 空闲 {settings.poll_idle_ms} ms</small></div><span className={settings.enabled?'chip on':'chip'}>{settings.enabled?'运行中':'已暂停'}</span></div>
       </Panel>
       <Panel title="诊断报告" note="脱敏，不含候选与密钥。" right={<div className="head-tools"><IconBtn icon={FileText} title="生成诊断报告" onClick={async()=>{try{setReport(await call<string>('diagnostic_report'));}catch(e){tell(String(e))}}}/>{report&&<IconBtn icon={ClipboardCopy} title="复制报告" onClick={()=>navigator.clipboard.writeText(report).then(()=>{}).catch(()=>tell('复制失败'))}/>}</div>}>
        {report?<pre className="report">{report}</pre>:<div className="model-empty">还没有生成报告。</div>}
       </Panel>
      </>}
      </motion.div>
     </AnimatePresence>
    </>}

    {page==='about'&&<>
     <div className="about-wrap">
      <motion.div className="about-hero-card" initial={{opacity:0,y:14,scale:.98}} animate={{opacity:1,y:0,scale:1}} transition={{duration:.45,ease:[.22,1,.36,1]}}>
       <AboutParticles/><div className="about-aurora a"/><div className="about-aurora b"/><div className="about-gridlines"/>
       <div className="about-orbit"><span/><span/></div><div className="about-glyph" aria-hidden="true">译</div>
       <motion.span className="about-spark s1" animate={{y:[0,-7,0],opacity:[.35,.9,.35]}} transition={{duration:4.4,repeat:Infinity,ease:'easeInOut'}}><Sparkles size={14}/></motion.span>
       <motion.span className="about-spark s2" animate={{y:[0,6,0],opacity:[.3,.8,.3]}} transition={{duration:5.2,repeat:Infinity,ease:'easeInOut',delay:.8}}><Star size={12}/></motion.span>
       <div className="about-hero">
        <div className="about-logo"><Logo size={48}/></div>
        <div className="about-name"><div className="about-name-line"><h1>rino<span>.</span></h1><span className="about-version">v0.1.0</span></div><p>随字而译，输入自然流畅。</p></div>
       </div>
       <p className="about-intro">实时识别微软拼音候选，译文随浮窗呈现。<br/>不干扰输入，不抢占焦点；翻译默认本地，在线语音需单独授权。</p>
       <motion.div className="about-fx" aria-hidden="true" animate={{y:[0,-6,0]}} transition={{duration:5,repeat:Infinity,ease:"easeInOut"}}><span className="fx-key">nihao</span><span className="fx-arrow"><ArrowRight size={13}/></span><span className="fx-value">hello</span></motion.div>
       <div className="about-meta">
        <div className="meta-item"><Code2 size={15}/><strong>Jiezcode</strong></div>
        <div className="meta-item"><Mail size={15}/><strong>1716285375@qq.com</strong></div>
        <div className="meta-item"><Scale size={15}/><strong>MIT</strong></div>
        
       </div>
       <div className="about-card-foot"><span><Copyright size={11}/>2026 Jiezcode · RINO</span><span className="about-foot-mark">MADE FOR QUIET TYPING</span></div>
      </motion.div>
     </div>
    </>}
    </motion.div>
    </AnimatePresence>
   </div>

  </div>

  {ctx&&<ContextMenu x={ctx.x} y={ctx.y} items={ctx.items} scale={uiScale} onClose={()=>setCtx(null)}/>}

 {createElement('sakuga-toast',{ref:toastHost,className:'sk-toast',duration:'2.6',style:{position:'fixed',left:'calc(50% - 118px)',bottom:'28px',zIndex:80,width:236,height:56}})}

  <AnimatePresence>{recOpen&&<motion.div className="overlay" initial={{opacity:0}} animate={{opacity:1}} exit={{opacity:0}} onClick={()=>setRecOpen(false)}>
   <motion.div className="modal rec-modal" initial={{opacity:0,scale:.96,y:8}} animate={{opacity:1,scale:1,y:0}} exit={{opacity:0,scale:.97,y:6}} transition={{type:'spring',stiffness:380,damping:32}} onClick={e=>e.stopPropagation()}>
    <RecommendDialog onClose={()=>setRecOpen(false)} tell={tell}/>
   </motion.div>
  </motion.div>}</AnimatePresence>

  <AnimatePresence>{closeAsk&&<motion.div className="overlay" initial={{opacity:0}} animate={{opacity:1}} exit={{opacity:0}} onClick={()=>setCloseAsk(false)}>
   <motion.div className="modal" initial={{opacity:0,scale:.96,y:8}} animate={{opacity:1,scale:1,y:0}} exit={{opacity:0,scale:.97,y:6}} transition={{type:'spring',stiffness:380,damping:32}} onClick={e=>e.stopPropagation()}>
    <CloseDialog settings={settings} onCancel={()=>setCloseAsk(false)} onChoose={applyClose}/>
   </motion.div>
  </motion.div>}</AnimatePresence>
 </div>
}

function RecommendDialog({onClose,tell}:{onClose:()=>void;tell:(t:string)=>void}){
 const [copied,setCopied]=useState<string|null>(null);
 const copy=(command:string)=>navigator.clipboard.writeText(command).then(()=>{setCopied(command);setTimeout(()=>setCopied(c=>c===command?null:c),1400)}).catch(()=>tell('复制失败'));
 return <>
  <div className="modal-head"><div><h3>推荐模型</h3><p>本地翻译常用模型，按需选择。</p></div><IconBtn icon={X} title="关闭" onClick={onClose} className="sr-hide"/></div>
  <div className="rec-list">{MODEL_PRESETS.map(m=><div className="rec-card" key={m.name}>
   <div className="rec-card-head"><strong>{m.name}</strong><small>{m.hint}</small></div>
   <div className="rec-code-row"><code>{m.command}</code><button className={copied===m.command?'copy-icon on':'copy-icon'} title={copied===m.command?'已复制':'复制命令'} aria-label="复制命令" onClick={()=>copy(m.command)}>{copied===m.command?<Check size={14}/>:<ClipboardCopy size={14}/>}</button></div>
  </div>)}</div>
  <p className="rec-note">安装后在「服务商」里选择 Ollama，并把模型 ID 填进对应字段。</p>
  <div className="modal-actions"><button className="primary-btn" onClick={onClose}>知道了</button></div>
 </>
}
function CloseDialog({settings,onCancel,onChoose}:{settings:AppSettings;onCancel:()=>void;onChoose:(a:'tray'|'quit',remember:boolean)=>void}){
 const [remember,setRemember]=useState(!settings.close_prompt);
 return <>
  <h3>要关闭 RINO 吗？</h3>
  <p>最小化后仍会在托盘里继续识别；退出会停止监测。</p>
  <label className="check"><input type="checkbox" checked={remember} onChange={e=>setRemember(e.target.checked)}/><span>记住我的选择，下次不再提示</span></label>
  <div className="modal-actions close-actions"><button className="ghost-btn" onClick={()=>onChoose('tray',remember)}>最小化到托盘</button><button className="danger-btn" onClick={()=>onChoose('quit',remember)}>退出程序</button><button className="primary-btn" onClick={onCancel}>取消</button></div>
 </>
}

function Boot(){const isHud=new URLSearchParams(window.location.search).get('window')==='hud';return isHud?<HudView/>:<Application/>}
createRoot(document.getElementById('root')!).render(<Boot/>);
