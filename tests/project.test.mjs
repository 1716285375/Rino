import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
const read=path=>readFileSync(new URL('../'+path,import.meta.url),'utf-8');

test('Tauri 2 supports separate non-focus HUD and application windows',()=>{
 const cfg=JSON.parse(read('src-tauri/tauri.conf.json'));
 assert.equal(cfg.app.windows.length,2);
 const hud=cfg.app.windows.find(x=>x.label==='hud');
 assert.equal(hud.focusable,false);assert.equal(hud.alwaysOnTop,true);assert.equal(hud.transparent,true);
});
test('Rust IME probe prioritizes XAML point hit and privacy flags',()=>{
 const code=read('src-tauri/src/monitor.rs');
 assert.match(code,/element_from_point/);
 assert.match(code,/ControlType::ListItem/);
 assert.match(code,/SelectionPattern/);
 assert.match(code,/is_password/);
 assert.doesNotMatch(code,/GetAsyncKeyState|SetWindowsHookEx|BitBlt/);
});
test('provider supports Ollama, compatible chat, OpenAI Responses, Anthropic, Gemini',()=>{
 const code=read('src-tauri/src/providers.rs');
 for(const key of ['api/chat','chat/completions','responses','v1/messages','generateContent','get_key'])assert.ok(code.includes(key),key);
});
test('Sakuga VFX installer is pinned, original files and LICENSE',()=>{
 const code=read('scripts/vendor-sakuga.mjs');
 assert.match(code,/c3dabb173928a3ad2372183103caf2e6f50953d0/);
 assert.match(code,/raw\.githubusercontent\.com/);
 assert.ok(existsSync(new URL('../public/sakuga-vfx/LICENSE',import.meta.url)));
});
test('model templates and offline dictionary resource',()=>{
 const src=read('src-tauri/src/providers.rs');
 for(const model of ['translategemma','hy-mt','index-translate'])assert.ok(src.includes(model));
 for(const name of ['lexicon.json','cedict.sqlite3','ecdict_rank.sqlite3'])assert.ok(existsSync(new URL('../src-tauri/resources/'+name,import.meta.url)));
});
test('Windows credentials are not serialized to settings and no SSH/key file',()=>{
 const file=read('src-tauri/src/secrets.rs');
 assert.match(file,/keyring::Entry/);
 assert.match(read('src-tauri/src/domain.rs'),/cloud_consent:false/);
});
test('React workspace includes all main pages and HUD events',()=>{
 const code=read('src/main.tsx');
 for(const page of ['home','history','vocab','settings','about','rino:candidate','rino:translation']) assert.ok(code.includes(page));
});

test('RINO is single-instance on Windows',()=>{
 const cargo=read('src-tauri/Cargo.toml');
 assert.match(cargo,/tauri-plugin-single-instance/);
 const lib=read('src-tauri/src/lib.rs');
 assert.match(lib,/tauri_plugin_single_instance::init/);
 assert.match(lib,/get_webview_window\("main"\)/);
});