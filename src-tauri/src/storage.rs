use crate::domain::{HistoryRow,AppSettings,VocabRow};
use anyhow::{Context,Result};
use chrono::{Duration,Utc};
use parking_lot::Mutex;
use rusqlite::{params,Connection,OpenFlags};
use std::{fs,path::{Path,PathBuf}};

pub struct LocalStore { pub config_file:PathBuf, vocab:Mutex<Connection>, lexicon:serde_json::Value, cedict:Option<Mutex<Connection>>, ecdict:Option<Mutex<Connection>> }
impl LocalStore {
 pub fn new(data_dir:&Path, resource_dir:&Path)->Result<Self> {
  fs::create_dir_all(data_dir)?;
  let vocab=Connection::open(data_dir.join("vocabulary.sqlite3"))?;
  vocab.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=1000; CREATE TABLE IF NOT EXISTS vocabulary (source TEXT NOT NULL,target_lang TEXT NOT NULL,translated TEXT NOT NULL,created_at TEXT NOT NULL,due_at TEXT NOT NULL,repetitions INTEGER NOT NULL DEFAULT 0,interval_days INTEGER NOT NULL DEFAULT 1,ease REAL NOT NULL DEFAULT 2.5,PRIMARY KEY(source,target_lang));")?;
  vocab.execute_batch("CREATE TABLE IF NOT EXISTS input_history (id INTEGER PRIMARY KEY AUTOINCREMENT,original TEXT NOT NULL,translated TEXT NOT NULL,source_lang TEXT NOT NULL,target_lang TEXT NOT NULL,engine TEXT NOT NULL,elapsed_ms INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL);CREATE INDEX IF NOT EXISTS idx_input_history_time ON input_history(created_at DESC);")?;
  let lexicon_path=resource_dir.join("lexicon.json");
  let lexicon=fs::read_to_string(&lexicon_path).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or_else(||serde_json::json!({}));
  let read_only=|filename:&str|Connection::open_with_flags(resource_dir.join(filename),OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX).ok().map(Mutex::new);
  Ok(Self{config_file:data_dir.join("settings.json"),vocab:Mutex::new(vocab),lexicon,cedict:read_only("cedict.sqlite3"),ecdict:read_only("ecdict_rank.sqlite3")})
 }
 pub fn load_settings(&self)->AppSettings {
  // Existing installs predate the dedicated MT adapters: merge the new
  // provider entries in (disabled) so they show up in settings.
  let mut settings:AppSettings=fs::read(&self.config_file).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_default();
  for mut p in AppSettings::default().providers {
   if !settings.providers.iter().any(|x|x.id==p.id) {p.enabled=false;settings.providers.push(p);}
  }
  settings
 }
 pub fn save_settings(&self,s:&AppSettings)->Result<()> {
  let tmp=self.config_file.with_extension("json.tmp");fs::write(&tmp,serde_json::to_vec_pretty(s)?)?;fs::rename(&tmp,&self.config_file).context("save settings")
 }
 pub fn lookup(&self,raw:&str,target:&str)->Option<Vec<String>> {
  let term=raw.trim();if term.is_empty()||term.chars().count()>120{return None;}
  let lowercase=term.to_lowercase();
  let node=self.lexicon.get(term).or_else(||self.lexicon.get(lowercase.as_str()));
  if let Some(value)=node.and_then(|n|n.get(target)) {
   if let Some(x)=value.as_str(){return Some(vec![x.to_owned()]);}
   if let Some(a)=value.as_array(){let vals=a.iter().filter_map(|v|v.as_str()).take(5).map(|s|s.to_owned()).collect::<Vec<_>>();if !vals.is_empty(){return Some(vals);}}
  }
  if target!="en" {return None;}
  let mut senses=Vec::<String>::new();
  if let Some(db)=&self.cedict {
    if let Ok(mut q)=db.lock().prepare("SELECT gloss FROM terms JOIN senses USING(entry_id) WHERE term=?1 ORDER BY entry_id,sense_order LIMIT 24"){
     if let Ok(rows)=q.query_map(params![term],|r|r.get::<_,String>(0)){for g in rows.flatten(){let core=strip_gloss_notes(&g);for part in core.split([';','/']){clean_gloss(part,&mut senses);}}}
    }
  }
  if senses.len()<3 {
   if let Some(db)=&self.ecdict {
    if let Ok(mut q)=db.lock().prepare("SELECT english FROM ranks WHERE term=?1 ORDER BY frequency,collins DESC,oxford DESC LIMIT 12"){
     if let Ok(rows)=q.query_map(params![term],|r|r.get::<_,String>(0)){for g in rows.flatten(){clean_gloss(&g,&mut senses);}}
    }
   }
  }
  if senses.is_empty(){None}else {Some(senses.into_iter().take(4).collect())}
 }
 pub fn add_vocab(&self,source:&str,lang:&str,translated:&str)->Result<()> {
   if source.trim().is_empty()||translated.trim().is_empty(){anyhow::bail!("cannot save empty word")}
   let now=Utc::now().to_rfc3339();self.vocab.lock().execute("INSERT INTO vocabulary(source,target_lang,translated,created_at,due_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(source,target_lang) DO UPDATE SET translated=excluded.translated",params![source,lang,translated,now])?;Ok(())
 }
 pub fn list_vocab(&self,due_only:bool)->Result<Vec<VocabRow>>{
  let db=self.vocab.lock();let sql=if due_only {"SELECT source,target_lang,translated,repetitions,interval_days,due_at FROM vocabulary WHERE due_at<=?1 ORDER BY due_at"}else{"SELECT source,target_lang,translated,repetitions,interval_days,due_at FROM vocabulary WHERE (?1 IS NOT NULL) ORDER BY created_at DESC"};
  let mut query=db.prepare(sql)?;let rows=query.query_map(params![Utc::now().to_rfc3339()],|r|Ok(VocabRow{source:r.get(0)?,target_lang:r.get(1)?,translated:crate::providers::tidy_translation(&r.get::<_,String>(2)?),repetitions:r.get(3)?,interval_days:r.get(4)?,due_at:r.get(5)?}))?;Ok(rows.collect::<rusqlite::Result<_>>()?)
 }
 pub fn review(&self,source:&str,lang:&str,quality:i32)->Result<()>{
  if !(0..=5).contains(&quality){anyhow::bail!("quality must be 0..5")}
  let db=self.vocab.lock();let (mut reps,mut interval,mut ease):(i32,i32,f64)=db.query_row("SELECT repetitions,interval_days,ease FROM vocabulary WHERE source=?1 AND target_lang=?2",params![source,lang],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
  let error=(5-quality) as f64;ease=(ease+0.1-error*(0.08+error*0.02)).max(1.3);
  if quality<3{reps=0;interval=1;}else{reps+=1;interval=if reps==1{1}else if reps==2{6}else{((interval as f64)*ease).round().max(1.) as i32};}
  let due=(Utc::now()+Duration::days(interval as i64)).to_rfc3339();db.execute("UPDATE vocabulary SET repetitions=?1,interval_days=?2,ease=?3,due_at=?4 WHERE source=?5 AND target_lang=?6",params![reps,interval,ease,due,source,lang])?;Ok(())
 }
 pub fn delete_vocab(&self,source:&str,lang:&str)->Result<()> { self.vocab.lock().execute("DELETE FROM vocabulary WHERE source=?1 AND target_lang=?2",params![source,lang])?;Ok(()) }
 // Input history: local only, capped, never leaves this machine.
 pub fn add_history(&self,original:&str,translated:&str,source:&str,target:&str,engine:&str,elapsed_ms:u64)->Result<()> {
  let text=original.trim();
  if text.is_empty()||text.chars().count()>500{return Ok(());}
  let conn=self.vocab.lock();
  conn.execute("INSERT INTO input_history (original,translated,source_lang,target_lang,engine,elapsed_ms,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
   params![text,translated,source,target,engine,elapsed_ms,chrono::Utc::now().to_rfc3339()])?;
  // Keep the table small enough that the page opens instantly.
  conn.execute("DELETE FROM input_history WHERE id NOT IN (SELECT id FROM input_history ORDER BY id DESC LIMIT 3000)",[])?;
  Ok(())
 }
 pub fn list_history(&self,limit:u32)->Result<Vec<HistoryRow>> {
  let conn=self.vocab.lock();
  let mut stmt=conn.prepare("SELECT id,original,translated,source_lang,target_lang,engine,elapsed_ms,created_at FROM input_history ORDER BY id DESC LIMIT ?1")?;
  let rows=stmt.query_map([limit.clamp(1,3000)],|r|Ok(HistoryRow{id:r.get(0)?,original:r.get(1)?,translated:crate::providers::tidy_translation(&r.get::<_,String>(2)?),source_lang:r.get(3)?,target_lang:r.get(4)?,engine:r.get(5)?,elapsed_ms:r.get::<_,i64>(6)? as u64,created_at:r.get(7)?}))?;
  Ok(rows.filter_map(Result::ok).collect())
 }
 pub fn delete_history(&self,id:i64)->Result<()> { self.vocab.lock().execute("DELETE FROM input_history WHERE id=?1",params![id])?;Ok(()) }
 pub fn clear_history(&self)->Result<()> { self.vocab.lock().execute("DELETE FROM input_history",[])?;Ok(()) }pub fn export_csv(&self)->Result<String> {
  fn field(s:&str)->String{format!("\"{}\"",s.replace('"',"\"\""))}
  let mut text="\u{feff}原文,目标语言,译文,复习次数,间隔天数,下次复习\r\n".to_owned();for x in self.list_vocab(false)?{text.push_str(&format!("{},{},{},{},{},{}\r\n",field(&x.source),field(&x.target_lang),field(&x.translated),x.repetitions,x.interval_days,field(&x.due_at)));}Ok(text)
 }
}
fn strip_gloss_notes(raw:&str)->String {
 let mut out=String::new();let mut depth=0usize;
 for ch in raw.chars() {
  match ch {
   '('|'（' => {depth+=1;if depth==1{out.push(' ');}},
   ')'|'）' => {if depth>0{depth-=1;out.push(' ');}},
   _ if depth==0 => out.push(ch),
   _=>{}
  }
 }
 out.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c:char| c==','||c==';'||c==':'||c==' ').to_owned()
}
fn clean_gloss(raw:&str,out:&mut Vec<String>) {
 let s=strip_gloss_notes(raw);let low=s.to_lowercase();
 if s.chars().count()>45 || s.is_empty() || s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) || ["variant of","old variant","see ","also written","surname ","cantonese","dialect","abbr. for","(cl:"].iter().any(|p|low.starts_with(p)){return;}
 if !out.contains(&s){out.push(s)}
}
#[cfg(test)] mod tests{use super::*;
 #[test] fn basic_gloss_filter(){let mut r=vec![];clean_gloss("hello",&mut r);clean_gloss("variant of abc",&mut r);assert_eq!(r,["hello"]);}
 #[test] fn gloss_notes_are_removed(){assert_eq!(strip_gloss_notes("a text (article, script, contract etc.)"),"a text");assert_eq!(strip_gloss_notes("(bound form) father"),"father");assert_eq!(strip_gloss_notes("module (in software"),"module");}
}
