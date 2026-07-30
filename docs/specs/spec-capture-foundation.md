# Spec: Phase 1 — Capture-Fundament Windows

> Created: 2026-07-30

Diese Spec liefert das Fundament der Erfassung: die Tonspur **einer** gewählten
Anwendung und das eigene Mikrofon als zwei strukturell getrennte Ströme, einen fest
dimensionierten Ringpuffer, eine Sitzungs-Zustandsmaschine, deren Start typseitig an
die Consent-Attestation gebunden ist, und ein CLI-Harness, mit dem beides ohne
Oberfläche nachweisbar ist. Kein Audio verlässt den Arbeitsspeicher.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] `list-sources` zeigt die Anwendungen mit **aktivem** Render-Stream, je mit
      Prozessnamen und der **Prozessbaum-Wurzel-PID**, die erfasst würde.
- [ ] `capture` erfasst genau die gewählte Anwendung: parallel in einer **anderen**
      Anwendung abgespielte Musik erscheint messbar nicht im erfassten Strom
      (Pegel bleibt auf Stille-Niveau).
- [ ] Mikrofon und Anwendungston liegen als zwei Ströme mit den Identitäten `Local`
      und `Remote` vor; kein Codepfad mischt sie.
- [ ] Es existiert kein Weg, eine Erfassung ohne `ConsentAttestation` zu starten —
      belegt durch einen `compile_fail`-Fall, nicht nur durch ein Review-Urteil.
- [ ] Ein Sitzungslauf hinterlässt **0 Bytes** neuer Dateien — maschinell belegt über
      die Testton-Quelle, dazu der Quell- und Manifest-Audit über die
      Erfassungs-Crates.
- [ ] Der Speicherverbrauch ist unabhängig von der Laufzeit konstant: die Kapazität
      des Ringpuffers verändert sich nie, ein zurückgefallener Leser meldet Verlust.
- [ ] `cargo xtask verify` läuft lokal grün.
- [ ] Die vier Widersprüche zwischen `docs/architecture.md`, `docs/constitution.md`
      und dem Code sind in den Foundation-Docs aufgelöst (siehe In scope).

## Scope

### In scope

- Crate `core`: `StreamIdentity` (`Local` / `Remote`), `CaptureSubject`,
  `ConsentAttestation`, der versionierte Attestation-Text (DE + EN), `SessionState`,
  `SessionId`, typisierte Fehler. **Nicht** `Session` selbst — siehe die
  Foundation-Doc-Korrektur unten.
- Crate `audio`: `AudioSource`- und `SourceFactory`-Trait, PCM-Frame- und
  Formattypen mit Zeitachse, fest dimensionierter Ringpuffer mit Leser-Cursorn,
  Downmix und Resampling auf 16 kHz mono, synthetische Testton-Quelle samt
  Testton-`SourceFactory`.
- Crate `audio-win`: `SourceFactory`-Implementierung — Aufzählung der Anwendungen mit
  aktivem Render-Stream, Auflösung auf die Prozessbaum-Wurzel,
  Prozessbaum-Loopback-Erfassung, Mikrofon-Erfassung mit betriebssystemseitiger
  Echokompensation.
- Crate `session`: `Session` als Zustandsmaschine und Orchestrator — typseitiges
  Consent-Gate, Event-Bus, Lebenszyklus samt Nullen der Puffer am Ende. Kennt keine
  Plattform, nur die Traits aus `audio`.
- Crate `cli`: das Harness für die Phasen 1–4 und der Composition Root —
  `list-sources` und `capture` mit Consent-Abfrage und Pro-Strom-Statistik.
- Zwei Audit-Tests, beide in Verify: der Quell- und Manifest-Audit über die
  Erfassungs-Crates, und ein **gerätefreier Sitzungs-FS-Audit** (Testton-Quelle,
  Verzeichnis-Snapshot vor und nach einer Sitzung, 0 neue Dateien).
- **Foundation-Doc-Korrekturen** — vier belegte Widersprüche, die diese Phase
  auflöst, statt sie auf `main` stehen zu lassen:
  1. `docs/architecture.md` weist `Session` der Crate `core` zu, beschreibt den
     Orchestrator aber in derselben Tabelle als `session`. `core` bekommt
     `SessionState` / `SessionId`, `session` bekommt `Session`.
  2. `docs/architecture.md` sagt für `core` „keine externen Abhängigkeiten",
     `docs/constitution.md` fordert Fehler „typisiert über `thiserror`". Präzisiert
     zu: keine I/O- und keine Plattform-Abhängigkeiten, `thiserror` erlaubt (ab
     Phase 3 zusätzlich `zeroize`).
  3. `docs/architecture.md` nennt die Strom-Identität als Verantwortung von `audio`,
     `protocol` darf laut Boundaries aber nur `core` kennen und braucht sie im
     Segment. `StreamIdentity` gehört nach `core`.
  4. Der Doc-Kommentar in `crates/core/src/lib.rs` kündigt `Session` für `core` an
     und wird mit Korrektur 1 mitgezogen.
