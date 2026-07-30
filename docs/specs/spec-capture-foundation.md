# Spec: Phase 1 — Capture-Fundament Windows

> Created: 2026-07-30

Diese Spec liefert das Fundament der Erfassung: die Tonspur **einer** gewählten
Anwendung und das eigene Mikrofon als zwei strukturell getrennte Ströme, einen fest
dimensionierten Ringpuffer, eine Sitzungs-Zustandsmaschine, deren Start typseitig an
die Consent-Attestation gebunden ist, und ein CLI-Harness, mit dem beides ohne
Oberfläche nachweisbar ist. Kein Audio verlässt den Arbeitsspeicher.

Prosa auf Deutsch, Identifier auf Englisch — `docs/constitution.md`, Conventions.

## Outcome

- [ ] `list-sources` zeigt die laufenden Anwendungen mit aktiver Tonausgabe, je mit
      Prozessnamen und der **Prozessbaum-Wurzel-PID**, die erfasst würde.
- [ ] `capture` erfasst genau die gewählte Anwendung: parallel in einer **anderen**
      Anwendung abgespielte Musik erscheint messbar nicht im erfassten Strom
      (Pegel bleibt auf Stille-Niveau).
- [ ] Mikrofon und Anwendungston liegen als zwei Ströme mit den Identitäten `Local`
      und `Remote` vor; kein Codepfad mischt sie.
- [ ] Es existiert kein Weg, eine Erfassung ohne `ConsentAttestation` zu starten —
      `Session::start` nimmt sie als Wert, und es gibt keinen zweiten Konstruktor.
- [ ] Ein zehnminütiger Lauf hinterlässt **0 Bytes** neuer Dateien; der Audit-Test
      belegt, dass die Erfassungs-Crates keinen Schreib-, Netz- oder
      Serialisierungspfad enthalten.
- [ ] Der Speicherverbrauch ist unabhängig von der Laufzeit konstant: ein
      absichtlich langsamer Leser meldet Verlust, statt den Puffer wachsen zu lassen.
- [ ] `cargo xtask verify` läuft grün, in CI auf `windows-latest` ebenso wie lokal.

## Scope

### In scope

- Crate `audio`: `AudioSource`-Trait, PCM-Frame- und Formattypen, fest
  dimensionierter Ringpuffer, Downmix und Resampling auf 16 kHz mono, Strom-Identität
  `Local` / `Remote`, synthetische Testton-Quelle.
- Crate `audio-win`: Aufzählung der Anwendungen mit aktiver Tonausgabe,
  Prozessbaum-Loopback-Erfassung, Mikrofon-Erfassung mit betriebssystemseitiger
  Echokompensation. Das einzige Crate mit `unsafe`.
- Crate `session`: Zustandsmaschine, typseitiges Consent-Gate, Event-Bus,
  Sitzungs-Lebenszyklus samt Nullen der Puffer am Ende.
- Crate `core`: `StreamIdentity`, `CaptureSubject`, `ConsentAttestation`, der
  versionierte Attestation-Text (DE + EN), typisierte Fehler.
- Crate `cli`: das Harness für die Phasen 1–4 — `list-sources` und `capture` mit
  Consent-Abfrage und Pro-Strom-Statistik.
- Der Audit-Test gegen Schreib-, Netz- und `serde`-Pfade in den
  Erfassungs-Crates, verdrahtet in Verify.
- Fundament-Nachzug aus Phase 0: Edition 2024, in `rust-toolchain.toml` gepinnte
  MSRV, GitHub-Actions-Workflow auf `windows-latest` (siehe die offene
  Scope-Entscheidung unten).

### Out of scope

- Spracherkennung, VAD, Segmentierung, Embeddings, Clustering — Phasen 2 und 3.
- Protokoll-Ausgabe und der automatisierte FS-Audit-Test über eine ganze Sitzung —
  Phase 4. Phase 1 belegt die Null-Byte-Zusage über den Quell-Audit und eine
  manuelle Verzeichnisprüfung.
- Der Fan-out **eines** Ringpuffers an **zwei** Konsumenten (ASR und VAD). Phase 1
  baut die Leser-Cursor-API, die den Fan-out später rein additiv macht, und nutzt
  vorerst einen Leser.
