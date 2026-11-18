//! Windows IME candidate reader. No keylogger, no screenshot, no clipboard.
//! Mirrors the proven GlintWin pipeline: hit test a few points around the
//! caret, rebuild the whole bar from ListItem siblings, and hold the last
//! snapshot while the bar merely repaints.
use crate::domain::{CandidateSnapshot,Rect};

pub fn normalize_candidate(raw:&str)->Option<String>{
 let value=raw.trim().trim_start_matches(|c:char|c.is_ascii_digit()||c=='.'||c==' '||c=='\t');
 let val=value.trim().trim_matches(['\r','\n']);
 if val.is_empty()||val.chars().count()>120||val.chars().any(|c|c.is_control()){return None}
 if ["候选","候选词","中文(简体)","输入法","More candidates","设置"].contains(&val){return None}
 Some(val.to_owned())
}
#[allow(dead_code)]
pub fn candidate_changed(a:&CandidateSnapshot,b:&CandidateSnapshot)->bool {a.content_key()!=b.content_key()}
pub fn hud_placement(bounds:Rect,monitor:Rect,hud_w:i32,hud_h:i32,gap:i32)->(i32,i32){
 let x=bounds.left.clamp(monitor.left,(monitor.right-hud_w).max(monitor.left));
 let mut y=bounds.bottom+gap;
 if y+hud_h>monitor.bottom {y=bounds.top-hud_h-gap;}
 let y=y.clamp(monitor.top,(monitor.bottom-hud_h).max(monitor.top));(x,y)
}