- Fundament-Nachzug aus Phase 0: Edition 2024, in `rust-toolchain.toml` gepinnte
  MSRV, GitHub-Actions-Workflow auf `windows-latest` — abhängig von der offenen
  Scope-Entscheidung unten.

### Out of scope

- Spracherkennung, VAD, Segmentierung, Embeddings, Clustering — Phasen 2 und 3.
- Protokoll-Ausgabe und der vollständige FS-Audit über eine **echte** Sitzung mit
  Embeddings — Phase 4. Phase 1 liefert die gerätefreie Vorstufe (siehe In scope).
- Der Fan-out **eines** Ringpuffers an **zwei** Konsumenten (ASR und VAD). Phase 1
  baut und testet die Leser-Cursor-API, die den Fan-out später rein additiv macht,
  und nutzt produktiv einen Leser.
- Oberfläche, Installer, Modellbereitstellung — Phasen 5 und 6.
- macOS- und Linux-Backends — Phasen 8 und 9. `AudioSource` und `SourceFactory`
  existieren, die Implementierungen nicht. Auch die **Plattform-Auswahl** ist noch
  nicht nötig: `cli` bindet in Phase 1 unbedingt `audio-win`; das cfg-freie
  Auswahl-Crate entsteht mit dem zweiten Backend (`docs/architecture.md`, Where new
  code goes).
- Auswahl des Mikrofon-Geräts durch den Nutzer. Phase 1 nimmt das
  Kommunikations-Standardgerät; eine Auswahl kommt mit den Einstellungen in Phase 5.
- Akustische Qualitätsbewertung des Tons. Erst Phase 2 misst etwas (WER).

## Constraints

- `#![forbid(unsafe_code)]` in **allen** Crates dieser Phase, `audio-win`
  eingeschlossen. `docs/constitution.md` erlaubt dem Backend die Ausnahme, aber jeder
  am 2026-07-30 geprüfte `wasapi`-Aufruf ist eine *safe* Funktion — die Ausnahme wird
  erst gezogen, wenn ein konkreter Fall sie erzwingt. Solange sie nicht gezogen ist,
  opted `audio-win` regulär in `[lints] workspace = true` ein.
  Wird sie gezogen: `[lints] workspace = true` lässt sich in Cargo **nicht** mit
  eigenen Lint-Tabellen kombinieren und `forbid` innen nicht per `allow`
  zurücknehmen — `audio-win` muss dann `too_many_lines`, `unwrap_used`,
  `expect_used` und `missing_docs` **vollständig duplizieren** und nur
  `unsafe_code` auslassen, sonst fallen die Gates im riskantesten Crate lautlos weg.
  Jeder `unsafe`-Block trägt dann einen `// SAFETY:`-Kommentar.
- PCM- und Format-Typen tragen kein `serde::Serialize` und kein `Debug`, das Samples
  ausgibt. `audio` und `audio-win` haben keinen Schreibpfad und keinen HTTP-Client.
- Der Ringpuffer ist fest dimensioniert und wird bei Überlauf überschrieben, nie
  vergrößert. Kapazität: **30 s je Strom** — die Obergrenze der Constitution, also
  bei 48 kHz stereo f32 rund **11,5 MB je Strom** und 23 MB für beide.
- Abhängigkeitsrichtung nach `docs/architecture.md`: `core` ← `audio` ← `audio-win`;
  `session` → `core` + `audio`, **nicht** `audio-win`. Kein `#[cfg(windows)]`
  außerhalb von `audio-win`.
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul; kein `unwrap()` und
  kein `expect()` außer in Tests und `main`.
- Neue Abhängigkeiten müssen unter die `cargo-deny`-Allowlist fallen. Für Phase 1
  vorgesehen: `wasapi` (MIT, auf `0.23.x` gepinnt), `sysinfo` (MIT), `rubato` (MIT),
  `zeroize` (MIT/Apache-2.0), `thiserror` (MIT/Apache-2.0), `tokio` (MIT, nur Feature
  `sync`), `clap` (MIT/Apache-2.0), `trybuild` (MIT/Apache-2.0, nur `dev`). Alle
  Lizenzen stehen bereits in `deny.toml`.