- Oberfläche, Installer, Modellbereitstellung — Phasen 5 und 6.
- macOS- und Linux-Backends — Phasen 8 und 9. `AudioSource` existiert, die
  Implementierungen nicht.
- Auswahl des Mikrofon-Geräts durch den Nutzer. Phase 1 nimmt das
  Kommunikations-Standardgerät; eine Auswahl kommt mit den Einstellungen in Phase 5.
- Akustische Qualitätsbewertung des Tons. Erst Phase 2 misst etwas (WER).

## Constraints

- `#![forbid(unsafe_code)]` in `core`, `audio`, `session`, `cli`; **nicht** in
  `audio-win`. Dort wird jeder `unsafe`-Block mit einem `// SAFETY:`-Kommentar
  begründet (`docs/constitution.md`, Architecture principles).
- PCM- und Format-Typen tragen kein `serde::Serialize` und kein `Debug`, das Samples
  ausgibt. `audio` und `audio-win` haben keinen Schreibpfad und keinen HTTP-Client.
- Der Ringpuffer ist fest dimensioniert (≤ 30 s je Strom) und wird bei Überlauf
  überschrieben, nie vergrößert.
- Abhängigkeitsrichtung nach `docs/architecture.md`: `core` ← `audio` ← `audio-win`;
  `session` → alle fachlichen Crates. Kein `#[cfg(windows)]` außerhalb von
  `audio-win`.
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul; kein `unwrap()` und
  kein `expect()` außer in Tests und `main`.
- Neue Abhängigkeiten müssen unter die `cargo-deny`-Allowlist fallen. Für Phase 1
  vorgesehen: `wasapi` (MIT), `sysinfo` (MIT), `rubato` (MIT), `zeroize`
  (MIT/Apache-2.0), `thiserror` (MIT/Apache-2.0), `tokio` (MIT, nur Feature `sync`),
  `clap` (MIT/Apache-2.0). Alle Lizenzen stehen bereits in `deny.toml`.
- Die GitHub-`windows-latest`-Runner haben **kein** Audiogerät. Alles, was in CI
  geprüft wird, muss ohne Gerät laufen.

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
- [ ] Ein installierter Call-Client für den Smoke-Test am QA-Gate (Teams, Zoom oder
      ein Browser-Meeting) und ein zweiter Ton-Erzeuger (Musik-App oder zweiter
      Browser) für den Isolations-Testfall. Bitte am Gate benennen, welcher Client
      es ist — die Prozessbaum-Auflösung wird gegen ihn verifiziert.
- [ ] Rust-Toolchain, `cargo-deny`, CMake und die Visual-Studio-C++-Build-Tools
      lokal vorhanden (`docs/workflow.md`, Environment prerequisites). Für Phase 1
      genügen Toolchain und `cargo-deny`; CMake wird erst ab Phase 2 gebraucht.
- [ ] Keine Secrets, keine Accounts, keine externe Provisionierung. GitHub Actions
      ist auf diesem öffentlichen Repo kostenfrei — nichts zu hinterlegen.

## Prior decisions

