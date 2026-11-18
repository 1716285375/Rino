# Third-party components and source attribution

## Glint

Original ideas and reference architecture: **jsogn/Glint**, https://github.com/jsogn/Glint, MIT License © 2026 Jhon_Walker. RINO uses a new Windows Rust implementation; it does not include original Swift source files.

## Sakuga VFX

Original Web Components: **Justin Finkelman**, https://github.com/finktheartist/sakuga-vfx, MIT License © 2026 Justin Finkelman.
Pinned source commit: `c3dabb173928a3ad2372183103caf2e6f50953d0`.

The build script downloads the following UNMODIFIED JavaScript files from the upstream repository: `button`, `toggle`, `tabs`, `checkbox`, `input`, `slider`, `toast`. After syncing, the original LICENSE appears in `public/sakuga-vfx/LICENSE`. **Sakuga is not part of the ZIP source archive until first sync**; thus an Internet connection is required the first time you run `npm run dev` or `npm run build`.

MIT grants use, modify, distribute, sublicense, and sell with attribution/license preservation. The original code, images and effects remain credited to the original maintainer.

## CC-CEDICT

Chinese-English dictionary conversion derived from **CC-CEDICT**, https://cc-cedict.org/, under Creative Commons Attribution-ShareAlike 4.0 International: https://creativecommons.org/licenses/by-sa/4.0/. This dictionary database is NOT relicensed MIT. See `src-tauri/resources/CEDICT-NOTICE.txt`.

## ECDICT

Dictionary resource derived from ECDICT, https://github.com/skywind3000/ECDICT, governed by its own resource license; the bundled transformed source notice is `src-tauri/resources/ECDICT-LICENSE.txt`. Check upstream provenance before redistributing further modifications.

## Other runtime dependencies

Rust crates and npm dependencies each retain their own license, as published in their distributions. Build with `cargo license` and npm license tools before a binary release. Models (Ollama, Hy-MT2-1.8B, Index-Translate-2B, TranslateGemma-4B) **are not bundled**; their weights, licenses, redistribution rules and model templates are independently governed.
