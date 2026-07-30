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
- [ ] Bei Lautsprecher-Nutzung erscheint der Ton der Gegenseite nicht im `Local`-Strom
      — die betriebssystemseitige Echokompensation ist aktiv, oder ihre Abwesenheit
      ist offengelegt.
- [ ] `cargo xtask verify` läuft lokal grün und in CI auf `windows-latest`.
- [ ] Kein Foundation-Dokument und kein Kommentar im Repo widerspricht mehr dem
      Code — die belegten Widersprüche sind aufgelöst (Liste in In scope).

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
- **Foundation-Doc-Korrekturen** — die belegten Widersprüche zwischen
  `docs/architecture.md`, `docs/constitution.md` und dem Code, die diese Phase
  auflöst, statt sie auf `main` stehen zu lassen. Diese Liste ist die verbindliche
  Aufzählung; sie wird als **ein** Schritt erledigt:
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
     (Korrektur 1) **und** sagt, die Crate hänge von nichts außerhalb der
     Standardbibliothek ab (Korrektur 2). Beide Sätze werden mitgezogen.
  5. `docs/architecture.md` nennt `audio-win` „das einzige Crate mit `unsafe`". Nach
     dieser Phase hat **kein** Crate `unsafe` — die Zeile wird zu „das einzige Crate,
     das die `unsafe`-Ausnahme ziehen **darf**, wenn ein konkreter Fall sie erzwingt".
  6. Der Kommentar über `[workspace.lints.rust]` in `Cargo.toml` sagt, die
     Plattform-Backends opteten bewusst **nicht** ein, „because they need `unsafe`".
     Das ist die gefährlichste der Korrekturen: wer in Phase 8 oder 9 ein Backend
     anlegt, liest sie als Anweisung, `[lints] workspace = true` weglassen — und
     verliert damit lautlos `unsafe_code`, `missing_docs`, `too_many_lines`,
     `unwrap_used` und `expect_used`. Der Kommentar wird zu: einopten, und **nur**
     wenn die Ausnahme gezogen wird, alle Lints außer `unsafe_code` duplizieren.
  7. `docs/architecture.md`, Boundaries, sagt „`session` → alle fachlichen Crates".
     `session` kennt `audio-win` bewusst **nicht** (sonst wäre es plattformgebunden).
     Präzisiert auf `core` + `audio`.
  8. `docs/architecture.md`, Flow 1, schreibt `Session::start(consent)` ohne das
     Argument, das die Quellen trägt — wird an die Signatur unten angeglichen.
  9. `docs/constitution.md`, Don'ts, verbietet einen HTTP-Client in einer Crate
     `capture`, die es im Komponenten-Plan nicht gibt. Gemeint sind `audio` und
     `audio-win` — genau die Crates, die der Audit-Test bewacht. Ohne die Korrektur
     zeigt ein Don't auf nichts.
  10. `docs/workflow.md` sagt, ohne CMake und die C++-Build-Tools scheitere
      Bootstrap. In Phase 1 ist Bootstrap nur `cargo fetch --locked`; die Aussage
      gilt erst ab Phase 2 und wird entsprechend eingegrenzt.
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

Am Spec-Acceptance-Gate am 2026-07-30 vorgelegt. Nichts davon blockiert die
Implementierung: die Toolchain ist durch den Commit „prove Bootstrap and Verify"
belegt, alle übrigen Punkte sind **Voraussetzungen des QA-Gates** am Ende der Phase,
nicht des Bauens. Sie werden dort abgehakt.

- [x] Rust-Toolchain und `cargo-deny` lokal vorhanden — belegt, Verify läuft. CMake
      und die Visual-Studio-C++-Build-Tools aus `docs/workflow.md` werden erst ab
      Phase 2 gebraucht; Bootstrap ist in Phase 1 nur `cargo fetch --locked`.
- [x] Keine Secrets, keine Accounts, keine externe Provisionierung. GitHub Actions
      ist auf diesem öffentlichen Repo kostenfrei — nichts zu hinterlegen.
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

## Prior decisions

### Sitzung, Zustände und Nahtstellen