| Decision | Rationale | Date |
|---|---|---|
| Loopback über `AudioClient::new_application_loopback_client(root_pid, include_tree = true)` aus `wasapi` v0.23.0 | Am Quelltext geprüft: die Fähigkeit existiert als sichere Rust-API, `include_tree` bildet auf `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` ab. Kein eigener FFI-Code nötig. Minor-Version pinnen | 2026-07-30 |
| Das Erfassungsformat wird **explizit übergeben**: 48 kHz, stereo, 32-bit Float, `StreamMode::EventsShared { autoconvert: true }` | `GetMixFormat` liefert auf einem Process-Loopback-Client `E_NOTIMPL` — das Format muss gesetzt werden. 48 kHz/stereo/f32 ist die im Referenzbeispiel verifizierte Kombination | 2026-07-30 |
| Resampling auf 16 kHz mono passiert **auf der Leseseite in `audio`**, nicht im Backend; der Ringpuffer hält das native Format | `docs/architecture.md`, Flow 2 („Ringpuffer → Resampling → Fan-out"). Hält das Resampling in einem `forbid(unsafe_code)`-Crate, wo es ohne Audiogerät testbar ist. 30 s bei 48 kHz stereo f32 sind 5,8 MB je Strom — vertretbar | 2026-07-30 |
| Die zu erfassende Anwendung wird auf ihre **Prozessbaum-Wurzel** aufgelöst, aber nur bis zu einer Stop-Liste (`explorer.exe`, `services.exe`, `svchost.exe`, `wininit.exe`, PID 0 und 4) | Das Referenzbeispiel warnt ausdrücklich: für Baum-Erfassung muss die **Eltern**-PID das Ziel sein, weil Call-Clients in Kindprozessen rendern. Ungebremstes Hochlaufen würde dagegen die Shell-Wurzel erfassen und die Quellen-Isolation brechen — genau das Kriterium dieser Phase. Die CLI zeigt die aufgelöste Wurzel **vor** der Consent-Abfrage | 2026-07-30 |
| Die Quellenliste kommt aus den **Render**-Audio-Sessions (`get_iaudiosessionmanager` → `get_audiosessionenumerator` → `get_process_id`), Prozessnamen und Elternschaft aus `sysinfo` | Liefert genau die Prozesse mit aktiver Tonausgabe, wie `docs/design.md` es für die Quellenauswahl fordert. `sysinfo` ist MIT und vermeidet weiteren `unsafe`-Code. Achtung: das Referenzbeispiel zählt `Direction::Capture` auf — wir brauchen `Direction::Render` | 2026-07-30 |
| Echokompensation wird auf dem **Mikrofon**-Client aktiviert: `Role::Communications` als Gerät, `StreamCategory::Communications` als Kategorie, dann `is_aec_supported()` → `get_aec_control()` → `set_echo_cancellation_render_endpoint(render_endpoint_id)` | Ohne AEC landet bei Lautsprecher-Nutzung der Ton der Gegenseite im `Local`-Strom und bricht das Kriterium „ich gegen Gegenseite 100 % korrekt". Der Weg kostet über die geprüfte API rund zehn Zeilen und hat eine eingebaute Fähigkeitsprobe. Landmine aus dem Referenzbeispiel: die Communications-**Kategorie** ist für einen Loopback-Stream ungültig und darf nur auf dem Mikrofon-Client gesetzt werden | 2026-07-30 |
| Ein Timeout beim Warten auf das Capture-Event ist **Stille, kein Fehler** — er erhöht einen Zähler und die Erfassung läuft weiter. Nur ein invalidiertes Gerät beendet den Strom | Eine ausgewählte Anwendung, die gerade nichts abspielt, ist der Normalfall. Das Referenzbeispiel behandelt den Timeout als fatal; für uns wäre das ein Abbruch bei jeder Gesprächspause | 2026-07-30 |
| Der Ringpuffer vergibt **je Leser einen Cursor**; ein zurückgefallener Leser verliert Samples und erhält die Verlustzahl gemeldet, statt den Puffer wachsen zu lassen | Konstanter Speicherverbrauch ist eine Zusage der Constitution. Die Cursor-Form macht den Fan-out an ASR und VAD in Phase 2/3 rein additiv, ohne den Puffer neu zu schreiben | 2026-07-30 |
| PCM-Puffer werden bei Sitzungsende **explizit genullt** (`zeroize`), nicht nur freigegeben | Die Constitution fordert das Nullen für Embeddings; für PCM ist es gleich billig und deckt die Zusage „nichts Audio-förmiges überlebt" auch im Arbeitsspeicher ab | 2026-07-30 |
| `core` darf `thiserror` verwenden (und ab Phase 3 `zeroize`), sonst keine externen Abhängigkeiten. Zeitstempel als `std::time::SystemTime` | `docs/architecture.md` sagt für `core` „keine externen Abhängigkeiten", die Constitution fordert Fehler „typisiert über `thiserror`" — ein echter Widerspruch. Die Constitution ist normativ, also gewinnt `thiserror`; `docs/architecture.md` wird in dieser Phase entsprechend präzisiert. Kein Datums-Formatierer in `core`: ISO-8601 ist eine `protocol`-Sache | 2026-07-30 |
| Der Event-Bus nutzt `tokio::sync::broadcast` (Feature `sync`, **keine** Runtime) | `docs/architecture.md` legt tokio für die Orchestrierung fest. `broadcast::Sender::send` funktioniert ohne laufende Runtime, also bleibt der Audiopfad synchron und Phase 5 kann async konsumieren, ohne den Bus zu ersetzen | 2026-07-30 |
| Der Attestation-Text liegt als **versionierte Konstante in `core`** (DE + EN), nicht in `cli` oder im Frontend | `docs/design.md` fordert den Volltext im Dialog; `protocol` muss festhalten, welche Fassung attestiert wurde. Eine Quelle für CLI, künftige Oberfläche und Protokollkopf | 2026-07-30 |
| Ein fehlendes Mikrofon ist eine **Warnung**, kein Abbruch: die Sitzung läuft mit dem `Remote`-Strom allein weiter | Wer nur zuhört, soll ein Protokoll bekommen. Der `Local`-Strom fehlt dann sichtbar, statt die Sitzung zu verhindern | 2026-07-30 |
| Der Audit-Test liegt in `xtask/tests/no_write_paths.rs` und prüft die Liste der bewachten Crates, überspringt noch nicht existierende | Ein eigenes Crate nur für einen Test würde die Crate-Liste in `docs/architecture.md` aufblähen; `xtask` ist schon die Heimat der Workspace-Gates. Die Skip-Regel lässt den Test mit den Phasen wachsen, statt bei jeder neuen Phase zu brechen | 2026-07-30 |
| Der Audit-Test prüft **zusätzlich zur** Symbol-Blockliste der Constitution (`fs::write`, `fs::File::create`, `OpenOptions::write`, `reqwest`, `ureq`) auf `Serialize` und `serde` in den Erfassungs-Crates | „Kein `Serialize` für Audio-Typen" ist ein Don't der Constitution, das sich nicht als negative Trait-Zusicherung ausdrücken lässt. Ein Quell- und Manifest-Scan ist die verfügbare maschinelle Prüfung | 2026-07-30 |
| Kein Design-Zyklus (`/loopkit:design`) in dieser Phase | Phase 1 liefert ein CLI-Harness, hat also keine UI-Fläche, und die Zustandsmaschine hat vier Zustände — eine Visualisierung würde keine Entscheidung schärfen. Die Quellenauswahl und der Consent-Dialog sind in `docs/design.md` bereits als Komponenten festgelegt und werden in Phase 5 entworfen | 2026-07-30 |
| OPEN — Wortlaut der Consent-Attestation (DE + EN), die vor jedem Start bestätigt wird | resolved at the spec-acceptance gate | — |
| OPEN — Verhalten, wenn das Mikrofon keine Echokompensation unterstützt: warnen und weiterlaufen, oder Start verweigern, bis Kopfhörer bestätigt sind | resolved at the spec-acceptance gate | — |
| OPEN — gehören die drei Fundament-Nachzüge aus Phase 0 (Edition 2024, gepinnte MSRV, CI-Workflow) in diese Phase oder in eigene `track:adhoc`-Issues | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 1 — Capture-Fundament Windows (angelegt am Spec-Acceptance-Gate)
- Issues: created from this spec once it is merged (one per implementable step)

Each issue references this spec path in its body.

## Verification

Maschinell, in Verify und CI:

- [ ] `cargo xtask verify` grün — `fmt`, `clippy -D warnings`, `cargo test --workspace`,
      `cargo deny check` — lokal und auf `windows-latest`.
- [ ] `cargo xtask build` grün.
- [ ] Der Audit-Test findet in `audio` und `audio-win` keinen Schreib-, Netz- oder
      `serde`-Pfad und schlägt fehl, wenn einer eingeführt wird (durch eine
      absichtliche Verletzung einmal belegt).
- [ ] Ringpuffer-Test gegen die Testton-Quelle: Kapazität ≤ 30 s wird erzwungen; ein
      langsamer Leser meldet Verlust, die belegte Speichergröße bleibt konstant.
- [ ] Resampling-Test: 48 kHz stereo → 16 kHz mono, verifiziert gegen einen
      synthetischen Sinus (Frequenz bleibt, Länge stimmt, kein Aliasing über der
      Nyquist-Grenze).
- [ ] `session`-Test: `stop()` nullt die Puffer; nach dem Ende ist kein Sample
      ungleich Null mehr im Puffer auffindbar.
- [ ] Es existiert kein `Session`-Konstruktor ohne `ConsentAttestation` — im Review
      als Compile-Eigenschaft bestätigt.

Manuell am Milestone-QA-Gate (Smoke-Test nach `docs/workflow.md`):

- [ ] `list-sources` listet den laufenden Call-Client mit Prozessnamen und der
      aufgelösten Wurzel-PID; ein stiller Prozess erscheint nicht.
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
- [ ] **Null Bytes:** vor und nach einem zehnminütigen Lauf ein
      Verzeichnisvergleich über Arbeitsverzeichnis und Nutzerprofil — keine neue
      Datei, kein gewachsenes Verzeichnis.
- [ ] **Nicht-Störung:** der Call läuft während der Erfassung ohne Ton-Ausfall
      weiter; der Call-Client zeigt keinen Fehler.
- [ ] Ein fehlendes Mikrofon führt zu einer Warnung und einer reinen
      `Remote`-Sitzung, nicht zu einem Abbruch.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Ein Call-Client rendert Ton aus einem Prozess **außerhalb** des gewählten Baums (eigener Audio-Dienst) — dann erfasst Phase 1 nichts | Der Smoke-Test gegen den echten Client ist Teil des QA-Gates. Schlägt er fehl, eskaliert das Issue mit `needs:planning` an die Planung statt eine Krücke zu bauen |
| Die Auflösung auf die Prozessbaum-Wurzel erfasst mehr als die gewählte Anwendung und bricht die Quellen-Isolation | Stop-Liste für Shell- und Dienst-Wurzeln, die aufgelöste Wurzel wird **vor** der Consent-Abfrage angezeigt, und der Isolations-Testfall prüft es messend |
| `wasapi`-API-Drift bei einem Minor-Update | Auf `0.23.x` pinnen. Die Prüfung dieser Spec bezieht sich ausdrücklich auf v0.23.0, gelesen am 2026-07-30 |
| CI-Runner ohne Audiogerät — Erfassungstests können dort nicht laufen | Die synthetische Testton-Quelle trägt die Tests von `audio` und `session`; `audio-win` bekommt in CI nur Kompilier- und reine Logiktests (Baum-Auflösung, Stop-Liste) |
| Die Echokompensation ist auf manchen Mikrofonen nicht verfügbar | Fähigkeitsprobe `is_aec_supported()`, offengelegte Degradierung, im Sitzungszustand geführt und ab Phase 4 im Protokollkopf vermerkt |
| Die Migration auf Edition 2024 bricht den Seed | Der Workspace enthält bisher ein Placeholder-Crate; Verify deckt den Bruch sofort auf. Läuft als erstes, kleinstes Issue |
| Ein Puffer-Überlauf im Dauerbetrieb verliert unbemerkt Sprache | Verlust ist ein gemeldetes Ereignis mit Zähler, das die CLI ausgibt, kein stiller Zustand. Bei 30 s Puffer ist ein Verlust ein echter Defekt und soll sichtbar sein |

## Decision log

- 2026-07-30: `wasapi-rs` v0.23.0 als lesender, wegwerfbarer Klon außerhalb des Repos
  geprüft, um die tragende Annahme der ganzen Anwendung zu belegen statt sie zu
  glauben. Belegt: `new_application_loopback_client` mit `include_tree`,
  Format-Übergabe wegen `E_NOTIMPL` bei `GetMixFormat`, Render-Session-Aufzählung,
  `Role::Communications`, `is_aec_supported`/`get_aec_control`, sowie zwei Landminen
  (Eltern-PID als Ziel, Communications-Kategorie nur auf dem Mikrofon-Client). Kein
  externer Code ins Repo übernommen.
- 2026-07-30: Die Echokompensation wandert **in** Phase 1, statt eine eigene Phase zu
  werden. Grund: sie ist über die geprüfte API rund zehn Zeilen mit eingebauter
  Fähigkeitsprobe, und ohne sie ist das Kriterium „ich gegen Gegenseite 100 %
  korrekt" bei Lautsprecher-Nutzung nicht erfüllt.