#[cfg(windows)] mod win {
 use super::*;
 use crate::domain::Candidate;
 use std::{mem::size_of,time::{Duration,Instant}};
 use uiautomation::{UIAutomation,UIElement,patterns::{UISelectionItemPattern,UISelectionPattern,UITextEditPattern},types::{ControlType,Point as UIAPoint,TreeScope,UIProperty},variants::Value};

 #[repr(C)]#[derive(Clone,Copy,Default)] struct WRect {left:i32,top:i32,right:i32,bottom:i32}
 impl WRect {fn to_rect(self)->Rect{Rect{left:self.left,top:self.top,right:self.right,bottom:self.bottom}}}
 #[repr(C)]#[derive(Clone,Copy,Default)] struct Point{x:i32,y:i32}
 #[repr(C)]#[derive(Clone,Copy,Default)] struct GUIThreadInfo {size:u32,flags:u32,active:isize,focus:isize,capture:isize,menu_owner:isize,move_size:isize,caret:isize,rc_caret:WRect}
 #[link(name="user32")]
 unsafe extern "system" {
  fn GetWindowRect(hwnd:isize,rect:*mut WRect)->i32;
  fn GetForegroundWindow()->isize;
  fn GetWindowThreadProcessId(hwnd:isize,pid:*mut u32)->u32;
  fn GetGUIThreadInfo(thread:u32,info:*mut GUIThreadInfo)->i32;
  fn ClientToScreen(hwnd:isize,point:*mut Point)->i32;
 }
 #[link(name="kernel32")]
 unsafe extern "system" {
  fn OpenProcess(access:u32,inherit:i32,pid:u32)->isize;
  fn CloseHandle(handle:isize)->i32;
  fn QueryFullProcessImageNameW(handle:isize,flags:u32,buf:*mut u16,len:*mut u32)->i32;
 }

 fn process_name(pid:u32)->String {
  unsafe {let h=OpenProcess(0x1000,0,pid);if h==0{return String::new()}
   let mut buf=[0u16;1024];let mut len=1024;
   let ok=QueryFullProcessImageNameW(h,0,buf.as_mut_ptr(),&mut len);
   CloseHandle(h);
   if ok==0{return String::new()}
   let full=String::from_utf16_lossy(&buf[..len as usize]);
   full.rsplit(['\\','/']).next().unwrap_or("").to_ascii_lowercase()}
 }
 fn is_ime_host(pid:u32)->bool {
  ["textinputhost.exe","inputapp.exe","chsime.exe","jpnime.exe","ctfmon.exe"].contains(&process_name(pid).as_str())
 }
 fn window_bounds(hwnd:isize)->Option<Rect>{
  if hwnd==0{return None}
  let mut r=WRect::default();
  (unsafe{GetWindowRect(hwnd,&mut r)}!=0).then(|| r.to_rect())
 }
 fn current_focus()->(isize,u32,Option<Rect>){
  let foreground=unsafe{GetForegroundWindow()};
  let mut pid=0;let tid=unsafe{GetWindowThreadProcessId(foreground,&mut pid)};
  let mut info=GUIThreadInfo::default();info.size=size_of::<GUIThreadInfo>() as u32;
  let mut caret=None;
  if tid!=0&&unsafe{GetGUIThreadInfo(tid,&mut info)}!=0&&info.caret!=0 {
   let mut p=Point{x:info.rc_caret.left,y:info.rc_caret.top};
   let mut q=Point{x:info.rc_caret.right.max(p.x+1),y:info.rc_caret.bottom.max(p.y+1)};
   if unsafe{ClientToScreen(info.caret,&mut p)}!=0&&unsafe{ClientToScreen(info.caret,&mut q)}!=0 {
    let r=Rect{left:p.x,top:p.y,right:q.x,bottom:q.y};
    if r.height()>0&&r.height()<150 {caret=Some(r)}
   }
  }
  if caret.is_none() {caret=caret_from_focus();}
  (foreground,pid,caret)
 }
 // Chromium apps report no caret through GetGUIThreadInfo; ask UIA for the
 // focused element's box and use its bottom-left corner instead.
 fn caret_from_focus()->Option<Rect>{
  let automation=UIAutomation::new().ok()?;
  let element=automation.get_focused_element().ok()?;
  let b=element.get_bounding_rectangle().ok()?;
  let rect=Rect{left:b.get_left(),top:b.get_top(),right:b.get_right(),bottom:b.get_bottom()};
  if rect.width()<=0||rect.height()<=0 {return None}
  Some(Rect{left:rect.left+4,top:(rect.bottom-22).max(rect.top),right:rect.left+5,bottom:rect.bottom})
 }
 fn element_rect(el:&UIElement)->Option<Rect>{
  let b=el.get_bounding_rectangle().ok()?;
  let rect=Rect{left:b.get_left(),top:b.get_top(),right:b.get_right(),bottom:b.get_bottom()};
  if rect.width()>2&&rect.height()>2 {Some(rect)} else {None}
 }
 fn selected(el:&UIElement)->bool {
  if let Ok(value)=el.get_property_value(UIProperty::SelectionItemIsSelected) {
   if matches!(value.get_value(),Ok(Value::BOOL(true))){return true}
  }
  if let Ok(pattern)=el.get_pattern::<UISelectionItemPattern>() {
   if pattern.is_selected().unwrap_or(false){return true}
  }
  if el.has_keyboard_focus().ok().unwrap_or(false){return true}
  el.get_item_status().ok().map(|s|{let l=s.to_lowercase();l.contains("selected")||s.contains("选中")}).unwrap_or(false)
 }
 fn row_of(el:&UIElement)->Option<(String,Rect,bool)> {
  let kind=el.get_control_type().ok()?;
  if !(matches!(kind,ControlType::ListItem)||matches!(kind,ControlType::DataItem)) {return None}
  let name=el.get_name().ok()?;
  let text=normalize_candidate(&name)?;
  let rect=element_rect(el)?;
  Some((text,rect,selected(el)))
 }
 // A hit often lands on one cell (or a Text inside it). Climb to the row and
 // walk its siblings so the whole bar — and therefore the highlighted index —
 // is available even when the panel itself cannot be traversed.
 fn rows_siblings(uia:&UIAutomation,element:&UIElement)->Vec<(String,Rect,bool)>{
  let Ok(walker)=uia.get_raw_view_walker() else {return Vec::new()};
  let mut anchor=element.clone();
  let mut seed=row_of(&anchor);
  for _ in 0..4 {
   if seed.is_some(){break}
   match walker.get_parent(&anchor){Ok(p)=>{anchor=p;seed=row_of(&anchor)},Err(_)=>break}
  }
  let mut rows=Vec::new();
  if let Some(seed)=seed.clone(){rows.push(seed)}
  if rows.is_empty(){return rows}
  for forward in [false,true] {
   let mut node=anchor.clone();
   for _ in 0..12 {
    node=match if forward {walker.get_next_sibling(&node)} else {walker.get_previous_sibling(&node)} {Ok(n)=>n,Err(_)=>break};
    match row_of(&node) {Some(r)=>if forward {rows.push(r)} else {rows.insert(0,r)},None=>{}}
   }
  }
  rows
 }
 pub enum BarState { Alive, Gone, Unknown }