| Decision | Rationale | Date |
|---|---|---|
| `Session` und `CapturePlan` liegen in `session`, nicht in `core`. Signatur: `Session::start(consent: ConsentAttestation, plan: CapturePlan) -> Result<Session, SessionError>`, wobei `CapturePlan` das gewählte `CaptureSubject` und die geöffneten `Box<dyn AudioSource>` trägt | Die Constitution schreibt `Session::start(consent: ConsentAttestation)` als Consent-Gate fest, sagt aber nichts über die übrigen Argumente. `core` ist laut `docs/architecture.md` I/O-frei — ein Orchestrator, der Capture-Threads besitzt, und ein Plan, der `Box<dyn AudioSource>` hält, sind dort fehl am Platz. Der Consent bleibt das **erste** Argument und ohne ihn existiert kein Konstruktor | 2026-07-30 |
| Die Quellen werden über einen `SourceFactory`-Trait in `audio` geöffnet; `audio-win` liefert `WindowsSources`, `audio` liefert `TestToneSources`. `cli` ist der Composition Root und wählt die Implementierung | Hält `session` plattformfrei (kein `cfg` außerhalb des Backends) und macht die Testton-Quelle injizierbar — sonst ist keiner der `session`-Tests ohne Audiogerät lauffähig. `app` in Phase 5 benutzt dieselbe Fabrik, damit CLI und Oberfläche austauschbar bleiben | 2026-07-30 |
| Zustände: `Capturing → Stopping → Ended`, zwei Kanten, `Ended` terminal — eine Sitzung wird nicht neu gestartet. Es gibt **keinen** `Idle`-Zustand: „untätig" ist die **Abwesenheit eines `Session`-Wertes**. Ein fehlendes Mikrofon ist ein **Attribut** der laufenden Sitzung (`local_stream: Option<…>`), kein Zustand | Ein `Idle`-Variante wäre unbeobachtbar, weil `Session::start` der einzige Konstruktor ist — eine lebende `Session` ist nie untätig. Sie weglassen macht das Consent-Gate stärker, nicht schwächer: solange keine Attestation vorliegt, existiert der Typ nicht, statt in einem Zustand zu warten. Mikrofon-Abwesenheit als Zustand würde die Kanten verdoppeln, ohne etwas zu unterscheiden | 2026-07-30 |
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
| Alle drei `BufferFlags` werden behandelt: `data_discontinuity` erzeugt eine **explizite Lücke** auf der Zeitachse (Ereignis mit Zähler), `silent` wird als Nullen materialisiert, damit die Achse dicht bleibt, und `timestamp_error` läuft in denselben Lücken-Pfad wie `data_discontinuity` — mit eigenem Zähler, weil er die gewählte Zeitbasis selbst für unzuverlässig erklärt | Die Discontinuity-Flagge ist der eigentliche Lücken-Melder von WASAPI; wird sie ignoriert, verschiebt sich die Zeitachse still gegen die andere und die Sprecherzuordnung in Phase 3 driftet. `timestamp_error` zu ignorieren wäre schlimmer: dann wird ein falscher Zeitstempel als gültig übernommen. Der Event-Timeout unten ist ein anderer Fall | 2026-07-30 |
| Resampling auf 16 kHz mono passiert **auf der Leseseite in `audio`** (`rubato`), nicht im Backend; der Ringpuffer hält das native Format | `docs/architecture.md`, Flow 2 („Ringpuffer → Resampling → Fan-out"). Hält das Resampling in einem `forbid(unsafe_code)`-Crate, wo es ohne Audiogerät testbar ist | 2026-07-30 |
| PCM-Puffer werden bei Sitzungsende **explizit genullt** (`zeroize`), nicht nur freigegeben | Die Constitution fordert das Nullen für Embeddings; für PCM ist es gleich billig und deckt die Zusage „nichts Audio-förmiges überlebt" auch im Arbeitsspeicher ab | 2026-07-30 |

### Windows-Backend — gegen `wasapi-rs` v0.23.0 geprüft

| Decision | Rationale | Date |
|---|---|---|
| Loopback über `AudioClient::new_application_loopback_client(root_pid, include_tree = true)` | Am Quelltext geprüft: die Fähigkeit existiert als sichere Rust-API, `include_tree` bildet auf `PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE` ab. Kein eigener FFI-Code nötig | 2026-07-30 |
| Beide Clients bekommen **explizit** 48 kHz, stereo, 32-bit Float, aber **getrennte** Modus-Parameter: Loopback `EventsShared { autoconvert: true, buffer_duration_hns: 0 }`, Mikrofon `EventsShared { autoconvert: true, buffer_duration_hns: min_time }` aus `get_device_period()` | `GetMixFormat` liefert auf einem Process-Loopback-Client `E_NOTIMPL`, das Format muss also gesetzt werden. `get_device_period` funktioniert auf demselben Client ebenfalls nicht — daher 0 dort und der echte Wert nur auf dem Mikrofon. Ein gemeinsames Format hält Resampler-Konfiguration und Puffergröße für beide Ströme identisch | 2026-07-30 |
| Die Chunk-Größe kommt **ausschließlich** aus `get_next_packet_size()` (Rückgabe `Option<u32>`; `None` bedeutet „nichts abzuholen", nicht „Fehler"); `get_buffer_size()` wird auf dem Loopback-Client nicht aufgerufen | `wasapi-rs` dokumentiert, dass `get_buffer_size` dort fehlerfrei absurde Werte liefert (Größenordnung 3·10⁹). Das Referenzbeispiel umgeht das mit einer **unbegrenzt reallokierenden** `VecDeque` — genau das Gegenteil von „fest dimensioniert, nie vergrößert". Wer das Beispiel abschreibt, baut den Verstoß mit ein | 2026-07-30 |
| `get_audiosessioncontrol()` steht auf dem Loopback-Client nicht zur Verfügung; das Strom-Ende wird dort am Lesefehler erkannt | Ebenfalls in der Limitationsliste der Crate dokumentiert. Ohne diese Festlegung sucht der Implementierer einen Kanal, den es nicht gibt | 2026-07-30 |
| Ein Timeout beim Warten auf das Capture-Event ist **Stille, kein Fehler** — er erhöht einen Zähler und die Erfassung läuft weiter. Nur ein invalidiertes Gerät beendet den Strom | Eine ausgewählte Anwendung, die gerade nichts abspielt, ist der Normalfall. Beide Referenzbeispiele behandeln den Timeout als fatal; für uns wäre das ein Abbruch bei jeder Gesprächspause | 2026-07-30 |
| Die Anwendung wird auf ihre **Prozessbaum-Wurzel** aufgelöst, aber nur bis zu einer Stop-Liste (`explorer.exe`, `services.exe`, `svchost.exe`, `wininit.exe`, PID 0 und 4) | Das Referenzbeispiel warnt ausdrücklich: für Baum-Erfassung muss die **Eltern**-PID das Ziel sein, weil Call-Clients in Kindprozessen rendern. Ungebremstes Hochlaufen würde dagegen die Shell-Wurzel erfassen und die Quellen-Isolation brechen — genau das Kriterium dieser Phase. Die CLI zeigt die aufgelöste Wurzel **vor** der Consent-Abfrage | 2026-07-30 |
| Die Quellenliste kommt aus den Audio-Sessions **aller** Render-Geräte der `Direction::Render`-Gerätesammlung, gefiltert auf `SessionState::Active`, ohne den eigenen Prozess und ohne PID 0 und 4, und **dedupliziert auf die aufgelöste Prozessbaum-Wurzel** — nicht auf die rohe Session-PID. Prozessnamen und Elternschaft aus `sysinfo` | `get_audiosessionenumerator` hängt am Gerät, nicht am System, also muss über die Gerätesammlung iteriert werden — die Endpunkt-Unabhängigkeit aus `docs/prior-art.md` gilt für den Erfassungs-Client, nicht für die Auflistung. Ungefiltert stünden `Inactive`- und `Expired`-Sessions in der Auswahl; `docs/design.md` fordert die Filterung auf aktive Tonausgabe. Dedup auf die Wurzel, weil zwei Kindprozesse desselben Clients sonst zwei identische Zeilen erzeugen. Achtung: das Referenzbeispiel zählt `Direction::Capture` auf — wir brauchen `Direction::Render` | 2026-07-30 |
| Zwischen `list-sources` und `capture` wird die Prozess-Identität **neu geprüft** (Name und Startzeit), nicht nur die PID | Windows recycelt PIDs. Ohne die Prüfung könnte `capture` eine andere Anwendung erfassen als die, der der Nutzer zugestimmt hat — ein Consent-Bruch, nicht nur ein Bug | 2026-07-30 |
| Echokompensation auf dem **Mikrofon**-Client: `Role::Communications` als Gerät, `StreamCategory::Communications` per `set_properties` **vor** `initialize_client`, danach `is_aec_supported()` → `get_aec_control()` → `set_echo_cancellation_render_endpoint(Some(render_endpoint_id))`. `initialize_mta()` läuft je Capture-Thread | Ohne AEC landet bei Lautsprecher-Nutzung der Ton der Gegenseite im `Local`-Strom und bricht das Kriterium „ich gegen Gegenseite 100 % korrekt". Der Weg kostet über die geprüfte API rund zehn Zeilen und hat eine eingebaute Fähigkeitsprobe. Die Reihenfolge ist bindend, und die Communications-**Kategorie** ist für einen Loopback-Stream ungültig — nur auf dem Mikrofon setzen | 2026-07-30 |

### Gates und Dokumente

| Decision | Rationale | Date |
|---|---|---|
| Der Quell-Audit liegt in `xtask/tests/no_write_paths.rs` — eine **bewusste Abweichung** von der Constitution, die den Pfad wörtlich als `tests/no_write_paths.rs` nennt | Der Workspace-Root ist ein virtuelles Manifest ohne Package, also kompiliert ein Wurzel-`tests/`-Verzeichnis nicht und `cargo test --workspace` würde den Test nie ausführen. `xtask` ist bereits die Heimat der Workspace-Gates. Die Abweichung wird am Spec-Acceptance-Gate offengelegt, nicht stillschweigend vollzogen | 2026-07-30 |
| Der Audit prüft die Crate-Liste und überspringt noch nicht existierende Crates | Lässt den Test mit den Phasen wachsen, statt bei jeder neuen Phase zu brechen | 2026-07-30 |
| Der Audit prüft **zusätzlich zur** Symbol-Blockliste der Constitution (`fs::write`, `fs::File::create`, `OpenOptions::write`, `reqwest`, `ureq`) auf `Serialize`/`serde` und wertet den **Abhängigkeitsgraphen** der Erfassungs-Crates aus | „Kein `Serialize` für Audio-Typen" ist ein Don't, das sich nicht als negative Trait-Zusicherung ausdrücken lässt. Ein Scan belegt nur die Abwesenheit **benannter** Symbole — die Manifest- und Graph-Prüfung schließt die Lücke, die ein Alias oder ein eigener `impl Write` sonst offen ließe | 2026-07-30 |
| Der Sitzungs-FS-Audit läuft **in Phase 1**, gerätefrei über die Testton-Quelle. Er beobachtet ein **prozess-eigenes** Verzeichnis: die Sitzung bekommt ein frisches Arbeitsverzeichnis **übergeben**, und nur dieses wird verglichen. Umbiegen von `TMP`/`TEMP` im laufenden Prozess ist **kein** gültiger Weg — `std::env::set_var` ist in Edition 2024 `unsafe`, und `forbid(unsafe_code)` gilt auch für Testcode. Wird eine Umgebungsvariable gebraucht, dann über einen Kindprozess (`Command::env`) | Die Constitution sagt „**bei jeder Änderung am Datenpfad** läuft zusätzlich der FS-Audit-Test" — Phase 1 *ist* der Datenpfad. `docs/workflow.md` macht ihn ab Phase 4 verpflichtend, was die frühere Zeile nicht aufhebt. Mit der Testton-Quelle ist er ohne Audiogerät lauffähig. Das gemeinsame Temp-Verzeichnis zu vergleichen wäre derselbe Fehler wie ein Snapshot über das ganze Nutzerprofil: cargo und fremde Prozesse schreiben dort, das Gate würde sporadisch rot und damit wertlos | 2026-07-30 |
| Das Consent-Gate wird per `compile_fail`-Fall (`trybuild`) belegt, nicht per Review-Urteil | Das Vision-Kriterium lautet „die Aufnahme startet **nachweisbar** nie ohne bestätigte Attestation". Ein Review ist eine Momentaufnahme, ein `compile_fail`-Fall ein Dauergate | 2026-07-30 |
| Kein Design-Zyklus (`/loopkit:design`) in dieser Phase | Phase 1 liefert ein CLI-Harness, hat also keine UI-Fläche. Die Zustandsmaschine ist oben mit drei Zuständen und zwei Kanten vollständig beschrieben — eine Visualisierung würde keine Entscheidung schärfen. Quellenauswahl und Consent-Dialog sind in `docs/design.md` als Komponenten festgelegt und werden in Phase 5 entworfen | 2026-07-30 |
| Der Attestation-Text ist die ausführliche Fassung mit DSGVO-Teil, im Volltext unten. `ATTESTATION_V1` in `core`, deutsch als Quelle, englisch 1:1 daraus übersetzt | Am Spec-Acceptance-Gate entschieden. Die Attestation **ist** laut `docs/vision.md` die Dokumentation des Consents — sie muss dem Nutzer sagen, wofür er einsteht, nicht nur ein Häkchen einsammeln. Die Sätze über die abwesende Aufnahme und das nicht überlebende Stimmprofil stehen darin, weil der Nutzer beim Einholen des Einverständnisses genau das zusichern können muss | 2026-07-30 |
| Fehlt die Echokompensation (`is_aec_supported() == false`), wird **gewarnt und weitergelaufen**. Die Degradierung wird im Sitzungszustand geführt, von der CLI ausgegeben und ab Phase 4 im Protokollkopf vermerkt | Am Spec-Acceptance-Gate entschieden. Ein Startverbot würde das Werkzeug auf jeder Maschine ohne AEC-fähiges Mikrofon unbenutzbar machen — auch für Nutzer mit Kopfhörern, die es gar nicht brauchen. Offenlegen statt bevormunden: der Nutzer sieht, dass die Sprecherzuordnung in dieser Sitzung unsicher ist | 2026-07-30 |
| Alle drei Fundament-Nachzüge aus Phase 0 (Edition 2024, in `rust-toolchain.toml` gepinnte MSRV, CI-Workflow auf `windows-latest`) gehören **in diese Phase**, als erstes und kleinstes Issue | Am Spec-Acceptance-Gate entschieden. Diese Phase legt vier neue Crates an, die die Edition erben — sie auf 2021 zu bauen und später zu migrieren wäre teurer als der Nachzug jetzt. Ohne CI mergen alle folgenden PRs allein gegen lokales Verify | 2026-07-30 |

### Attestation-Text V1 (verbindlicher Wortlaut)

Deutsch ist die Quelle; die englische Fassung wird daraus übersetzt und mit
derselben Versionsnummer geführt. Der Text wird **im Volltext** angezeigt, nie
gekürzt (`docs/design.md`, Consent-Dialog), und die Bestätigung ist nur über das
Häkchen möglich.

> Ich bestätige, dass ich alle Gesprächsteilnehmer vor Beginn der Erfassung über die
> Mitschrift informiert habe und dass ihr Einverständnis vorliegt.
>
> Mir ist bewusst, dass das Aufzeichnen oder Mitschreiben des nicht öffentlich
> gesprochenen Wortes ohne Einverständnis der Sprechenden strafbar ist (§ 201 StGB).
>
> Das entstehende Protokoll enthält personenbezogene Daten. Für seine Aufbewahrung
> und Löschung bin ich verantwortlich.
>
> Es entsteht zu keinem Zeitpunkt eine Ton- oder Bildaufnahme, und kein Stimmprofil
> überlebt diese Sitzung.
>
> Diese Bestätigung wird mit Zeitstempel im Protokollkopf festgehalten.
>
> `[ ]` Ich bestätige das Vorstehende.

## Tracking

- Milestone: [Phase 1 — Capture-Fundament Windows](https://github.com/bhemsen/transcriber/milestone/1)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist — eines je
  implementierbarem Schritt

Jedes Issue verweist im Body auf diesen Spec-Pfad.

## Verification

Maschinell, in Verify und in CI auf `windows-latest`:

- [ ] `cargo xtask verify` grün — `fmt`, `clippy -D warnings`, `cargo test --workspace`,
      `cargo deny check`.
- [ ] `cargo xtask build` grün.
- [ ] Der Quell- und Manifest-Audit findet in `audio` und `audio-win` keines der
      gelisteten Symbole und keine der gelisteten Abhängigkeiten, und schlägt fehl,
      wenn eines eingeführt wird — einmal durch eine absichtliche Verletzung belegt.
- [ ] Der Sitzungs-FS-Audit läuft eine Sitzung gegen die Testton-Quelle und findet
      im Arbeitsverzeichnis und im **prozess-eigenen** Temp-Verzeichnis (`TMP`/`TEMP`
      für die Testsitzung umgebogen) 0 neue Dateien.
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
- [ ] Zeitachsen-Test über alle drei Flags: eine injizierte `data_discontinuity`
      **und** ein injizierter `timestamp_error` erscheinen je als Lücke mit eigenem
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
- 2026-07-30: Zwei Review-Runden durch einen frischen Agenten haben die
  Widersprüche zwischen `docs/architecture.md`, `docs/constitution.md`,
  `docs/workflow.md` und dem Code aufgedeckt. Die vollständige, verbindliche
  Aufzählung steht in In scope — hier keine zweite, die davon abweichen könnte.
  Dazu zwei stillschweigende Abweichungen von der Constitution, die jetzt
  offengelegt sind: der Pfad des Audit-Tests und der Versuch, den FS-Audit erst ab
  Phase 4 laufen zu lassen. Der gefährlichste Einzelfund war der Kommentar in
  `Cargo.toml`, der einem künftigen Backend-Autor sagt, die Workspace-Lints nicht zu
  aktivieren — er hätte in Phase 8 oder 9 fünf Gates lautlos entfernt.
- 2026-07-30: Spec-Acceptance-Gate. Drei offene Punkte entschieden: der
  Attestation-Text in der ausführlichen Fassung mit DSGVO-Teil (Volltext oben), eine
  fehlende Echokompensation wird gewarnt und nicht verboten, und alle drei
  Fundament-Nachzüge aus Phase 0 laufen in dieser Phase. Damit ist keine Entscheidung
  dieser Phase mehr offen.
- 2026-07-30: `audio-win` startet mit `#![forbid(unsafe_code)]`, obwohl die
  Constitution dem Backend die Ausnahme erlaubt — jeder geprüfte `wasapi`-Aufruf ist
  eine safe Funktion, `sysinfo` ebenso, und die einzige `pub unsafe fn` der Crate
  (`Device::from_raw`) braucht diese Phase nicht. Die Ausnahme wird erst gezogen,
  wenn ein konkreter Fall sie erzwingt. Weil diese Entscheidung zwei bestehende
  Artefakte falsch macht — die Zeile „das einzige Crate mit `unsafe`" in
  `docs/architecture.md` und den Lint-Kommentar in `Cargo.toml` — trägt sie ihre
  eigene Korrektur in der Liste in In scope; eine Entscheidung, die ein Dokument
  hinter sich unwahr lässt, ist nicht fertig.
- 2026-07-30: Der CI-Workflow installiert `cargo-deny` mit der vorinstallierten
  `stable`-Toolchain des Runners (`cargo +stable install`), nicht mit der in
  `rust-toolchain.toml` gepinnten MSRV, und pinnt die Version explizit
  (`--version 0.20.2`). Grund: `cargo-deny`s eigene MSRV läuft der unseren
  unabhängig davon und schon voraus — 0.20.x verlangt Rust ≥ 1.88, während diese
  Phase auf 1.85.0 pinnt. Ein `cargo install cargo-deny --locked` ohne
  Toolchain- und Versions-Override hätte in CI **erst nach dem Merge** mit
  einem MSRV-Fehler abgebrochen; lokal blieb es unbemerkt, weil dort bereits
  eine vorgebaute `cargo-deny`-Binary (dieselbe Version, 0.20.2) installiert
  war und der aus dem Quelltext bauende Fall — der einzige, an dem die MSRV
  greift — nie durchlief. Die Versions-Pinnung ist ein separater Grund: sie
  hält das Gate reproduzierbar, unabhängig vom Toolchain-Override. Wer die
  Projekt-MSRV anhebt, sollte die Toolchain-Entkopplung nicht stillschweigend
  entfernen — sie ist einzig deshalb da, weil `cargo-deny` keine
  Rückwärtskompatibilität zur gepinnten Rust-Version zusichert.
- 2026-07-30: Issue #4 (`core`-Domänentypen) legt vier Detailentscheidungen fest,
  die die Spec offen ließ:
  - `CaptureSubject` trägt genau `process_name: String` und `root_pid: u32` — exakt
    das, was `list-sources` laut Outcome zeigt und was `audio-win` zur Auflösung
    braucht. Keine weiteren Felder, bis eine spätere Phase sie verlangt.
  - `SessionId` ist ein opaker, monotoner `u64`-Zähler (`AtomicU64`), keine UUID,
    ohne öffentlichen Zugriff auf den Rohwert (nur `Debug`, für Logging). Sessions
    werden nie persistiert oder über Prozessgrenzen hinweg wiederaufgenommen, also
    reicht Eindeutigkeit **innerhalb eines Prozesslaufs**; eine zusätzliche
    Abhängigkeit (`uuid`, `rand`) wäre für diese Phase unbegründet und steht auch
    nicht in der Allowlist der Constraints. Bewusst **kein** `Default`: `new()` hat
    einen Seiteneffekt (der Zähler rückt vor), zwei Aufrufe liefern nie denselben
    Wert — genau das Gegenteil dessen, was `Default` üblicherweise zusichert.
  - Der Attestation-Text wird als zusammenhängender String mit `\n\n` zwischen den
    Absätzen abgelegt, der letzte Satz „Ich bestätige das Vorstehende." /
    „I confirm the above." eingeschlossen. Das rohe Markdown-Checkbox-Symbol
    `[ ]` ist reine UI-Notation der Spec-Darstellung, nicht Teil des Wortlauts, und
    fehlt deshalb in der Konstante — das eigentliche Häkchen zeichnet der Dialog.
    Die deutsche Quelle unterscheidet bewusst „Erfassung" (unsere eigene Handlung)
    von „Aufzeichnen"/„Aufnahme" (die verbotene bzw. abwesende Tonaufnahme); die
    englische Fassung hält das mit „capture" gegen „recording" durch, sonst würde
    Absatz 1 der eigenen Zusicherung in Absatz 4 widersprechen. Der Klammerzusatz
    „(§ 201 StGB)" bleibt ohne Erläuterungszusatz in beiden Sprachen — 1:1, nicht
    erklärt.
  - `SessionState` trägt die beiden Kanten (`request_stop`, `end`) selbst als reine,
    zustandslose Methoden, die bei einer ungültigen Kante einen
    `SessionStateError` (via `thiserror`) zurückgeben statt zu paniken. Das ist die
    Zustandsmaschine selbst — verschieden vom `Session`-Orchestrator in `session`,
    der Threads und `AudioSource`s besitzt und weiterhin dort bleibt.
- 2026-07-30: Issue #5 (`audio`-Ringpuffer und Frame-/Formattypen) legt drei
  Detailentscheidungen fest, die die Spec offen ließ:
  - Ein `Frame::Gap` (`data_discontinuity` oder `timestamp_error`) belegt **keinen**
    Platz im Ringpuffer und rückt den internen Schreib-Cursor **nicht** vor — er
    erhöht ausschließlich den passenden Zähler (`discontinuity_count` /
    `timestamp_error_count`). Die reale Zeit einer Lücke steht allein im
    `DeviceTimestamp`, den der Frame trägt; der Ringpuffer selbst kennt nur
    Sample-Positionen, keine Zeit. Ein `Frame::silent` dagegen wird als echte
    Null-Samples in den Ring geschrieben — nur so bleibt die gelesene
    Sample-Folge für `silent` dicht, während sie über eine Lücke hinweg bewusst
    nicht dicht ist.
  - Die beiden Gap-Zähler hängen am `RingBuffer`, nicht am einzelnen Leser-Cursor.
    Phase 1 nutzt ohnehin nur einen Leser produktiv (siehe Out of scope); ein
    globaler Zähler je Ursache erfüllt die Verification-Zeile "je Ursache ein
    eigener Zähler" ohne die Cursor-API vorzeitig um Fan-out-Semantik zu
    erweitern, die erst Phase 2/3 braucht.
  - Verlust wird **lazy** beim `read()`-Aufruf berechnet (Soll- gegen
    Ist-Position des jeweiligen Cursors), nicht eager bei jedem `push()`. Das
    hält `push()` in O(1) unabhängig von der Leserzahl — der konkrete Mechanismus
    hinter der Spec-Zusage, dass der Fan-out an ASR und VAD in Phase 2/3 rein
    additiv wird, ohne den Ringpuffer selbst anzufassen.
- 2026-07-30: Issue #8 (Quell-, Manifest- und Graph-Audit) legt drei
  Detailentscheidungen fest, die die Spec offen ließ:
  - Der Symbol-Scan normalisiert Whitespace vor dem Vergleich (alle
    Leerraumzeichen entfernt), damit sowohl `std :: fs :: write` als auch
    `use std::fs::write as w;` erkannt werden — Letzteres, weil der
    `use`-Pfad selbst noch die volle qualifizierte Zeichenkette trägt, auch
    wenn der spätere Aufruf nur den Alias `w(...)` nennt. Für
    `OpenOptions::write` reicht das nicht: idiomatischer Code schreibt diesen
    Pfad praktisch nie als eine zusammenhängende Kette, sondern
    `OpenOptions::new()` und danach `.write(true)` als eigener
    Builder-Schritt. Dafür gibt es eine zusätzliche Heuristik: Datei enthält
    `OpenOptions` **und** `.write(` → Fund. Was der Scan bewusst nicht kann:
    eine Cross-Crate-Umleitung erkennen, die den Pfad nie im Quelltext der
    bewachten Crate selbst wiederholt (z. B. ein `impl Write` in einer
    unbewachten Hilfs-Crate) — dafür ist der Graph-Check die zweite,
    unabhängige Sicherung, aber nur für die drei Crate-Namen (`serde`,
    `reqwest`, `ureq`), nicht für die beiden `fs`-Symbole, die keine Crate
    sind. Der Audit scannt ausschließlich die vier fest benannten
    Crate-Verzeichnisse aus `audit::GUARDED_CRATES`, niemals `xtask` selbst —
    ein Selbsttest (`guarded_crates_never_include_the_audit_tool_itself`)
    hält das dauerhaft fest, damit der Scan nie seine eigenen
    String-Literale oder Fixtures als Fund meldet.
  - Manifest- und Graph-Prüfung laufen als zwei getrennte Schichten über
    dieselbe reine Funktion (`blocked_dependencies_in`): die Manifest-Schicht
    liest `[dependencies]` aus der `Cargo.toml` der Crate direkt (Text-Parser
    für die flache `name = "version"`-Form, die jede Crate hier aktuell
    nutzt — eine `[dependencies.foo]`-Untertabelle würde nicht erkannt,
    bewusst offengelegte Lücke statt stiller Annahme); die Graph-Schicht ruft
    `cargo tree --offline -e normal,build` für das Crate auf und prüft den
    **vollständigen transitiven** Abhängigkeitsbaum, damit auch eine indirekt
    ankommende `serde`-Abhängigkeit auffällt. Beide Schichten brauchen keine
    neue Abhängigkeit (kein `cargo_metadata`, kein `serde_json`) — `cargo
    tree` ist ein in Cargo eingebauter Befehl, der Klartext statt JSON
    liefert, also entfällt ein Eintrag in der `cargo-deny`-Allowlist.
  - Der Test liegt bewusst nicht als `xtask/tests/audit.rs`, sondern als
    `xtask/tests/no_write_paths.rs` (der Einstiegspunkt, von Cargo als
    einziges Test-Target automatisch erkannt) plus
    `xtask/tests/audit/mod.rs` (die reinen Detektoren mit ihren
    Fixture-Tests, per `mod audit;` eingebunden). Ein `audit.rs` direkt unter
    `tests/` hätte Cargo als **zweites, eigenständiges** Test-Target
    entdeckt — die Fixture-Tests wären doppelt gelaufen und `audit`s
    `pub(crate)`-Sichtbarkeit hätte über die Crate-Grenze hinweg nicht mehr
    gegolten. Die `<name>/mod.rs`-Form ist das etablierte Muster für
    geteilte Hilfsmodule in Integrationstests, genau um das zu vermeiden.
