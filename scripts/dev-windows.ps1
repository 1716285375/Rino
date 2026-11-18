$ErrorActionPreference='Stop'
# The project lives on an exFAT volume, which cannot hard-link cargo's
# incremental cache; cargo falls back to copying and prints a warning on every
# build. Plain (non-incremental) builds are also faster there.
if (-not $env:CARGO_INCREMENTAL) { $env:CARGO_INCREMENTAL = '0' }
# rustup installs to %USERPROFILE%\.cargo\bin, which is not always on PATH for
# non-interactive shells; make the script self-sufficient.
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ((Test-Path $cargoBin) -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  $env:PATH = "$cargoBin;$env:PATH"
}
if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) { throw '请先安装 Rust stable MSVC toolchain' }
if (-not (Get-Command npm -ErrorAction SilentlyContinue)) { throw '请先安装 Node.js 22+' }
npm install
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
npm run tauri dev