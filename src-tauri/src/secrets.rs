// Keys never appear in settings, front-end state, logs or diagnostic bundles.
pub fn set_key(profile_id:&str,key:&str)->Result<(),String>{
 if !profile_id.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'){return Err("invalid profile id".into())}
 #[cfg(windows)]{
   let entry=keyring::Entry::new("dev.rino.linguistic",profile_id).map_err(|e|e.to_string())?;
   if key.trim().is_empty(){entry.delete_credential().map_err(|e|e.to_string())?;}else{entry.set_password(key).map_err(|e|e.to_string())?;}Ok(())
 }
 #[cfg(not(windows))]{let _=key;Err("系统密钥存储仅在 Windows 正式版启用".into())}
}
pub fn get_key(profile_id:&str)->Option<String>{
 #[cfg(windows)]{keyring::Entry::new("dev.rino.linguistic",profile_id).ok()?.get_password().ok()}
 #[cfg(not(windows))]{let _=profile_id;None}
}
pub fn has_key(profile_id:&str)->bool {get_key(profile_id).map(|s|!s.is_empty()).unwrap_or(false)}
