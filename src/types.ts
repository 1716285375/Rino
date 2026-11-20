export interface Rect {left:number;top:number;right:number;bottom:number}
export interface Candidate {text:string;bounds:Rect;selected:boolean}
export interface CandidateSnapshot {text:string;selected_index:number;candidates:Candidate[];bounds:Rect;window_bounds:Rect;process_id:number;source:string;revision:number}
export interface Translation {original:string;translated:string;target:string;engine:string;alternatives:string[];elapsed_ms:number}
export interface ProviderProfile {id:string;label:string;protocol:string;base_url:string;model:string;app_id?:string|null;enabled:boolean}
export interface AppSettings {source:string;target:string;provider_id:string;providers:ProviderProfile[];enabled:boolean;show_candidate_row:boolean;start_minimized:boolean;poll_active_ms:number;poll_idle_ms:number;hud_gap:number;font_size:number;auto_speech:boolean;tts_provider:string;tts_voice:string;tts_cloud_consent:boolean;cloud_consent:boolean;show_alternatives:boolean;max_options:number;enable_ime_diagnostics:boolean;close_prompt:boolean;close_action:string;theme:string;density:string;dark:boolean}
export interface VocabRow {source:string;target_lang:string;translated:string;repetitions:number;interval_days:number;due_at:string}
export interface HistoryRow {id:number;original:string;translated:string;source_lang:string;target_lang:string;engine:string;elapsed_ms:number;created_at:string}
export interface Metrics {snapshots:number;translations:number;cache_hits:number;errors:number;last_reader_status:string;last_translation_ms:number}
export interface ModelRecord {name:string;size:number;family:string}
export const LANGUAGES:Record<string,string> = {en:'英语',ja:'日语',ko:'韩语',fr:'法语',de:'德语',es:'西班牙语',ru:'俄语',it:'意大利语',pt:'葡萄牙语',ar:'阿拉伯语',th:'泰语',zh:'中文'};
export const DEFAULTS:AppSettings={source:'zh',target:'en',provider_id:'auto',providers:[],enabled:true,show_candidate_row:false,start_minimized:false,poll_active_ms:75,poll_idle_ms:240,hud_gap:9,font_size:16,auto_speech:false,tts_provider:'system',tts_voice:'',tts_cloud_consent:false,cloud_consent:false,show_alternatives:true,max_options:4,enable_ime_diagnostics:false,close_prompt:true,close_action:'tray',theme:'blossom',density:'cozy',dark:false};
export const MODEL_PRESETS=[
 {name:'Gemma 3 4B',model:'gemma3:4b',hint:'当前常用 · 本地通用模型',command:'ollama run gemma3:4b'},
 {name:'Qwen3 1.7B',model:'qwen3:1.7b',hint:'轻量通用 · 响应快',command:'ollama run qwen3:1.7b'},
 {name:'Qwen3 4B',model:'qwen3:4b',hint:'通用模型 · 质量更好',command:'ollama run qwen3:4b'},
 {name:'TranslateGemma 4B',model:'translategemma:4b',hint:'Ollama 官方模型 · 多语言翻译',command:'ollama pull translategemma:4b'},
 {name:'Index-Translate 2B',model:'hf.co/IndexTeam/Index-Translate-2B-GGUF:Q4_K_M',hint:'Index Team GGUF · 中英互译',command:'ollama run hf.co/IndexTeam/Index-Translate-2B-GGUF:Q4_K_M'},
 {name:'Hy-MT2 1.8B',model:'Hy-MT2-1.8B',hint:'腾讯混元翻译模型 · 需匹配 GGUF',command:'请使用匹配的 GGUF 与 tokenizer 模板导入'}
];