- Die GitHub-`windows-latest`-Runner haben **kein** Audiogerät. Alles, was in CI
  geprüft wird, muss ohne Gerät laufen. Der Workflow muss außerdem `cargo-deny`
  installieren — `cargo xtask verify` ruft `cargo deny check`, und auf einem nackten
  Runner ist es nicht vorhanden.

## Prior art

- [Per-process audio capture on Windows (Phase 1)](../prior-art.md#per-process-audio-capture-on-windows-phase-1)
  — die tragende Fähigkeit der ganzen Anwendung. Am 2026-07-30 gegen `wasapi-rs`
  v0.23.0 im Quelltext geprüft, Ergebnisse in den Prior decisions unten.
- [Legal and consent framing (Phase 1)](../prior-art.md#legal-and-consent-framing-phase-1)
  — begründet, warum das Consent-Gate **vor** dem Start greift und warum die
  Attestation mit Zeitstempel dokumentiert wird: die Erfassung ist die Tat, nicht
  das Protokoll.
- [Produktgestalt und Desktop-Stack (Phase 5)](../prior-art.md#produktgestalt-und-desktop-stack-phase-5)
  — das AVOID „Mikrofon + **vollständiges** System-Audio mischen" ist genau die
  Grenze, die diese Phase zieht: minimiert wird beim Abgriff.

## Human prerequisites

- [ ] Windows-11-Maschine mit funktionierendem Mikrofon **und** Lautsprechern
      (nicht nur Headset) — die Echokompensation ist nur mit Lautsprechern prüfbar.
- [ ] Die Windows-Datenschutzeinstellung „Apps dürfen auf das Mikrofon zugreifen"
      ist für Desktop-Anwendungen aktiv. Ist sie aus, scheitert die
      Mikrofon-Erfassung mitten in der Implementierung.
- [ ] Ein installierter Call-Client für den Smoke-Test am QA-Gate (Teams, Zoom oder
      ein Browser-Meeting) und ein zweiter Ton-Erzeuger (Musik-App oder zweiter
      Browser) für den Isolations-Testfall. Bitte am Gate benennen, welcher Client
      es ist — die Prozessbaum-Auflösung wird gegen ihn verifiziert.
- [ ] Ein **zweiter Gesprächsteilnehmer** oder ein zweites Gerät im Call. Ohne eine
      sprechende Gegenseite sind der Pegel-Nachweis auf `Remote` und der komplette
      AEC-Testfall am QA-Gate nicht durchführbar.
- [ ] Rust-Toolchain und `cargo-deny` lokal vorhanden. CMake und die
      Visual-Studio-C++-Build-Tools aus `docs/workflow.md` werden erst ab Phase 2
      gebraucht — Bootstrap ist in Phase 1 nur `cargo fetch --locked`.
- [ ] Keine Secrets, keine Accounts, keine externe Provisionierung. GitHub Actions
      ist auf diesem öffentlichen Repo kostenfrei — nichts zu hinterlegen.

## Prior decisions

### Sitzung, Zustände und Nahtstellen

| Decision | Rationale | Date |
|---|---|---|
| `Session` liegt in `session`, nicht in `core`. Signatur: `Session::start(consent: ConsentAttestation, plan: CapturePlan) -> Result<Session, SessionError>`, wobei `CapturePlan` das gewählte `CaptureSubject` und die geöffneten `Box<dyn AudioSource>` trägt | Die Constitution schreibt `Session::start(consent: ConsentAttestation)` als Consent-Gate fest, sagt aber nichts über die übrigen Argumente. `core` ist laut `docs/architecture.md` I/O-frei — ein Orchestrator, der Capture-Threads besitzt, ist dort fehl am Platz. Der Consent bleibt das **erste** Argument und ohne ihn existiert kein Konstruktor | 2026-07-30 |
| Die Quellen werden über einen `SourceFactory`-Trait in `audio` geöffnet; `audio-win` liefert `WindowsSources`, `audio` liefert `TestToneSources`. `cli` ist der Composition Root und wählt die Implementierung | Hält `session` plattformfrei (kein `cfg` außerhalb des Backends) und macht die Testton-Quelle injizierbar — sonst ist keiner der `session`-Tests ohne Audiogerät lauffähig. `app` in Phase 5 benutzt dieselbe Fabrik, damit CLI und Oberfläche austauschbar bleiben | 2026-07-30 |
| Zustände: `Idle → Capturing → Stopping → Ended`. Der einzige Übergang nach `Capturing` ist `Session::start` mit Attestation; `Ended` ist terminal, eine Sitzung wird nicht neu gestartet. Ein fehlendes Mikrofon ist ein **Attribut** der laufenden Sitzung (`local_stream: Option<…>`), kein Zustand | Vier Zustände und drei Kanten sind die vollständige Mechanik des Crates; sie gehören in die Spec, damit der Implementierer sie nicht erfindet. Mikrofon-Abwesenheit als Zustand würde die Kanten verdoppeln, ohne etwas zu unterscheiden | 2026-07-30 |
| Ein fehlendes Mikrofon ist eine **Warnung**, kein Abbruch: die Sitzung läuft mit dem `Remote`-Strom allein weiter | Wer nur zuhört, soll ein Protokoll bekommen. Der `Local`-Strom fehlt dann sichtbar, statt die Sitzung zu verhindern | 2026-07-30 |
| Der Event-Bus nutzt `tokio::sync::broadcast` (Feature `sync`, **keine** Runtime) | `docs/architecture.md` legt tokio für die Orchestrierung fest. `broadcast::Sender::send` funktioniert ohne laufende Runtime, also bleibt der Audiopfad synchron und Phase 5 kann async konsumieren, ohne den Bus zu ersetzen | 2026-07-30 |
| Der Attestation-Text liegt als **versionierte Konstante in `core`** (DE + EN), nicht in `cli` oder im Frontend | `docs/design.md` fordert den Volltext im Dialog; `protocol` muss festhalten, welche Fassung attestiert wurde. Eine Quelle für CLI, künftige Oberfläche und Protokollkopf | 2026-07-30 |
| `core` darf `thiserror` verwenden (ab Phase 3 zusätzlich `zeroize`), sonst keine externen Abhängigkeiten. Zeitstempel als `std::time::SystemTime` | Auflösung des Widerspruchs zwischen `docs/architecture.md` und der Constitution — die Constitution ist normativ und bindend, `architecture.md` ist erklärtes lebendes Dokument. Kein Datums-Formatierer in `core`: ISO-8601 ist eine `protocol`-Sache | 2026-07-30 |

### Ringpuffer, Zeitachse und Resampling

| Decision | Rationale | Date |
|---|---|---|
| Kapazität **30 s je Strom**, als benannte Konstante, beim Bau gegen die Obergrenze geprüft | Die Constitution nennt ≤ 30 s; der konkrete Wert fehlte und würde sonst erfunden. 30 s maximiert die Lag-Toleranz für die ASR-Fenster aus Phase 2 und kostet 11,5 MB je Strom | 2026-07-30 |
| Der Ringpuffer vergibt **je Leser einen Cursor**; ein zurückgefallener Leser verliert Samples und erhält die Verlustzahl gemeldet, statt den Puffer wachsen zu lassen | Konstanter Speicherverbrauch ist eine Zusage der Constitution. Die Cursor-Form macht den Fan-out an ASR und VAD in Phase 2/3 rein additiv, ohne den Puffer neu zu schreiben | 2026-07-30 |
| Jeder Strom trägt seine **eigene Zeitachse** aus `BufferInfo.timestamp` (Geräteposition, 100-ns-Einheiten), normalisiert auf eine gemeinsame Sitzungs-Null, die `Session::start` festhält | Mikrofon und virtuelles Loopback-Gerät haben unabhängige Uhren. `docs/architecture.md` legt die Sprecherlabels „über Zeitüberlappung" auf die Segmente — ohne eine gemeinsame Null ist diese Überlappung in Phase 3 nicht berechenbar. Wanduhr bei Ankunft wäre durch Puffer-Latenz verfälscht | 2026-07-30 |
| `BufferFlags::data_discontinuity` erzeugt eine **explizite Lücke** auf der Zeitachse (Ereignis mit Zähler), `BufferFlags::silent` wird als Nullen materialisiert, damit die Achse dicht bleibt | Die Discontinuity-Flagge ist der eigentliche Lücken-Melder von WASAPI; wird sie ignoriert, verschiebt sich die Zeitachse still gegen die andere und die Sprecherzuordnung in Phase 3 driftet. Der Event-Timeout unten ist ein anderer Fall | 2026-07-30 |
| Resampling auf 16 kHz mono passiert **auf der Leseseite in `audio`** (`rubato`), nicht im Backend; der Ringpuffer hält das native Format | `docs/architecture.md`, Flow 2 („Ringpuffer → Resampling → Fan-out"). Hält das Resampling in einem `forbid(unsafe_code)`-Crate, wo es ohne Audiogerät testbar ist | 2026-07-30 |
| PCM-Puffer werden bei Sitzungsende **explizit genullt** (`zeroize`), nicht nur freigegeben | Die Constitution fordert das Nullen für Embeddings; für PCM ist es gleich billig und deckt die Zusage „nichts Audio-förmiges überlebt" auch im Arbeitsspeicher ab | 2026-07-30 |

### Windows-Backend — gegen `wasapi-rs` v0.23.0 geprüft

| Decision | Rationale | Date |
|---|---|---|
| Loopback über `AudioClient::new_application_loopback_client(root_pid, include_tree = true)` | Am Quelltext geprüft: die Fähigkeit existiert als sichere Rust-API, `include_tree` bildet auf `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` ab. Kein eigener FFI-Code nötig | 2026-07-30 |
| Beide Clients bekommen **explizit** 48 kHz, stereo, 32-bit Float, aber **getrennte** Modus-Parameter: Loopback `EventsShared { autoconvert: true, buffer_duration_hns: 0 }`, Mikrofon `EventsShared { autoconvert: true, buffer_duration_hns: min_time }` aus `get_device_period()` | `GetMixFormat` liefert auf einem Process-Loopback-Client `E_NOTIMPL`, das Format muss also gesetzt werden. `get_device_period` funktioniert auf demselben Client ebenfalls nicht — daher 0 dort und der echte Wert nur auf dem Mikrofon. Ein gemeinsames Format hält Resampler-Konfiguration und Puffergröße für beide Ströme identisch | 2026-07-30 |
| Die Chunk-Größe kommt **ausschließlich** aus `get_next_packet_size()`; `get_buffer_size()` wird auf dem Loopback-Client nicht aufgerufen | `wasapi-rs` dokumentiert, dass `get_buffer_size` dort fehlerfrei absurde Werte liefert (Größenordnung 3·10⁹). Das Referenzbeispiel umgeht das mit einer **unbegrenzt reallokierenden** `VecDeque` — genau das Gegenteil von „fest dimensioniert, nie vergrößert". Wer das Beispiel abschreibt, baut den Verstoß mit ein | 2026-07-30 |
| `get_audiosessioncontrol()` steht auf dem Loopback-Client nicht zur Verfügung; das Strom-Ende wird dort am Lesefehler erkannt | Ebenfalls in der Limitationsliste der Crate dokumentiert. Ohne diese Festlegung sucht der Implementierer einen Kanal, den es nicht gibt | 2026-07-30 |
| Ein Timeout beim Warten auf das Capture-Event ist **Stille, kein Fehler** — er erhöht einen Zähler und die Erfassung läuft weiter. Nur ein invalidiertes Gerät beendet den Strom | Eine ausgewählte Anwendung, die gerade nichts abspielt, ist der Normalfall. Beide Referenzbeispiele behandeln den Timeout als fatal; für uns wäre das ein Abbruch bei jeder Gesprächspause | 2026-07-30 |
| Die Anwendung wird auf ihre **Prozessbaum-Wurzel** aufgelöst, aber nur bis zu einer Stop-Liste (`explorer.exe`, `services.exe`, `svchost.exe`, `wininit.exe`, PID 0 und 4) | Das Referenzbeispiel warnt ausdrücklich: für Baum-Erfassung muss die **Eltern**-PID das Ziel sein, weil Call-Clients in Kindprozessen rendern. Ungebremstes Hochlaufen würde dagegen die Shell-Wurzel erfassen und die Quellen-Isolation brechen — genau das Kriterium dieser Phase. Die CLI zeigt die aufgelöste Wurzel **vor** der Consent-Abfrage | 2026-07-30 |
| Die Quellenliste kommt aus den **Render**-Audio-Sessions, gefiltert auf `SessionState::Active`, dedupliziert je PID, ohne den eigenen Prozess und ohne PID 0 und 4. Prozessnamen und Elternschaft aus `sysinfo` | `get_audiosessionenumerator` liefert auch `Inactive` und `Expired` — ungefiltert stünden abgelaufene Sessions in der Auswahl. `docs/design.md` fordert die Filterung auf aktive Tonausgabe. `sysinfo` ist MIT und vermeidet weiteren Plattform-Code. Achtung: das Referenzbeispiel zählt `Direction::Capture` auf — wir brauchen `Direction::Render` | 2026-07-30 |
| Zwischen `list-sources` und `capture` wird die Prozess-Identität **neu geprüft** (Name und Startzeit), nicht nur die PID | Windows recycelt PIDs. Ohne die Prüfung könnte `capture` eine andere Anwendung erfassen als die, der der Nutzer zugestimmt hat — ein Consent-Bruch, nicht nur ein Bug | 2026-07-30 |
| Echokompensation auf dem **Mikrofon**-Client: `Role::Communications` als Gerät, `StreamCategory::Communications` per `set_properties` **vor** `initialize_client`, danach `is_aec_supported()` → `get_aec_control()` → `set_echo_cancellation_render_endpoint(Some(render_endpoint_id))`. `initialize_mta()` läuft je Capture-Thread | Ohne AEC landet bei Lautsprecher-Nutzung der Ton der Gegenseite im `Local`-Strom und bricht das Kriterium „ich gegen Gegenseite 100 % korrekt". Der Weg kostet über die geprüfte API rund zehn Zeilen und hat eine eingebaute Fähigkeitsprobe. Die Reihenfolge ist bindend, und die Communications-**Kategorie** ist für einen Loopback-Stream ungültig — nur auf dem Mikrofon setzen | 2026-07-30 |

### Gates und Dokumente

| Decision | Rationale | Date |
|---|---|---|
| Der Quell-Audit liegt in `xtask/tests/no_write_paths.rs` — eine **bewusste Abweichung** von der Constitution, die den Pfad wörtlich als `tests/no_write_paths.rs` nennt | Der Workspace-Root ist ein virtuelles Manifest ohne Package, also kompiliert ein Wurzel-`tests/`-Verzeichnis nicht und `cargo test --workspace` würde den Test nie ausführen. `xtask` ist bereits die Heimat der Workspace-Gates. Die Abweichung wird am Spec-Acceptance-Gate offengelegt, nicht stillschweigend vollzogen | 2026-07-30 |
| Der Audit prüft die Crate-Liste und überspringt noch nicht existierende Crates | Lässt den Test mit den Phasen wachsen, statt bei jeder neuen Phase zu brechen | 2026-07-30 |
| Der Audit prüft **zusätzlich zur** Symbol-Blockliste der Constitution (`fs::write`, `fs::File::create`, `OpenOptions::write`, `reqwest`, `ureq`) auf `Serialize`/`serde` und wertet den **Abhängigkeitsgraphen** der Erfassungs-Crates aus | „Kein `Serialize` für Audio-Typen" ist ein Don't, das sich nicht als negative Trait-Zusicherung ausdrücken lässt. Ein Scan belegt nur die Abwesenheit **benannter** Symbole — die Manifest- und Graph-Prüfung schließt die Lücke, die ein Alias oder ein eigener `impl Write` sonst offen ließe | 2026-07-30 |
| Der Sitzungs-FS-Audit läuft **in Phase 1**, gerätefrei über die Testton-Quelle | Die Constitution sagt „**bei jeder Änderung am Datenpfad** läuft zusätzlich der FS-Audit-Test" — Phase 1 *ist* der Datenpfad. `docs/workflow.md` macht ihn ab Phase 4 verpflichtend, was die frühere Zeile nicht aufhebt. Mit der Testton-Quelle ist er ohne Audiogerät lauffähig, also gibt es keinen Grund zu warten | 2026-07-30 |
| Das Consent-Gate wird per `compile_fail`-Fall (`trybuild`) belegt, nicht per Review-Urteil | Das Vision-Kriterium lautet „die Aufnahme startet **nachweisbar** nie ohne bestätigte Attestation". Ein Review ist eine Momentaufnahme, ein `compile_fail`-Fall ein Dauergate | 2026-07-30 |
| Kein Design-Zyklus (`/loopkit:design`) in dieser Phase | Phase 1 liefert ein CLI-Harness, hat also keine UI-Fläche. Die Zustandsmaschine ist oben mit vier Zuständen und drei Kanten vollständig beschrieben — eine Visualisierung würde keine Entscheidung schärfen. Quellenauswahl und Consent-Dialog sind in `docs/design.md` als Komponenten festgelegt und werden in Phase 5 entworfen | 2026-07-30 |
| OPEN — Wortlaut der Consent-Attestation (DE + EN), die vor jedem Start bestätigt wird | resolved at the spec-acceptance gate | — |
| OPEN — Verhalten, wenn das Mikrofon keine Echokompensation unterstützt: warnen und weiterlaufen, oder Start verweigern, bis Kopfhörer bestätigt sind | resolved at the spec-acceptance gate | — |
| OPEN — gehören die drei Fundament-Nachzüge aus Phase 0 (Edition 2024, gepinnte MSRV, CI-Workflow) in diese Phase? Die Alternative ist **nicht** von der Planung ausführbar: `track:adhoc`-Issues erzeugt laut `docs/workflow.md` der Mensch. Bei „nicht in dieser Phase" entfällt das CI-Outcome hier und der Mensch legt die Issues an | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 1 — Capture-Fundament Windows (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist — eines je
  implementierbarem Schritt

Jedes Issue verweist im Body auf diesen Spec-Pfad.

## Verification

Maschinell, in Verify (und, je nach offener Entscheidung, in CI):

- [ ] `cargo xtask verify` grün — `fmt`, `clippy -D warnings`, `cargo test --workspace`,
      `cargo deny check`.
- [ ] `cargo xtask build` grün.
- [ ] Der Quell- und Manifest-Audit findet in `audio` und `audio-win` keines der
      gelisteten Symbole und keine der gelisteten Abhängigkeiten, und schlägt fehl,
      wenn eines eingeführt wird — einmal durch eine absichtliche Verletzung belegt.
- [ ] Der Sitzungs-FS-Audit läuft eine Sitzung gegen die Testton-Quelle und findet
      im Arbeits- und Temp-Verzeichnis 0 neue Dateien.
- [ ] `compile_fail`-Fall: ein `Session`-Start ohne `ConsentAttestation` kompiliert
      nicht.
- [ ] Ringpuffer-Test gegen die Testton-Quelle: die Kapazität (`capacity()`) bleibt
      über den ganzen Lauf unverändert und es findet keine Reallokation statt; ein
      langsamer Leser meldet die Verlustzahl.
- [ ] Zwei-Cursor-Test: zwei Leser mit unterschiedlichem Tempo lesen denselben
      Puffer, der schnelle verliert nichts, der langsame meldet Verlust — der Beleg
      für die Fan-out-Zusage an Phase 2/3.
- [ ] Resampling-Test: 48 kHz stereo → 16 kHz mono gegen einen synthetischen Sinus
      (Frequenz bleibt, Länge stimmt, kein Aliasing über der Nyquist-Grenze).
- [ ] Zeitachsen-Test: eine injizierte `data_discontinuity` erscheint als Lücke mit
      Zähler; `silent`-Frames erscheinen als Nullen, die Achse bleibt dicht.
- [ ] `session`-Test: `stop()` nullt die Puffer; danach ist kein Sample ungleich Null
      mehr auffindbar.
- [ ] Reine Logiktests in `audio-win` ohne Gerät: Baum-Auflösung gegen die
      Stop-Liste, Session-Filterung (`Active`, Dedup, Ausschlüsse),
      Identitätsprüfung gegen eine recycelte PID.

Manuell am Milestone-QA-Gate (Smoke-Test nach `docs/workflow.md`):

- [ ] `list-sources` listet den laufenden Call-Client mit Prozessnamen und der
      aufgelösten Wurzel-PID; eine Anwendung **ohne offenen Render-Stream**
      erscheint nicht.
- [ ] **Quellen-Isolation:** Call-Client gewählt, in einer anderen Anwendung Musik
      abgespielt → der `Remote`-Strom bleibt auf Stille-Niveau. Danach spricht die
      Gegenseite → der Pegel steigt.
- [ ] **Strom-Trennung:** ins Mikrofon gesprochen → Pegel nur auf `Local`.
- [ ] **Echokompensation:** mit Lautsprechern und aktivem AEC erscheint der Ton der
      Gegenseite nicht über dem Stille-Niveau auf `Local`. Meldet
      `is_aec_supported()` false, erscheint die dokumentierte Warnung (Verhalten nach
      der offenen Entscheidung oben).
- [ ] **Consent-Gate:** Abbruch der Attestation-Abfrage startet keine Erfassung —
      Frame-Zähler beider Ströme bleiben bei 0.
- [ ] **Null Bytes, prozessbezogen:** ein zehnminütiger Lauf wird mit Process Monitor
      gefiltert auf die PID des Harness beobachtet — kein `CreateFile` mit
      Schreibabsicht außerhalb der Konsole. Der maschinelle Sitzungs-FS-Audit oben
      ist das eigentliche Dauergate; ein Snapshot über das ganze Nutzerprofil wäre
      keine Eigenschaft unseres Prozesses, weil Windows dort ohnehin schreibt.
- [ ] **Nicht-Störung:** der Call läuft während der Erfassung ohne Ton-Ausfall
      weiter; der Call-Client zeigt keinen Fehler.
- [ ] Ein fehlendes Mikrofon führt zu einer Warnung und einer reinen
      `Remote`-Sitzung, nicht zu einem Abbruch.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Ein Call-Client rendert Ton aus einem Prozess **außerhalb** des gewählten Baums (eigener Audio-Dienst) — dann erfasst Phase 1 nichts | Der Smoke-Test gegen den echten Client ist Teil des QA-Gates. Schlägt er fehl, eskaliert das Issue mit `needs:planning` an die Planung statt eine Krücke zu bauen |
| Die Auflösung auf die Prozessbaum-Wurzel erfasst mehr als die gewählte Anwendung und bricht die Quellen-Isolation | Stop-Liste für Shell- und Dienst-Wurzeln, die aufgelöste Wurzel wird **vor** der Consent-Abfrage angezeigt, und der Isolations-Testfall prüft es messend |
| Das Referenzbeispiel als Vorlage bringt seine unbegrenzt wachsende Puffer-Strategie mit | Explizit entschieden: Chunk-Größe nur aus `get_next_packet_size()`, `get_buffer_size` auf dem Loopback-Client verboten. Der Ringpuffer-Test prüft die Kapazität messend |
| Uhren-Drift zwischen Mikrofon- und Loopback-Gerät verschiebt die Ströme gegeneinander und beschädigt später die Sprecherzuordnung | Geräte-Zeitstempel statt Wanduhr, gemeinsame Sitzungs-Null, `data_discontinuity` als explizite Lücke mit Zähler. Die tatsächliche Drift wird in Phase 3 messbar und dort bewertet |
| `wasapi`-API-Drift bei einem Minor-Update | Auf `0.23.x` pinnen. Die Prüfung dieser Spec bezieht sich ausdrücklich auf v0.23.0, gelesen am 2026-07-30 |
| CI-Runner ohne Audiogerät — Erfassungstests können dort nicht laufen | Die synthetische Testton-Quelle trägt die Tests von `audio` und `session`; `audio-win` bekommt in CI nur Kompilier- und reine Logiktests |
| Die Echokompensation ist auf manchen Mikrofonen nicht verfügbar | Fähigkeitsprobe `is_aec_supported()`, offengelegte Degradierung, im Sitzungszustand geführt und ab Phase 4 im Protokollkopf vermerkt |
| Die Migration auf Edition 2024 bricht den Seed | Der Workspace enthält bisher ein Placeholder-Crate; Verify deckt den Bruch sofort auf. Läuft als erstes, kleinstes Issue |
| Ein Puffer-Überlauf im Dauerbetrieb verliert unbemerkt Sprache | Verlust ist ein gemeldetes Ereignis mit Zähler, das die CLI ausgibt, kein stiller Zustand. Bei 30 s Puffer ist ein Verlust ein echter Defekt und soll sichtbar sein |

## Decision log

- 2026-07-30: `wasapi-rs` v0.23.0 als lesender, wegwerfbarer Klon außerhalb des Repos
  geprüft, um die tragende Annahme der ganzen Anwendung zu belegen statt sie zu
  glauben. Belegt: `new_application_loopback_client` mit `include_tree`,
  Format-Übergabe wegen `E_NOTIMPL` bei `GetMixFormat`, Render-Session-Aufzählung mit
  `SessionState`, `Role::Communications`, `is_aec_supported`/`get_aec_control`. Kein
  externer Code ins Repo übernommen.
- 2026-07-30: Drei Landminen aus der Limitationsliste des Loopback-Clients, die alle
  drei in Entscheidungen übersetzt sind: die Eltern-PID muss das Erfassungsziel sein;
  die Communications-Kategorie ist auf einem Loopback-Stream ungültig; und
  `get_buffer_size`/`get_device_period` liefern dort keine brauchbaren Werte — der
  Grund, warum das Referenzbeispiel unbegrenzt reallokiert und warum wir das nicht
  dürfen.
- 2026-07-30: Die Echokompensation wandert **in** Phase 1, statt eine eigene Phase zu
  werden. Grund: sie ist über die geprüfte API rund zehn Zeilen mit eingebauter
  Fähigkeitsprobe, und ohne sie ist das Kriterium „ich gegen Gegenseite 100 %
  korrekt" bei Lautsprecher-Nutzung nicht erfüllt. `docs/roadmap.md` bekommt den
  Zusatz beim Merge mit.
- 2026-07-30: Review des Spec-Entwurfs durch einen frischen Agenten hat vier
  Widersprüche zwischen `docs/architecture.md`, `docs/constitution.md` und dem Code
  aufgedeckt (Heimat von `Session`, `core`-Abhängigkeiten, Heimat von
  `StreamIdentity`, Pfad des Audit-Tests) sowie zwei stillschweigende Abweichungen
  von der Constitution (Audit-Test-Pfad, FS-Audit erst ab Phase 4). Alle sind jetzt
  benannt und einem Schritt dieser Phase zugeordnet, statt auf `main` stehen zu
  bleiben.
- 2026-07-30: `audio-win` startet mit `#![forbid(unsafe_code)]`, obwohl die
  Constitution dem Backend die Ausnahme erlaubt — jeder geprüfte `wasapi`-Aufruf ist
  eine safe Funktion. Die Ausnahme wird erst gezogen, wenn ein konkreter Fall sie
  erzwingt.
