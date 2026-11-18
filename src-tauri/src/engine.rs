use crate::{domain::{AppSettings,ProviderProfile,Translation},providers::ProviderClient,storage::LocalStore};
use anyhow::{anyhow,Result};
use lru::LruCache;
use parking_lot::Mutex;
use std::{collections::HashMap,num::NonZeroUsize,sync::Arc,time::{Duration,Instant}};
use tokio::sync::Semaphore;

#[derive(Clone,Copy,Default)]
pub struct ProviderStat {pub ok:u64,pub fail:u64,pub last_ms:u64,pub ema_ms:u64}

pub struct TranslationEngine {
  pub provider: ProviderClient,
  cache:Mutex<LruCache<String,Translation>>,
  local_gpu:Arc<Semaphore>,
  // A provider that just failed is not retried on the next keystroke: the
  // user is still typing and a dead endpoint must not add latency to every word.
  cooldown:Mutex<HashMap<String,Instant>>,
  stats:Mutex<HashMap<String,ProviderStat>>,
}
impl TranslationEngine {
 pub fn new()->Self {Self{
  provider:ProviderClient::new(),
  cache:Mutex::new(LruCache::new(NonZeroUsize::new(2048).unwrap())),
  local_gpu:Arc::new(Semaphore::new(1)),
  cooldown:Mutex::new(HashMap::new()),
  stats:Mutex::new(HashMap::new()),
 }}
 pub fn reset(&self){self.cache.lock().clear();}
 pub fn stats(&self)->Vec<(String,ProviderStat)>{
  self.stats.lock().iter().map(|(k,v)|(k.clone(),*v)).collect()
 }
 fn cooled(&self,id:&str)->bool{
  let map=self.cooldown.lock();
  map.get(id).map(|until|Instant::now()<*until).unwrap_or(false)
 }
 fn cool(&self,id:&str,err:&str){
  let seconds=if err.contains("429")||err.contains("rate") {60} else {15};
  self.cooldown.lock().insert(id.to_owned(),Instant::now()+Duration::from_secs(seconds));
 }
 fn record(&self,id:&str,ok:bool,ms:u64){
  let mut map=self.stats.lock();
  let e=map.entry(id.to_owned()).or_default();
  if ok {e.ok+=1;e.last_ms=ms;e.ema_ms=if e.ema_ms==0 {ms} else {(e.ema_ms*3+ms)/4};}
  else {e.fail+=1;}
 }
 /// Ordered list of providers to try for this request.
 fn chain<'a>(&self,settings:&'a AppSettings)->Vec<&'a ProviderProfile>{
  let all=settings.providers.iter().filter(|p|p.enabled);
  if settings.provider_id!="auto" {
   return all.filter(|p|p.id==settings.provider_id).collect();
  }
  let mt=|p:&&ProviderProfile|matches!(p.protocol.as_str(),"deepl"|"libre"|"baidu");
  let local=|p:&&ProviderProfile|p.id=="ollama"||p.id=="llamacpp";
  let mut chain:Vec<&ProviderProfile>=Vec::new();
  chain.extend(all.clone().filter(mt));            // dedicated MT: one round trip
  chain.extend(all.clone().filter(local));         // local model, already warm
  chain.extend(all.filter(|p|!mt(&p)&&!local(&p))); // remote LLM
  chain
 }
 pub async fn translate(&self,store:&LocalStore,settings:&AppSettings,text:&str)->Result<(Translation,bool)>{
  let source=settings.source.as_str();let target=settings.target.as_str();
  let text=text.trim();if text.is_empty(){return Err(anyhow!("empty candidate"))}
  let chain=self.chain(settings);
  let fingerprint=chain.first().map(|p|format!("{}|{}|{}|{}",p.id,p.model,p.base_url,p.protocol)).unwrap_or_else(||settings.provider_id.clone());
  let key=format!("{source}\u{1f}{target}\u{1f}{fingerprint}\u{1f}{text}");
  if let Some(v)=self.cache.lock().get(&key).cloned(){return Ok((v,true))}
  let start=Instant::now();
  if source==target {return Ok((Translation{original:text.into(),translated:text.into(),target:target.into(),engine:"原文".into(),alternatives:vec![],elapsed_ms:0},false))}
  let offline=settings.provider_id=="auto"||settings.provider_id=="offline";
  if offline {
    if let Some(choices)=store.lookup(text,target){
     let t=Translation{original:text.into(),translated:choices[0].clone(),target:target.into(),engine:"本地词典".into(),alternatives:choices.iter().skip(1).cloned().collect(),elapsed_ms:start.elapsed().as_millis() as u64};
     self.cache.lock().put(key,t.clone());return Ok((t,false));
    }
    if settings.provider_id=="offline"{return Err(anyhow!("本地词典暂无该词的译文"))}
  }
  let mut last_error=anyhow!("没有可用的翻译 Provider");
  let mut tried=0;
  for profile in chain {
   if self.cooled(&profile.id) {continue}
   // Cloud providers need explicit consent; a local endpoint never does.
   let local=crate::providers::is_local_url(&profile.base_url);
   if !settings.cloud_consent && !local {last_error=anyhow!("未授权向云端发送输入法候选词");continue}
   tried+=1;
   let budget=crate::providers::token_budget(text);
   let _gpu_permit=if profile.id=="ollama"||profile.id=="llamacpp" {Some(self.local_gpu.acquire().await?)}else{None};
   let t0=Instant::now();
   match self.provider.translate(profile,text,source,target,budget).await {
    Ok(translated)=>{
     let ms=t0.elapsed().as_millis() as u64;
     self.record(&profile.id,true,ms);
     let t=Translation{original:text.into(),translated,target:target.into(),
      engine:profile.label.clone(),alternatives:vec![],elapsed_ms:start.elapsed().as_millis() as u64};
     self.cache.lock().put(key,t.clone());return Ok((t,false));
    }
    Err(e)=>{
     let message=e.to_string();
     self.record(&profile.id,false,0);
     self.cool(&profile.id,&message);
     crate::trace(&format!("provider {} failed: {}",profile.id,message.chars().take(80).collect::<String>()));
     last_error=e;
    }
   }
  }
  if tried==0 {return Err(anyhow!("没有可用的翻译 Provider（已全部处于退避或无授权）"))}
  Err(last_error)
 }
}