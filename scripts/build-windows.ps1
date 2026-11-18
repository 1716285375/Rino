$ErrorActionPreference='Stop'
# The project lives on an exFAT volume, which cannot hard-link cargo's
# incremental cache; cargo falls back to copying and prints a warning on every
# build. Plain (non-incremental) builds are also faster there.
if (-not $env:CARGO_INCREMENTAL) { $env:CARGO_INCREMENTAL = '0' }
npm install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run tauri build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'Installer: src-tauri\target\release\bundle\nsis\'