#[derive(Debug)]
pub struct WindowsReader {
  uia:UIAutomation,last_status:String,pub read_count:u64,
  last_snapshot:Option<CandidateSnapshot>,last_hit_at:Instant,last_hit_offset:Option<(i32,i32)>,
  dump_at:Instant,
  last_composition:Option<String>,
  panel_element:Option<UIElement>,panel_at:Instant,
  last_options:Vec<String>,last_index:usize,misses:u32,
  last_bar:Option<Rect>,scan_next:Instant,
 }
 impl WindowsReader {
  pub fn new()->Result<Self,String>{Ok(Self{
   uia:UIAutomation::new().map_err(|e|e.to_string())?,
   last_status:"waiting".into(),read_count:0,
   last_snapshot:None,
   dump_at:Instant::now()-Duration::from_secs(5),
   last_composition:None,
   last_hit_at:Instant::now()-Duration::from_secs(5),
   last_hit_offset:None,
   panel_element:None,
   panel_at:Instant::now()-Duration::from_secs(5),
   last_options:Vec::new(),last_index:0,misses:0,
   last_bar:None,
   scan_next:Instant::now(),
  })}
  pub fn status(&self)->String {self.last_status.clone()}
  pub fn has_panel(&self)->bool {self.panel_element.is_some()}

  // Is the bar still painted under the cell we last reported? A list item at
  // that point means yes; the composition line that takes its place right
  // after a commit is not a list item, so it does not fool us.
  // Three-way answer. `Gone` is proven by the pixel under the cell falling
  // back to the *application* process: while the bar lives, that pixel belongs
  // to the IME host, so seeing the app there means the panel really closed.
  fn bar_state(&self)->BarState {
   let Some(snapshot)=&self.last_snapshot else {return BarState::Gone};
   let cell_box=if snapshot.bounds.width()>0 {snapshot.bounds} else {snapshot.window_bounds};
   if cell_box.width()<=0||cell_box.height()<=0 {return BarState::Gone}
   let x=cell_box.left+(cell_box.width()/3).max(4);
   let y=cell_box.top+cell_box.height()/2;
   let Ok(mut element)=self.uia.element_from_point(UIAPoint::new(x,y)) else {return BarState::Unknown};
   let Ok(walker)=self.uia.get_raw_view_walker() else {return BarState::Unknown};
   for _ in 0..6 {
    if let Ok(kind)=element.get_control_type() {
     if matches!(kind,ControlType::ListItem)||matches!(kind,ControlType::DataItem) {return BarState::Alive}
    }
    element=match walker.get_parent(&element){Ok(p)=>p,Err(_)=>break};
   }
   match self.uia.element_from_point(UIAPoint::new(x,y)) {
    Ok(hit)=>match hit.get_process_id() {
     Ok(pid) if pid==snapshot.process_id => BarState::Gone,
     _ => BarState::Unknown,
    },
    Err(_)=>BarState::Unknown,
   }
  }
  pub fn panel_alive(&self)->bool {matches!(self.bar_state(),BarState::Alive)}

  /// Deleted letters still in the composition (separators excluded).
  pub fn composition_letters(&self)->Option<usize> {
   self.last_composition.as_ref().map(|c|{
    c.chars().filter(|ch|!ch.is_whitespace()&&*ch!='\'').count()
   })
  }

  /// The cell we reported is definitively no longer a candidate row.
  pub fn bar_gone(&self)->bool {
   self.last_snapshot.is_some()&&matches!(self.bar_state(),BarState::Gone)
  }
  // Diagnostic: dump the panel subtree so we can see exactly which elements
  // the IME exposes (RINO_DUMP_PANEL=1, at most once every 2 s).
  fn dump_panel(&mut self,root:&UIElement) {
   if std::env::var("RINO_DUMP_PANEL").ok().as_deref()!=Some("1") {return}
   if self.dump_at.elapsed()<Duration::from_millis(2000) {return}
   self.dump_at=Instant::now();
   crate::trace(&format!("panel-dump root={:?}",root.get_name().unwrap_or_default()));
   self.dump_ancestors(root);
   self.dump_composition();
   let Ok(walker)=self.uia.get_raw_view_walker() else {return};
   let mut queue:Vec<(UIElement,usize)>=vec![(root.clone(),0)];
   let mut seen=0;
   while let Some((node,depth))=queue.pop() {
    if seen>40 {break}
    seen+=1;
    let kind=node.get_control_type().map(|k|format!("{k:?}")).unwrap_or_else(|_|"?".into());
    let name=node.get_name().unwrap_or_default();
    let class=String::new();
    let id=node.get_automation_id().unwrap_or_default();
    crate::trace(&format!("  dump {} {:?} cls={:?} id={:?} name={:?}",
      "  ".repeat(depth),kind,class,id,name.chars().take(40).collect::<String>()));
    if depth>=3 {continue}
    if let Ok(mut child)=walker.get_first_child(&node) {
     let mut kids=Vec::new();
     loop {
      kids.push((child.clone(),depth+1));
      match walker.get_next_sibling(&child) {Ok(next)=>child=next,Err(_)=>break}
     }
     for k in kids.into_iter().rev() {queue.push(k)}
    }
   }
  }

  // Walk the panel's ancestors so the pinyin line (a sibling of the candidate
  // list, not a child of it) shows up in the dump.
  fn dump_ancestors(&mut self,element:&UIElement) {
   let Ok(walker)=self.uia.get_raw_view_walker() else {return};
   let mut node=element.clone();
   for level in 1..=3 {
    let Ok(parent)=walker.get_parent(&node) else {return};
    let kind=parent.get_control_type().map(|k|format!("{k:?}")).unwrap_or_default();
    let name=parent.get_name().unwrap_or_default();
    crate::trace(&format!("panel-dump ancestor{level} {kind} name={:?}",name.chars().take(30).collect::<String>()));
    if level==3 {
     if let Ok(mut child)=walker.get_first_child(&parent) {
      let mut n=0;
      loop {
       n+=1; if n>20 {break}
       let k=child.get_control_type().map(|k|format!("{k:?}")).unwrap_or_default();
       let nm=child.get_name().unwrap_or_default();
       let r=element_rect(&child);
       crate::trace(&format!("  sib {k} name={:?} rect={:?}",nm.chars().take(30).collect::<String>(),
         r.map(|b|(b.left,b.top,b.right,b.bottom))));
       match walker.get_next_sibling(&child) {Ok(next)=>child=next,Err(_)=>break}
      }
     }
    }
    node=parent;
   }
  }

  /// Composition (pinyin) string of the focused control, when the app exposes
  /// `TextEditPattern`. Microsoft Pinyin itself does not publish it in the
  /// candidate panel, but Chromium/Word style controls do.
  fn read_composition(&mut self)->Option<String> {
   let focus=self.uia.get_focused_element().ok()?;
   let text=focus.get_pattern::<UITextEditPattern>().ok()?;
   let range=text.get_active_composition().ok()?;
   let value=range.get_text(-1).ok().unwrap_or_default();
   if value.is_empty() {None} else {Some(value)}
  }

  fn dump_composition(&mut self) {
   let Some(value)=self.read_composition() else {crate::trace("composition: none");return};
   crate::trace(&format!("composition {:?}",value.chars().take(24).collect::<String>()));
  }

  #[allow(dead_code)]
  fn old_dump_composition(&mut self) {
   let Ok(focus)=self.uia.get_focused_element() else {crate::trace("composition: no focus");return};
   let kind=focus.get_control_type().map(|k|format!("{k:?}")).unwrap_or_default();
   match focus.get_pattern::<UITextEditPattern>() {
    Ok(text)=>match text.get_active_composition() {
     Ok(range)=>{
      let value=range.get_text(-1).unwrap_or_default();
      crate::trace(&format!("composition {kind}: {:?}",value.chars().take(24).collect::<String>()));
     },
     Err(e)=>crate::trace(&format!("composition {kind}: active err {e}")),
    },
    Err(_)=>crate::trace(&format!("composition {kind}: no text pattern")),
   }
  }

  // Build a snapshot from any element inside the bar.
  fn panel_snapshot(&mut self,element:&UIElement,pid:u32,caret:Option<Rect>)->Option<CandidateSnapshot>{
   let mut rows=Vec::new();
   if let Ok(condition)=self.uia.create_property_condition(UIProperty::ControlType,(ControlType::ListItem as i32).into(),None) {
    if let Ok(found)=element.find_all(TreeScope::Descendants,&condition) {
     for node in found {
      if let Some(row)=row_of(&node){rows.push(row)}
      if rows.len()>18 {break}
     }
    }
   }
   if rows.len()<2 {rows=rows_siblings(&self.uia,element)}
   if rows.is_empty() {return None}
   // Drop nested duplicates sitting on the same pixels.
   let mut cleaned:Vec<(String,Rect,bool)>=Vec::new();
   for row in rows {
    if cleaned.iter().any(|(t,b,_)|t==&row.0&&b.distance(&row.1)<=4.){continue}
    cleaned.push(row);
   }
   if cleaned.is_empty()||cleaned.len()>15 {return None}
   let horizontal=cleaned.iter().map(|r|r.1.left).max().unwrap_or(0)-cleaned.iter().map(|r|r.1.left).min().unwrap_or(0)
       >cleaned.iter().map(|r|r.1.top).max().unwrap_or(0)-cleaned.iter().map(|r|r.1.top).min().unwrap_or(0);
   if horizontal {
    // Keep one row band; a long pinyin wraps the bar into two lines.
    let mut mids:Vec<i32>=cleaned.iter().map(|r|(r.1.top+r.1.bottom)/2).collect();
    mids.sort();let band=mids[mids.len()/2];
    let kept:Vec<(String,Rect,bool)>=cleaned.iter().filter(|r|((r.1.top+r.1.bottom)/2-band).abs()<=8||r.2).cloned().collect();
    if kept.len()>=2 {cleaned=kept}
    cleaned.sort_by_key(|r|(r.1.left,r.1.top));
   } else {cleaned.sort_by_key(|r|(r.1.top,r.1.left));}
   let options:Vec<String>=cleaned.iter().map(|r|r.0.clone()).collect();
   if options!=self.last_options {self.last_options=options;self.last_index=0;}
   let mut index=cleaned.iter().position(|r|r.2);
   if index.is_none() {
    index=self.selection_index(element,&cleaned);
   }
   let idx=index.unwrap_or_else(||self.last_index.min(cleaned.len()-1));
   if index.is_some(){self.last_index=idx}
   let chosen=cleaned.get(idx)?.clone();
   let window_bounds=cleaned.iter().fold(chosen.1,|acc,r|Rect{
    left:acc.left.min(r.1.left),top:acc.top.min(r.1.top),
    right:acc.right.max(r.1.right),bottom:acc.bottom.max(r.1.bottom)});
   // A hit on the composition line yields a single row sitting above the caret
   // — that is not the bar.
   if cleaned.len()<=1 {
    if let Some(c)=caret {if window_bounds.top<c.top-4 {return None}}
   }
   self.dump_panel(element);
   let candidates=cleaned.iter().map(|r|Candidate{text:r.0.clone(),bounds:r.1,selected:r.1.distance(&chosen.1)<=2.}).collect();
   Some(CandidateSnapshot{
    text:chosen.0,selected_index:idx,candidates,
    bounds:chosen.1,window_bounds,process_id:pid,
    source:"Microsoft 拼音 · UIA".into(),revision:0,
   })
  }
  // Ask the list container which item it considers selected; Microsoft Pinyin
  // keeps the highlight there when no row reports keyboard focus.
  fn selection_index(&self,element:&UIElement,rows:&[(String,Rect,bool)])->Option<usize>{
   let walker=self.uia.get_raw_view_walker().ok()?;
   let mut node=element.clone();
   for _ in 0..4 {
    if let Ok(pattern)=node.get_pattern::<UISelectionPattern>() {
     if let Ok(items)=pattern.get_selection() {
      for item in items {
       if let Some(rect)=element_rect(&item) {
        if let Some(index)=rows.iter().position(|r|r.1.distance(&rect)<=4.) {return Some(index)}
       }
      }
     }
    }
    node=match walker.get_parent(&node) {Ok(p)=>p,Err(_)=>return None};
   }
   None
  }
  fn hit_snapshot(&mut self,caret:Option<Rect>,deep:bool,pid:u32)->Option<CandidateSnapshot>{
   let caret=caret?;
   let mut points:Vec<(i32,i32)>=vec![(38,9),(12,8),(90,10),(170,9),(-40,9),(-120,9)];
   if deep&&self.last_hit_at.elapsed()<Duration::from_millis(1500) {
    points.extend_from_slice(&[(250,9),(330,9),(-200,9),(-280,9),(38,22),(120,22),(38,34),(-60,22),(38,-14),(120,-26)]);
   }
   if let Some(offset)=self.last_hit_offset {
    points.retain(|p|p!=&offset);
    points.insert(0,offset);
   }
   if !deep {points.truncate(2);}
   for (dx,dy) in points {
    let (x,y)=(caret.right+dx,caret.bottom+dy);
    let Ok(element)=self.uia.element_from_point(UIAPoint::new(x,y)) else {continue};
    let Ok(element_pid)=element.get_process_id() else {continue};
    if !is_ime_host(element_pid as u32) {continue}
    if let Some(snapshot)=self.panel_snapshot(&element,pid,Some(caret)) {
     self.last_hit_offset=Some((dx,dy));
     self.panel_element=Some(element);
     self.panel_at=Instant::now();
     return Some(snapshot)
    }
   }
   None
  }
  fn panel_from_cache(&mut self,pid:u32,caret:Option<Rect>)->Option<CandidateSnapshot>{
   let Some(panel)=self.panel_element.clone() else {return None};
   if self.panel_at.elapsed()>Duration::from_millis(500){return None}
   if panel.is_offscreen().unwrap_or(false){return None}
   self.panel_snapshot(&panel,pid,caret)
  }
  fn probe_last_bar(&mut self,pid:u32,caret:Option<Rect>)->Option<CandidateSnapshot>{
   let bar=self.last_bar?;
   let cy=(bar.top+bar.bottom)/2;
   let quarter=(bar.right-bar.left)/4;
   let points=[((bar.left+bar.right)/2,cy),(bar.left+quarter,cy),(bar.right-quarter,cy),
               (bar.left+quarter,cy+8),(bar.right-quarter,cy+8),(bar.left+10,bar.top+4),(bar.right+10,cy)];
   for (x,y) in points {
    let Ok(element)=self.uia.element_from_point(UIAPoint::new(x,y)) else {continue};
    let Ok(element_pid)=element.get_process_id() else {continue};
    if !is_ime_host(element_pid as u32) {continue}
    if let Some(snapshot)=self.panel_snapshot(&element,pid,caret) {return Some(snapshot)}
   }
   None
  }
  // Last resort, only when nothing is known about the bar: sample a grid
  // under the focused window, the way the Python prototype does.
  fn scan_window(&mut self,bounds:Rect,pid:u32,caret:Option<Rect>)->Option<CandidateSnapshot>{
   if bounds.width()<=0||bounds.height()<=0 {return None}
   let ys=[bounds.bottom+14,bounds.bottom-64,bounds.bottom-116,bounds.bottom-172];
   let span=(bounds.width()-160).clamp(240,1400);
   for y in ys {
    for i in 0..8 {
     let x=bounds.left+60+i*(span/8).max(1);
     let Ok(element)=self.uia.element_from_point(UIAPoint::new(x,y)) else {continue};
     let Ok(element_pid)=element.get_process_id() else {continue};
     if !is_ime_host(element_pid as u32) {continue}
     if let Some(snapshot)=self.panel_snapshot(&element,pid,caret) {
      self.panel_element=Some(element);self.panel_at=Instant::now();
      return Some(snapshot)
     }
    }
   }
   None
  }
  pub fn probe(&mut self)->Option<CandidateSnapshot>{
   self.read_count+=1;
   let (foreground,pid,caret)=current_focus();
   if pid==std::process::id() {self.last_status="应用前台；暂停输入法监控".into();return None}
   if self.uia.get_focused_element().ok().and_then(|e|e.is_password().ok()).unwrap_or(false){
    self.last_status="密码框已跳过".into();return None
   }
   // 1. Fresh hit test around the caret.
   let mut fast=self.hit_snapshot(caret,true,pid);
   if let Some(snapshot)=&fast {
    if !inside_window(foreground,snapshot) {fast=None}
   }
   // 2. The panel element we already hold (cheap while it is fresh).
   if fast.is_none() {fast=self.panel_from_cache(pid,caret)}
   // 3. The rect that worked a moment ago.
   if fast.is_none() {fast=self.probe_last_bar(pid,caret)}
   // 4. Bridge the redraw gap: one short retry, twice at most.
   if fast.is_none()&&self.last_hit_at.elapsed()<Duration::from_millis(600) {
    thread_sleep(8);
    fast=self.hit_snapshot(caret,false,pid);
   }
   // 5. Window scan, only when we have no frame to hold.
   if fast.is_none()&&self.last_snapshot.is_none()&&Instant::now()>=self.scan_next {
    self.scan_next=Instant::now()+Duration::from_millis(300);
    if let Some(window)=window_bounds(foreground) {fast=self.scan_window(window,pid,caret)}
   }
   if let Some(snapshot)=fast {
    self.last_hit_at=Instant::now();
    self.misses=0;
    self.last_composition=self.read_composition();
    self.last_bar=Some(snapshot.window_bounds);
    self.last_status=format!("已识别：{}",snapshot.source);
    self.last_snapshot=Some(snapshot.clone());
    return Some(snapshot)
   }
   self.misses+=1;
   // Hold the last frame while the bar is merely repainting: a recent hit or a
   // list item still sitting under the cell we reported.
   if let Some(snapshot)=&self.last_snapshot {
    // The cell we reported is still a list item: the bar merely repainted, so
    // keep the frame. If it is not, the word was committed — answer at once
    // instead of waiting for a timeout (that timeout was the hide lag).
    // Alive: the bar repainted. Unknown: the pixel still belongs to the IME,
    // so give the redraw a moment. Gone: the word is committed, answer now.
    let hold=match self.bar_state() {
     BarState::Alive=>true,
     BarState::Unknown=>self.last_hit_at.elapsed()<Duration::from_millis(120),
     BarState::Gone=>false,
    };
    if hold {
     self.last_status="候选保持中（重绘间隙）".into();
     return Some(snapshot.clone())
    }
   }
   self.last_snapshot=None;
   self.last_status="候选已收回".into();
   None
  }
 }
 fn inside_window(foreground:isize,snapshot:&CandidateSnapshot)->bool{
  if foreground==0 {return true}
  let Some(window)=window_bounds(foreground) else {return true};
  if window.width()<=0||window.height()<=0 {return true}
  let b=snapshot.window_bounds;
  b.width()>0&&b.left>=window.left-160&&b.left<=window.right+160
   &&b.top>=window.top-160&&b.top<=window.bottom+160
 }
 fn thread_sleep(ms:u64){std::thread::sleep(Duration::from_millis(ms))}
}
#[cfg(windows)] pub use win::WindowsReader;
#[cfg(not(windows))] pub use win::BarState;
#[cfg(not(windows))]
#[derive(Debug)]
pub struct WindowsReader;
#[cfg(not(windows))] impl WindowsReader{
 pub fn new()->Result<Self,String>{Ok(Self)}
 pub fn status(&self)->String{"IME watcher runs only on Windows".into()}
 pub fn has_panel(&self)->bool{false}
 pub fn panel_alive(&self)->bool{false}
 pub fn probe(&mut self)->Option<CandidateSnapshot>{None}
}

#[cfg(test)] mod tests{use super::*;
 #[test]fn parses_ime_numbers(){assert_eq!(normalize_candidate("1.你好").unwrap(),"你好");assert!(normalize_candidate("\n").is_none());}
 #[test]fn geometry_flip(){let mon=Rect{left:0,top:0,right:1920,bottom:1080};let c=Rect{left:1800,top:980,right:1880,bottom:1010};assert_eq!(hud_placement(c,mon,430,100,8),(1490,872));}
}