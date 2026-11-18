import type { AppSettings,CandidateSnapshot,HistoryRow,Metrics,ModelRecord,Translation,VocabRow } from './types';
import { DEFAULTS } from './types';

export function inTauri():boolean {if(typeof window==='undefined')return false;return '__TAURI_INTERNALS__' in window;}
export async function call<T>(cmd:string,args:Record<string,unknown>={}):Promise<T>{
  if(!inTauri()) throw new Error('浏览器预览模式：启动桌面客户端以调用原生能力');
  const {invoke}=await import('@tauri-apps/api/core');return invoke<T>(cmd,args);
}
export async function readSettings(){return inTauri()?call<AppSettings>('get_settings'):Promise.resolve({...DEFAULTS,dark:new URLSearchParams(window.location.search).get('dark')==='1',providers:[{id:'ollama',label:'Ollama (本地)',protocol:'ollama',base_url:'http://127.0.0.1:11434',model:'translategemma:4b',enabled:true}]});}
export async function saveSettings(settings:AppSettings){if(!inTauri())return Promise.resolve();return call<void>('save_settings',{settings});}
export async function getSnapshot(){return call<CandidateSnapshot|null>('current_candidate');}
export async function metrics(){return inTauri()?call<Metrics>('runtime_metrics'):Promise.resolve({snapshots:0,translations:0,cache_hits:0,errors:0,last_reader_status:'浏览器预览',last_translation_ms:0});}
export async function translate(text:string){return call<Translation>('translate_text',{text});}
export async function vocab(due_only:boolean){return inTauri()?call<VocabRow[]>('list_vocab',{dueOnly:due_only}):Promise.resolve([]);}
export async function models(){return call<ModelRecord[]>('list_ollama_models');}
export async function history(limit=300){return inTauri()?call<HistoryRow[]>('list_history',{limit}):Promise.resolve([]);}
export async function deleteHistory(id:number){return call<void>('delete_history',{id});}
export async function clearHistory(){return call<void>('clear_history');}
export async function showMain(){try{return await call<void>('show_main');}catch{return;}}
export async function confirmClose(action:'tray'|'quit',remember:boolean){return call<void>('confirm_close',{action,remember});}
export async function listenEvent<T>(event:string,callback:(v:T)=>void):Promise<()=>void>{
  if(!inTauri())return ()=>{};
  const {listen}=await import('@tauri-apps/api/event');return listen<T>(event,evt=>callback(evt.payload));
}
export function speak(text:string,lang:string):void{
  if(!('speechSynthesis' in window))return;
  window.speechSynthesis.cancel();
  const utter=new SpeechSynthesisUtterance(text);
  utter.lang={en:'en-US',ja:'ja-JP',zh:'zh-CN',ko:'ko-KR',fr:'fr-FR',de:'de-DE',es:'es-ES',ru:'ru-RU' }[lang]??lang;
  utter.rate=0.92;window.speechSynthesis.speak(utter);
}