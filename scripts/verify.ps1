$ErrorActionPreference='Stop'
Write-Host '[1/5] TypeScript + Vite'
npm run check
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host '[2/5] Node tests'
npm test
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host '[3/5] Rust formatting'
cargo fmt --manifest-path src-tauri/Cargo.toml --all
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host '[4/5] Rust check'
cargo check --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host '[5/5] Rust tests'
cargo test --manifest-path src-tauri/Cargo.toml
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'All Windows source verification steps passed.'
