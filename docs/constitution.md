# Constitution

> Normativ und bindend. Jedes Prinzip ist konkret und überprüfbar. Rund eine
> Seite; diese Datei ist über CLAUDE.md dauerhaft im Kontext.

## Tech stack

| Bereich | Wahl | Begründung |
| ------- | ---- | ---------- |
| Kernsprache | Rust, Edition 2024, MSRV in `rust-toolchain.toml` gepinnt | Echtzeit-Audiopfad ohne GC-Pausen; die tragenden Bindings sind Rust; deterministisches Nullen von Puffern ist nur so zusicherbar |
| Desktop-Shell | Tauri 2 | Von beiden führenden Projekten im Feld unabhängig gewählt; kleines Paket, Rust-Backend, keine Node-Runtime im Auslieferungsstand |
| Frontend | React 19 + Vite + TypeScript (strict) | Größte Contributor-Basis im Tauri-Umfeld; ein Open-Source-Projekt lebt von externen Beiträgen |
| Audio Windows | `wasapi-rs` (MIT) über WASAPI Process Loopback | Einzige API, die Per-Prozess-Audio ohne virtuellen Audiotreiber liefert |
| Audio macOS / Linux | Core Audio Process Taps / PipeWire, hinter `AudioSource` | Später implementiert, der Trait existiert ab Tag 1 |
| ASR | `whisper-rs` (whisper.cpp) mit CUDA-/Vulkan-/CPU-Ladder | Reifere GPU-Backends und bessere deutsche Qualität — trägt die Kriterien WER ≤ 15 % und 8 GB VRAM |
| VAD, Segmentierung, Embeddings, Clustering | `sherpa-onnx` (Apache-2.0) + pyannote-segmentation-3.0-ONNX | Diarization ohne Python-Laufzeit, statisch linkbar; overlap-aware Segmentierung ist der größte DER-Hebel |
| Protokoll-Speicher | Markdown + JSON-Sidecar in einem Nutzerordner, keine Datenbank | Sichtbar und löschbar ohne Werkzeug; "keine versteckte Datenbank" ist Teil des Produktversprechens |
| Projektlizenz | Apache-2.0 | Patentklausel bei Audio-APIs relevant; kompatibel mit sherpa-onnx (Apache-2.0) und wasapi-rs (MIT) |
| Lizenz-Gate | `cargo-deny` mit Allowlist | GPL-/AGPL-Abhängigkeiten maschinell ausgeschlossen — die PipeWire-Referenz ist GPL-2.0 und darf nur gelesen werden |
| CI | GitHub Actions auf `windows-latest` | Zielplattform zuerst; macOS- und Linux-Runner kommen mit den jeweiligen Backends |

## Architecture principles

- `#![forbid(unsafe_code)]` in jedem Crate außer den Plattform-Backends
  (`audio-win`, später `audio-mac`, `audio-linux`).
- PCM- und Embedding-Typen implementieren kein `serde::Serialize` und kein `Debug`,
  das Samples ausgibt. Persistenz ist damit ein Compile-Fehler, keine Review-Frage.
- Die Crates `audio`, `audio-win`, `asr` und `diarize` besitzen keinen Schreibpfad
  und keinen HTTP-Client. Das Lesen von Modelldateien ist erlaubt (whisper.cpp und
  sherpa-onnx laden intern über Pfade). Erzwungen durch den Test
  `tests/no_write_paths.rs`, der die Quellen dieser Crates gegen eine
  Symbol-Blockliste prüft: `fs::write`, `fs::File::create`, `OpenOptions::write`,
  `reqwest`, `ureq`. Nur `protocol` schreibt, nur `provisioning` greift aufs Netz.
- `protocol` hängt nicht von `audio` ab. Damit ist "der Schreiber kennt PCM nicht"
  eine Eigenschaft des Abhängigkeitsgraphen, nicht eine Absichtserklärung.
- Der Audio-Ringpuffer ist fest dimensioniert (≤ 30 s je Strom) und wird bei
  Überlauf überschrieben, nie vergrößert — der Speicherverbrauch ist unabhängig von
  der Call-Dauer konstant.
- Sprecher-Embeddings liegen in `Zeroizing`-Puffern und werden bei Sitzungsende
  explizit genullt.
- Der Aufnahmestart ist typseitig an die Attestation gebunden:
  `Session::start(consent: ConsentAttestation)`. Es existiert kein Konstruktor ohne
  sie.
- Plattformcode nur hinter dem `AudioSource`-Trait; kein `#[cfg(windows)]` außerhalb
  der Backend-Crates.
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul (clippy
  `too_many_lines` als Fehler).
- Kein Netzwerkzugriff außer dem Modell-Download aus einer versionierten Allowlist
  mit SHA-256-Prüfung. Jede weitere URL im Code ist ein Merge-Blocker.
- Kein `unwrap()` und kein `expect()` außerhalb von Tests und `main` (clippy
  `unwrap_used` als Fehler); Fehler typisiert über `thiserror`.

## Conventions

- Code, Identifier, Commit-Messages, Issues und Kommentare auf Englisch. Die
  Dokumente unter `docs/` auf Deutsch. UI-Texte zweisprachig Deutsch und Englisch.
- Crates heißen `transcriber-<concern>` und liegen unter `crates/<concern>`.
- Keine `utils`-, `common`- oder `helpers`-Module. Ein Modul trägt einen Concern
  im Namen.
- Konfiguration als TOML-Datei im Nutzerprofil. Keine Registry-Schreibzugriffe.
- Modelle liegen nicht im Repository und nicht im Installer, sondern werden beim
  ersten Start geladen.

## Quality gates

- Das Verify-Kommando aus `docs/workflow.md` läuft grün: `cargo fmt --check`,
  `cargo clippy -D warnings`, `cargo test`, `cargo deny check` — ab Phase 5
  zusätzlich Frontend-Typecheck und -Lint.
- Bei jeder Änderung am Datenpfad läuft zusätzlich der FS-Audit-Test: eine Sitzung
  hinterlässt kein Audio-Artefakt und kein Embedding.
- Jede neue Abhängigkeit braucht einen Eintrag in der `cargo-deny`-Allowlist,
  sonst blockiert der Merge.
- Review durch einen frischen Agent, der den Code nicht geschrieben hat.

## Don'ts

- Kein HTTP-Client in `audio`, `audio-win`, `asr`, `diarize` oder `protocol`.
- Keine Python-Laufzeit und kein PyTorch im Auslieferungspaket.
- Kein `Serialize` für Audio- oder Embedding-Typen.
- Kein Kopieren von GPL-/AGPL-Code; `obs-pipewire-audio-capture` ist ausschließlich
  Lesestoff.
- Keine Funktion, die die Anwendung verbirgt, aus der Prozessliste entfernt oder
  Aufnahme-Indikatoren des Call-Clients unterdrückt.
- Keine Telemetrie, kein externes Crash-Reporting, kein Auto-Update, der Inhalte
  überträgt.
- Kein Speichern von Roh-Audio, auch nicht temporär, auch nicht "nur zum Debuggen".
  Für Debugging existiert ein synthetischer Testton-Pfad.
