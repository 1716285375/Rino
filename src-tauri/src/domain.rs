use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect { pub left: i32, pub top: i32, pub right: i32, pub bottom: i32 }
impl Rect {
    pub fn width(&self)->i32 { (self.right-self.left).max(0) }
    pub fn height(&self)->i32 { (self.bottom-self.top).max(0) }
    #[allow(dead_code)]
    pub fn plausible(&self)->bool { self.width()>=18 && self.height()>=12 && self.width()<1600 && self.height()<900 }
    pub fn distance(&self, other: &Self)->f64 {
        let dx=(other.left-self.right).max(self.left-other.right).max(0) as f64;
        let dy=(other.top-self.bottom).max(self.top-other.bottom).max(0) as f64;
        dx.hypot(dy)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Candidate { pub text:String, pub bounds:Rect, pub selected:bool }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateSnapshot {
    pub text:String, pub selected_index:usize, pub candidates:Vec<Candidate>,
    pub bounds:Rect, pub window_bounds:Rect, pub process_id:u32, pub source:String,
    pub revision:u64,
}
impl CandidateSnapshot {
    pub fn content_key(&self)->String {
        let mut key = format!("{}:{}:{}",self.process_id,self.selected_index,self.text);
        for c in &self.candidates { key.push('\u{1f}');key.push_str(&c.text); }
        key
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Translation {
    pub original:String, pub translated:String, pub target:String, pub engine:String,
    pub alternatives:Vec<String>, pub elapsed_ms:u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderProfile {
    pub id:String, pub label:String, pub protocol:String, pub base_url:String,
    pub model:String, pub app_id:Option<String>, pub enabled:bool,
}
impl Default for ProviderProfile {
    fn default()->Self {Self{id:"ollama".into(),label:"Ollama (本地)".into(),protocol:"ollama".into(),base_url:"http://127.0.0.1:11434".into(),model:"translategemma:4b".into(),app_id:None,enabled:true}}
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub source:String, pub target:String, pub provider_id:String, pub providers:Vec<ProviderProfile>,
    pub enabled:bool, pub show_candidate_row:bool, pub start_minimized:bool,
    pub poll_active_ms:u64, pub poll_idle_ms:u64, pub hud_gap:i32,
    pub font_size:u32, pub auto_speech:bool, pub tts_provider:String, pub tts_voice:String, pub tts_cloud_consent:bool, pub cloud_consent:bool,
    pub show_alternatives:bool, pub max_options:usize, pub enable_ime_diagnostics:bool,
    pub close_prompt:bool, pub close_action:String, pub theme:String, pub density:String, pub dark:bool,
}
impl Default for AppSettings {
 fn default()->Self {
  Self{source:"zh".into(),target:"en".into(),provider_id:"auto".into(),
   providers:vec![
    ProviderProfile::default(),
    ProviderProfile{id:"llamacpp".into(),label:"llama.cpp (本地)".into(),protocol:"openai".into(),base_url:"http://127.0.0.1:8080/v1".into(),model:"Index-Translate-2B".into(),app_id:None,enabled:true},
    ProviderProfile{id:"deepseek".into(),label:"DeepSeek".into(),protocol:"openai".into(),base_url:"https://api.deepseek.com/v1".into(),model:"deepseek-chat".into(),app_id:None,enabled:true},
    ProviderProfile{id:"openai".into(),label:"OpenAI · Responses API".into(),protocol:"responses".into(),base_url:"https://api.openai.com/v1".into(),model:"".into(),app_id:None,enabled:true},
    ProviderProfile{id:"anthropic".into(),label:"Anthropic".into(),protocol:"anthropic".into(),base_url:"https://api.anthropic.com".into(),model:"".into(),app_id:None,enabled:true},
    ProviderProfile{id:"gemini".into(),label:"Google Gemini".into(),protocol:"gemini".into(),base_url:"https://generativelanguage.googleapis.com/v1beta".into(),model:"".into(),app_id:None,enabled:true},
    ProviderProfile{id:"siliconflow".into(),label:"硅基流动".into(),protocol:"openai".into(),base_url:"https://api.siliconflow.cn/v1".into(),model:"".into(),app_id:None,enabled:true},
    ProviderProfile{id:"openrouter".into(),label:"OpenRouter".into(),protocol:"openai".into(),base_url:"https://openrouter.ai/api/v1".into(),model:"".into(),app_id:None,enabled:true},
    ProviderProfile{id:"deepl".into(),label:"DeepL".into(),protocol:"deepl".into(),base_url:"https://api-free.deepl.com".into(),model:"".into(),app_id:None,enabled:false},
    ProviderProfile{id:"baidu".into(),label:"百度翻译".into(),protocol:"baidu".into(),base_url:"https://fanyi-api.baidu.com".into(),model:"".into(),app_id:None,enabled:false},
    ProviderProfile{id:"libre".into(),label:"LibreTranslate".into(),protocol:"libre".into(),base_url:"http://127.0.0.1:5000".into(),model:"".into(),app_id:None,enabled:false},
    ProviderProfile{id:"custom".into(),label:"自定义兼容 API".into(),protocol:"openai".into(),base_url:"".into(),model:"".into(),app_id:None,enabled:true}
   ],enabled:true,show_candidate_row:false,start_minimized:false,poll_active_ms:75,poll_idle_ms:240,
   hud_gap:9,font_size:16,auto_speech:false,tts_provider:"system".into(),tts_voice:"".into(),tts_cloud_consent:false,cloud_consent:false,
   show_alternatives:true,max_options:4,enable_ime_diagnostics:false,
   close_prompt:true,close_action:"tray".into(),theme:"blossom".into(),density:"cozy".into(),dark:false}
 }
}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct VocabRow {pub source:String,pub target_lang:String,pub translated:String,pub repetitions:i32,pub interval_days:i32,pub due_at:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct HistoryRow {pub id:i64,pub original:String,pub translated:String,pub source_lang:String,pub target_lang:String,pub engine:String,pub elapsed_ms:u64,pub created_at:String}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct Metrics {pub snapshots:u64,pub translations:u64,pub cache_hits:u64,pub errors:u64,pub last_reader_status:String,pub last_translation_ms:u64}
#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ModelRecord {pub name:String,pub size:u64,pub family:String}

pub fn language_name(lang:&str)->&'static str {
 match lang {"zh"=>"Chinese", "en"=>"English", "ja"=>"Japanese", "ko"=>"Korean", "fr"=>"French", "de"=>"German", "es"=>"Spanish", "ru"=>"Russian", "it"=>"Italian", "pt"=>"Portuguese", "ar"=>"Arabic", "th"=>"Thai", _=>"Auto-detected language"}
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn geometry(){let a=Rect{left:20,top:20,right:90,bottom:50};let b=Rect{left:100,top:20,right:120,bottom:50};assert_eq!(a.width(),70);assert_eq!(a.distance(&b),10.);}
 #[test] fn key_changes_with_selected(){let c=Candidate{text:"你好".into(),bounds:Rect::default(),selected:true};let s=CandidateSnapshot{text:c.text.clone(),selected_index:0,candidates:vec![c],bounds:Rect::default(),window_bounds:Rect::default(),process_id:2,source:"UIA".into(),revision:1};let mut t=s.clone();t.selected_index=1;assert_ne!(s.content_key(),t.content_key());}
 #[test] fn default_has_no_cloud_selected(){let s=AppSettings::default();assert_eq!(s.provider_id,"auto");assert!(!s.cloud_consent);}
}
