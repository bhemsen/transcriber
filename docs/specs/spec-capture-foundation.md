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
- 2026-07-30: Issue #6 (`audio`-Downmix und -Resampling) legt vier
  Detailentscheidungen fest:
  - **Downmix vor Resampling**, nicht danach: die Kanäle werden per
    arithmetischem Mittel auf mono gemischt, bevor `rubato` läuft. Mitteln
    zweier Kanäle erzeugt keine Frequenz, die im Quellsignal nicht schon
    vorhanden war — die Reihenfolge kann also selbst kein Aliasing erzeugen —
    und halbiert nebenbei die Sample-Menge, die der Resampler filtern muss.
    Die eigentliche Anti-Alias-Filterung bleibt vollständig `rubato`s Aufgabe.
  - `rubato::FftFixedInOut` statt eines `Sinc*`-Resamplers: bei 48 kHz → 16 kHz
    ist das Verhältnis exakt 3:1, genau der Fall, für den dieser Resampler-Typ
    gebaut ist. Sein Anti-Alias-Cutoff wird aus `fft_size_in`/`fft_size_out`
    automatisch knapp **unterhalb** der Ziel-Nyquist-Frequenz gelegt (bei
    16 kHz Ziel real bei rund 7,3 kHz, nicht erst bei 8 kHz — `rubato`s
    `BlackmanHarris2`-Fenster liegt bewusst auf der sicheren Seite) — ohne
    dass eigene Sinc-Parameter (`f_cutoff`, `sinc_len`, Fenster) von Hand
    kalibriert werden müssen, was die Fehlerfläche für ein falsch
    konfiguriertes Filter auf null reduziert. Eingabe-Chunk-Größe: als
    **Wunschgröße** 480 Frames (10 ms bei 48 kHz) an `FftFixedInOut::new`
    übergeben, aber `StreamResampler` liest die tatsächliche Chunk-Größe über
    `input_frames_next()`/`output_frames_next()` zurück, statt die
    Wunschgröße weiterzuverwenden — bei 48 kHz → 16 kHz (3:1) bleibt sie
    unverändert bei 480/160, bei anderen Raten (z. B. 44,1 kHz, wo `rubato`
    auf 882 Frames aufrundet) nicht. Ein erster Entwurf verwendete die
    Wunschgröße direkt weiter; das Review vor dem Merge deckte auf, dass das
    für jede Rate außer 48 kHz **jeden** `process_into_buffer`-Aufruf
    fehlschlagen ließ und über den Fehlerpfad in `resample_one_chunk` die
    gesamte Audiospur lautlos verwarf. Ein Regressionstest gegen 44,1 kHz
    belegt die Korrektur.
  - Der Resampler-Zustand (der FFT-Overlap-Tail, das Downmix-Restsample unter
    einem vollen Frame, die noch nicht abgeholten resampelten Samples) liegt
    vollständig in `StreamResampler`, gebunden an genau einen `ReaderId` bei
    Konstruktion. Zwei Leser desselben Ringpuffers bekommen zwei Instanzen —
    das macht den Fan-out aus Phase 2/3 rein additiv (eine weitere Instanz je
    zusätzlichem Leser, kein gemeinsamer Zustand, den ein langsamer Leser
    verderben könnte). Belegt durch einen Test mit zwei Lesern in
    unterschiedlichem Lesetempo, die byte-identische Ausgabe liefern.
  - Der synthetische Sinus für den Resampling-Test lebt ausschließlich im
    Testmodul (`crates/audio/src/resample/tests.rs`), nicht als öffentliche
    API — die Testton-Quelle samt `SourceFactory` ist Issue #7s Fläche, nicht
    diese.
  - **Zeroizing statt `Vec<f32>`:** die vier PCM-haltigen Felder von
    `StreamResampler` (`raw_scratch`, `raw_leftover`, `mono_pending`,
    `output_ready`) liegen in `Zeroizing<Vec<f32>>`, wie `RingBuffer::storage`
    und `Frame::Samples::samples` es bereits tun — die Constitution fordert
    das Nullen für PCM-Puffer allgemein, nicht nur für den Ringpuffer selbst.
    Ein Review vor dem Merge deckte auf, dass ein erster Entwurf hier vier
    unzeroisierte Felder plus zwei unzeroisierte temporäre `Vec`s pro
    resampeltem Chunk hatte. Behoben durch zwei Änderungen: die vier Felder
    bekamen `Zeroizing` **und** eine feste Erst-Kapazität in `new()`
    (`chunk_frames_in`/`chunk_frames_out`), und `downmix`/`resample_one_chunk`
    wurden umgeschrieben, um direkt in diesen Feldern zu arbeiten (Slices
    ansehen, dann `drain`/`truncate`), statt pro Aufruf einen frischen `Vec`
    zu bauen — damit existiert während einer Sitzung kein PCM-Sample mehr
    außerhalb der vier `Zeroizing`-Felder und von `rubato`s eigenem
    FFT-Overlap-Zustand (der von außen nicht zeroisierbar ist, siehe Risiko
    unten). Reduziert nebenbei die Allokationen im Lesepfad auf praktisch
    null im eingeschwungenen Zustand.
  - Zwei bewusst zurückgestellte Folgefragen, als eigene Issues erfasst statt
    nur hier vermerkt: [#33](https://github.com/bhemsen/transcriber/issues/33)
    (`StreamResampler` prüft `RingBuffer`/`StreamFormat`-Zusammengehörigkeit
    nicht strukturell) und
    [#34](https://github.com/bhemsen/transcriber/issues/34) (Gruppenlaufzeit
    von `rubato` und der nicht abrufbare Rest unter einer vollen Chunk-Größe
    am Sitzungsende sind weder dokumentiert noch abfließbar). Beide sind kein
    Bruch der Zusagen dieser Phase — `rubato`s eigener FFT-Zustand bleibt
    ohnehin außerhalb der `Zeroizing`-Reichweite, unabhängig vom Ausgang von
    #34.
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
- 2026-07-30: Review-Runde durch einen frischen Agenten (Opus) deckte einen
  echten Fund auf und führte zu vier Nachschärfungen, alle vor dem Merge
  umgesetzt:
  - **Der Fund:** `fs::File::create` als wörtliche Zeichenkette erkennt die
    idiomatische Form nie, die echter Code tatsächlich schreibt —
    `use std::fs::File;` gefolgt vom nicht qualifizierten `File::create(...)`
    — weil der volle Pfad dabei nirgends zusammenhängend im Quelltext steht.
    Genau dieselbe Beobachtung, die zur Builder-Heuristik für
    `OpenOptions::write` führte, war auf `File::create` nicht angewandt
    worden. Behoben durch Ersetzen des Blocklist-Eintrags durch `File::create`
    (Teilstring von `fs::File::create`, erkennt also weiterhin auch die
    vollqualifizierte Form, zusätzlich auch `File::create_new`) und eine
    zweite Builder-Heuristik für den Alias `File::options().write(...)`
    (stabil seit Rust 1.75 — dasselbe Muster wie `OpenOptions::new()`, nur
    unter anderem Namen).
  - Der Manifest- und der Graph-Parser bekamen je eine
    Nicht-Vakuität-Prüfung, dem bereits vorhandenen
    `at_least_one_guarded_crate_exists…`-Test nachgebildet: der
    Graph-Parser schlägt fehl, wenn das aufgerufene Paket nicht einmal sich
    selbst in der `cargo tree`-Ausgabe findet (das wäre sonst von einem
    kaputten Parser, der leer zurückgibt, nicht unterscheidbar); der
    Manifest-Parser schlägt fehl, wenn er nie einen `[dependencies]`-Header
    gesehen hat, statt eine leere, fälschlich "saubere" Liste zu melden.
  - Die Crate-Namen-Prüfung wechselt von exaktem Abgleich auf Präfix-Abgleich
    (`name.starts_with(prefix)`), weil `serde` sich seit 1.0.220 in
    `serde_core`/`serde_derive` aufspaltet — eine Familie, die unter dem
    exakten Namen `serde` durchrutschen könnte, ohne dass die Kern-Crate
    selbst je auftaucht.
  - `cargo tree` bekommt zusätzlich `--locked` (ein veraltetes Lockfile soll
    laut fehlschlagen, nicht still umgangen werden) und läuft über
    `env!("CARGO")` statt der wörtlichen Zeichenkette `"cargo"`, damit exakt
    die bauende Toolchain aufgerufen wird, nicht was ein `PATH`-Lookup sonst
    fände. `audit_crate` liest zusätzlich `build.rs`, falls vorhanden — ein
    Schreibpfad, den der reine `src/`-Scan sonst nie sehen würde.
  - Bewusst zurückgestellt, als Folge-Empfehlung ohne diesen Merge zu
    blockieren: eine vollständige Quervalidierung aller `crates/`-
    Unterverzeichnisse gegen eine explizite Allow-/Guard-Liste (heute deckt
    `at_least_one_guarded_crate_exists…` den akuten Fall ab, dass **keine**
    bewachte Crate mehr gefunden wird; ein Tippfehler in genau **einem**
    Eintrag bleibt möglich, solange mindestens eine andere Crate noch
    existiert) und die Erkennung von `[target.'cfg(...)'.dependencies]` in
    der Manifest-Schicht (relevant erst mit `audio-win`, dokumentiert als
    offene Lücke im Doc-Kommentar von `parse_direct_dependencies`).
- 2026-07-30: Issue #7 (`AudioSource`/`SourceFactory`-Traits und die
  synthetische Testton-Quelle) legt die Trait-Form fest, an der #9, #11–#13
  und #14 nicht mehr rütteln sollen:
  - `AudioSource: Send` mit genau drei Methoden: `identity() -> StreamIdentity`,
    `format() -> StreamFormat`, `pull(timeout: Duration) ->
    Result<AudioSourceEvent, AudioSourceError>` und einer vierten mit
    Default-Implementierung, `degradation() -> Option<SourceDegradation>`
    (Default `None`). `AudioSourceEvent` ist `Frame(Frame) | Idle | Ended` —
    die drei Zustände, die der Windows-Backend-Teil dieser Spec bereits
    festlegt (Event-Timeout, `get_next_packet_size() -> None` → `Idle`;
    Lesefehler auf dem Loopback-Client, invalidiertes Gerät → `Ended`), sodass
    `AudioSourceError` nur noch für einen wirklich unklassifizierten Fehler
    übrigbleibt. `Send` als Supertrait genügt, damit `Box<dyn AudioSource>`
    ohne weitere Annotation auf einen eigenen Capture-Thread wandert — belegt
    durch einen Test, der eine Instanz per `thread::spawn` verschiebt und
    `pull` dort aufruft, statt die Annahme nur zu behaupten.
  - `SourceDegradation` trägt bewusst nur `EchoCancellationUnavailable` und
    keinen Windows-Bezug im Namen oder Kommentar — die Fähigkeitsprobe
    `is_aec_supported()` ist `audio-win`s Sache, dieser Typ ist nur das
    plattformfreie Vokabular, über das eine Quelle das Ergebnis meldet.
  - `SourceFactory` (kein `Send`-Supertrait — nur die geöffneten `AudioSource`s
    wandern auf einen Capture-Thread, die Fabrik selbst nicht) mit
    `list_subjects()`, `open_remote(&CaptureSubject) ->
    Result<Box<dyn AudioSource>, SourceFactoryError>` und
    `open_local() -> Result<Option<Box<dyn AudioSource>>, SourceFactoryError>`.
    Ein fehlendes Mikrofon ist `Ok(None)`; `SourceFactoryError::Open{identity,
    reason}` ist für einen echten Fehler reserviert — auch für den Fall, dass
    `audio-win`s Identitätsprüfung zwischen `list-sources` und `capture` eine
    seit der Auflistung veränderte Prozessidentität entdeckt (eine recycelte
    PID) und deshalb bewusst laut scheitern muss statt eine falsche Anwendung
    zu öffnen.
  - `TestToneSource` synthetisiert einen Sinus aus einem Phasenakkumulator,
    der über `pull`-Aufrufe hinweg weiterläuft (kein Klicken an
    Chunk-Grenzen), in Chunks von 480 Frames (10 ms bei 48 kHz — in derselben
    Größenordnung wie das, was `get_next_packet_size()` auf dem echten
    Loopback-Client liefert; der reale Wert schwankt mit der
    Geräte-Periode, landet also nicht zuverlässig exakt auf 480) und mit
    `with_total_frames(...)` optional auf ein festes Budget begrenzt, damit
    ein Test `Ended` deterministisch erreichen kann, ohne dass ein Gerät das
    Stromende signalisiert. `TestToneSources` vergibt dem
    `Remote`- und dem `Local`-Strom unterschiedliche Frequenzen (440/660 Hz),
    damit ein künftiger Zwei-Strom-Sitzungstest (Issue #9) sie allein am Ton
    unterscheiden kann, und `without_microphone()` lässt `open_local()`
    `Ok(None)` liefern, um den "fehlendes Mikrofon"-Vertrag ohne echtes Gerät
    zu prüfen.
  - `audio` bekommt mit diesem Issue seine erste interne Pfad-Abhängigkeit
    (`transcriber-core`, für `CaptureSubject`/`StreamIdentity`) — im Einklang
    mit der Abhängigkeitsrichtung `core ← audio` aus `docs/architecture.md`.
    `cargo deny check`s `wildcards = "deny"`-Regel meldet eine Pfad-Abhängigkeit
    ohne `version`-Angabe als Wildcard-Fund. Ein Review vor dem Merge deckte
    auf, dass der naheliegende Gegenzug — `version = "0.1.0"` neben `path`
    ergänzen — den Build am nächsten Minor-Bump zerbricht: reproduziert durch
    `[workspace.package] version` auf `0.2.0` gesetzt und `cargo metadata
    --offline` ausgeführt, was mit `failed to select a version for the
    requirement transcriber-core = "^0.1.0"` fehlschlägt, weil jede Crate ihre
    Version über `version.workspace = true` erbt und die Pfad-Abhängigkeit
    damit eine zweite, unverbundene Kopie der Versionsnummer trägt —
    `docs/release.md` nennt `[workspace.package] version` ausdrücklich „die
    einzige Quelle der Versionsnummer". Stattdessen: `publish = false` in
    `crates/audio/Cargo.toml` **und** `crates/core/Cargo.toml` (ohnehin
    korrekt — `docs/release.md`: „Kein Registry-Schritt … nichts wird nach
    crates.io … publiziert") plus `allow-wildcard-paths = true` unter
    `[bans]` in `deny.toml`; `version` bleibt von der Pfad-Abhängigkeit ganz
    weg. Beide Teile sind nötig — `cargo-deny` 0.20.2 lehnt
    `allow-wildcard-paths` allein für eine als publizierbar markierte Crate
    ab. Belegt durch denselben `0.2.0`-Versionsbump erneut ausgeführt: löst
    jetzt ohne Fehler auf, und `cargo deny check` bleibt grün.
  - Zurückgestellt, nicht Teil dieses Issues: eine explizite `stop()`/`close()`-
    Methode auf `AudioSource`. Rust-Ownership plus `Drop` auf der konkreten
    Backend-Implementierung genügt, um eine Ressource beim Fallenlassen
    freizugeben; eine zusätzliche Trait-Methode dafür hätte nur Fläche ohne
    einen Fall, den die Downstream-Issues bereits brauchen.
  - Ein Review vor dem Merge deckte auf, dass `TestToneSource` `pull`s
    `timeout`-Argument zwar entgegennahm, aber niemals `Idle` melden konnte —
    genau der Fall, den die Spec als am häufigsten falsch behandelten nennt
    ("für uns wäre das ein Abbruch bei jeder Gesprächspause"). Behoben durch
    `TestToneSource::with_idle_every(n: NonZeroU64)` (jeder n-te `pull()`
    meldet `Idle` statt eines Frames, ohne die Tonhöhe oder `frames_emitted`
    zu verändern) und den passenden Durchgriff `TestToneSources::
    with_idle_every`/`with_total_frames`, damit auch ein Test, der nur
    `Box<dyn SourceFactory>` sieht (der ganze Zweck der injizierbaren
    Fabrik), einen geöffneten Strom deterministisch durch `Idle` und bis zu
    einem begrenzten `Ended` treiben kann, statt nur der direkte
    `TestToneSource`-Konstruktor.
  - Zurückgestellt, als Folge-Empfehlung ohne diesen Merge zu blockieren,
    beide aus demselben Review: `AudioSourceError`/`SourceFactoryError`
    tragen `reason: String` statt eines erhaltenen `#[source]`-Fehlers — für
    `audio-win`s künftige `wasapi`-Fehler ginge damit die ursprüngliche
    Fehlerkette verloren, sichtbar bliebe nur der formatierte Text. Und
    `CaptureSubject` (in `core`) trägt keine Startzeit, nur `process_name`
    und `root_pid` — der Kanal für den spec-verbindlichen Identitätscheck
    (Name **und** Startzeit) zwischen `list-sources` und `capture` existiert
    also in `SourceFactoryError::Open`, der Beleg dafür aber noch nicht;
    `SourceFactory::open_remote`s Doc-Kommentar hält die Lücke jetzt fest,
    damit `#11`/`#12` sie nicht erst beim Bauen entdecken. Beides ist eine
    `core`- bzw. Feinschliff-Änderung, keine, die die Trait-Form dieses
    Issues ändert.
- 2026-07-30: Issue #11 (`audio-win`-Enumeration und Prozessbaum-Auflösung)
  legt fünf Detailentscheidungen fest:
  - `CaptureSubject` (in `core`) bekommt das dritte Feld `started_at:
    SystemTime` — die von #7 offengelassene Lücke wird hier geschlossen, weil
    dieses Issue die einzige Beweisquelle dafür ist (`sysinfo::Process::
    start_time()`). Bewusst `std::time::SystemTime`, kein `sysinfo`-Typ:
    `core` bleibt plattform- und I/O-frei, und `SystemTime` ist bereits die
    Zeit-Repräsentation von `ConsentAttestation::confirmed_at`. `sysinfo`
    liefert Sekunden seit `UNIX_EPOCH` als `u64`; die Umrechnung
    (`UNIX_EPOCH + Duration::from_secs(..)`) passiert ausschließlich in
    `audio-win`s Edge-Schicht (`sources.rs`), nie in `core`. Alle fünf
    bestehenden Aufrufstellen von `CaptureSubject::new` (`audio`s
    `TestToneSources` und deren Tests) sind mit `SystemTime::now()`
    mitgezogen.
  - Partial-Trait-Wahl: **Option (b)** — `WindowsSources: SourceFactory`
    landet bereits in diesem Issue, mit echtem `list_subjects` und
    `open_remote`/`open_local`, die je ein explizites
    `SourceFactoryError::Open { identity, reason: "... (issue #12/#13)" }`
    zurückgeben, nie einen erfolgsförmigen Wert. Grund gegen Option (a): bei
    (a) müssten #12 und #13 unabhängig voneinander denselben Typ
    `WindowsSources` samt `impl SourceFactory` neu anlegen — ein sicherer
    Merge-Konflikt, sollten beide Issues nebenläufig laufen (wie es in
    diesem Projekt für #9/#14 explizit vorgesehen ist). Mit dem Typ bereits
    vorhanden, ersetzen #12 und #13 je nur den Körper *ihrer* Methode — ein
    isolierter, kleiner Diff ohne Überlapp.
  - `identity_still_matches` (Name- und Startzeit-Vergleich, kein
    PID-Vergleich — die Lookup-PID hat #12s künftiger Aufrufer bereits
    verwendet, um die aktuellen Werte zu holen) ist bewusst `pub`, nicht
    `pub(crate)`, obwohl sie in diesem Issue noch **keinen** produktiven
    Aufrufer hat (`open_remote` ist Stub). Grund: `cargo clippy
    --workspace --all-targets` (die von `xtask` genutzte Verify-Form) baut
    die Lib sowohl mit als auch ohne `cfg(test)`; ohne einen Aufrufer außerhalb
    von `#[cfg(test)] mod tests` wäre die Funktion im `cfg(test)`-freien
    Durchlauf `dead_code` und würde die Verify-Gate durch `-D warnings` rot
    machen. `pub`-Elemente sind von diesem Lint ausgenommen, weil eine
    Bibliothek externe Nutzung nicht ausschließen kann — eine ehrliche
    Lösung hier, weil die Funktion tatsächlich als Teil der öffentlichen
    Schnittstelle für #12s `open_remote` gedacht ist, nicht als Umgehung des
    Lints.
  - Die Session-Aufzählung ist zweischichtig: `audio-win::sources` (die
    einzige Stelle mit `wasapi`- und `sysinfo`-Aufrufen) übersetzt
    `wasapi::AudioSessionControl`/`SessionState` und `sysinfo::Process` in
    die plattformneutralen Typen `RenderSession`/`SessionActivity`
    (`sessions.rs`) und `ProcessSnapshot` (`process_tree.rs`), über die die
    drei geforderten gerätefreien Logiktests laufen
    (`process_tree/tests.rs`, `sessions/tests.rs`). `SessionActivity`
    dupliziert `wasapi::SessionState`s drei Varianten unter eigenem Namen,
    statt den Typ direkt zu verwenden, damit `sessions.rs` `wasapi` nicht
    importieren muss.
  - Die Prozessbaum-Auflösung (`resolve_tree_root`) stoppt vor dem Wechsel zu
    einem Eltern-PID in drei Fällen zusätzlich zur Stop-Liste selbst: ein
    Zyklus (Eltern-PID bereits im aktuellen Lauf besucht), ein verwaistes
    Eltern-PID ohne Prozess-Tabellen-Eintrag, und — am Einstieg — ein
    bereits stop-gelisteter Start-PID. Alle drei sind durch je einen
    gerätefreien Test belegt (`walk_terminates_on_a_cycle_...`,
    `walk_stops_at_an_orphaned_parent_...`), nicht nur durch die
    Stop-Namen/PIDs selbst.
- 2026-07-30: Ein Review vor dem Merge (frischer Agent, Opus) deckte einen
  echten Isolations-Fund auf und führte zu vier Nachschärfungen an Issue #11,
  alle vor dem Merge umgesetzt:
  - **Der Fund:** `sessions.rs`s `is_excluded` prüfte nur `own_pid` und die
    Stop-**PIDs** (0/4), nie die Stop-**Namen**. Eine Session, die
    buchstäblich von einem stop-gelisteten Prozess selbst gehalten wird —
    z. B. Systemklänge über einen von `svchost.exe` gehosteten
    Audio-Dienst, ein auf Windows realer Fall — löste sich auf sich selbst
    als Wurzel auf und wurde als Erfassungs-Ziel angeboten, dessen
    Prozessbaum die Shell oder ein Dienst-Host ist, nicht eine Anwendung.
    Genau der Isolationsbruch, den die Stop-Liste verhindern soll, erreicht
    von der anderen Seite. Behoben, indem `is_stop_listed` aus
    `process_tree.rs` `pub(crate)` wird und `is_excluded` sowohl auf die
    rohe Session-PID als auch auf die aufgelöste Wurzel per
    `is_stop_listed` statt der reinen PID-Liste prüft.
  - Zwei Tests waren **vakuos**, belegt durch manuelles Mutationstesten
    (die geprüfte Zeile entfernt, Suite blieb grün):
    `walk_never_crosses_into_a_stop_listed_parent_pid` hatte für die
    Stop-PID selbst keinen Prozess-Tabellen-Eintrag, sodass der
    verwaiste-Eltern-Pfad zufällig dasselbe Ergebnis lieferte wie die
    PID-Stop-Prüfung — behoben durch einen expliziten Eintrag für die
    Stop-PID. `the_callers_own_process_is_excluded_even_if_active` prüfte
    nur den Fall, in dem die rohe Session-PID bereits `own_pid` ist, nie
    den Fall, in dem `own_pid` erst die aufgelöste Wurzel eines fremden
    Kind-PIDs ist — ergänzt um
    `the_callers_own_process_is_excluded_when_it_is_only_the_resolved_root`.
    Dazu ein neuer Test `a_session_owned_by_a_stop_listed_process_produces_no_subject`
    über alle vier Stop-Namen, und `stop_listed_pids_are_excluded_even_if_reported_active`
    läuft jetzt über beide Stop-PIDs statt nur PID 4.
  - `process_snapshots()` (`sources.rs`) rief `sysinfo::ProcessRefreshKind::
    everything()`, obwohl die Funktion nur `name()`, `parent()` und
    `start_time()` liest. Diese drei Felder werden von `sysinfo` beim
    Entdecken eines Prozesses unbedingt befüllt, unabhängig vom Refresh-Kind
    — `everything()` hätte zusätzlich Kommandozeile und Umgebungsblock
    **jedes sichtbaren fremden Prozesses** (auf Windows über
    `ReadProcessMemory` auf dessen PEB) auf den nicht-zeroisierten Heap
    dieses Prozesses gezogen, ungenutzt. Widerspricht der
    Datenminimierung aus `docs/vision.md`. Behoben durch
    `ProcessRefreshKind::nothing()`.
  - Der Doc-Kommentar auf `SourceFactory::open_remote` (`audio::factory`)
    behauptete noch, `CaptureSubject` trage keine Startzeit — durch dieses
    Issue nicht mehr wahr. Aktualisiert auf einen Verweis auf
    `CaptureSubject::started_at` und `identity_still_matches`, damit #12
    keine bereits überflüssige Umgehung baut.
- 2026-07-30: Issue #9 (`session`-Zustandsmaschine mit typseitigem
  Consent-Gate) legt sechs Detailentscheidungen fest:
  - `CapturePlan` trägt `subject: CaptureSubject`, `remote: Box<dyn
    AudioSource>` (obligatorisch — `open_remote` liefert nie eine Absenz)
    und `local: Option<Box<dyn AudioSource>>`, exakt `docs/architecture.md`s
    Flow 1 ("die geöffneten `AudioSource`s (Mikrofon und
    Prozessbaum-Loopback)"). `Session` besitzt pro Strom einen eigenen
    Capture-Thread und einen `Arc<Mutex<RingBuffer>>`; eine Quelle
    *produziert* nur über `pull`, ohne eigene Pufferung — die
    Eigentums-Grenze aus der Issue-Vorgabe.
  - Die gemeinsame Sitzungs-Null (`SessionZero`, in `clock.rs`) ist ein
    `Instant`, den `Session::start` einmal aufzeichnet. Jeder Capture-Thread
    verankert beim Start einen eigenen `StreamClock` darauf: die seit der
    Sitzungs-Null vergangene Wanduhrzeit im Moment des Thread-Starts, plus —
    ab dem ersten Frame — der Geräte-Tick-Nullpunkt dieses Stroms. Spätere
    Frames normalisieren sich als Wanduhr-Anker plus vergangene Geräte-Ticks.
    Damit tragen `Remote` und `Local` trotz unabhängiger Geräte-Uhren eine
    vergleichbare, auf dieselbe Null bezogene Zeitachse — belegt durch einen
    Test, der zwei zu unterschiedlichen Zeitpunkten verankerte Uhren für
    denselben Geräte-Tick unterschiedliche, monoton größere Offsets liefern
    lässt. `Session::elapsed(identity)` legt das Ergebnis offen (`None` nur
    für `Local` ohne Mikrofon).
  - `RingBuffer` (Crate `audio`) bekommt zwei neue Methoden, `zeroize()`
    (füllt die Storage in-place mit `0.0`, keine Reallokation) und
    `all_zero()` (reine Abfrage, gibt nie Samples zurück). Nötig, weil die
    Akzeptanz dieses Issues explizit maschinell beweisen muss, dass nach
    `stop()` kein Sample mehr auffindbar ist — `Zeroizing`s Nullen-bei-Drop
    allein hätte das nicht *während* die `Session` noch erreichbar ist,
    belegt. Eine crate-übergreifende Änderung, hier bewusst offengelegt statt
    stillschweigend vollzogen; `audio` bleibt sonst unverändert.
  - `Session::stop()` durchläuft real beide Kanten
    (`request_stop` dann `end`) statt sie zu verschmelzen — ein Aufrufer ohne
    Interesse am `Stopping`-Zwischenzustand bekommt die Bequemlichkeit einer
    Methode, aber die Zustandsmaschine bleibt zweikantig und einzeln
    aufrufbar (`request_stop`/`end` sind beide `pub`).
  - Die Capture-Loop prüft `stop` **nach** jedem `pull`, nie davor: ein
    Review-Fund während der Implementierung zeigte, dass ein `stop`, der die
    Thread-Terminierung im Wettlauf mit deren allererster Ausführung
    schlägt, sonst einen Strom mit bereits bereitstehenden Daten (die
    Testton-Quelle mit `with_total_frames`) ganz ohne einen einzigen `pull`
    beenden konnte — von außen ununterscheidbar von einer erfolgreich
    genullten Sitzung, weil beide Male der Puffer nur Nullen zeigt. Behoben,
    indem jede Iteration mindestens einen `pull` ausführt, bevor `stop`
    geprüft wird.
  - Der `trybuild`-`compile_fail`-Fall reicht bewusst `()` statt eines
    fehlenden zweiten Arguments an `Session::start` — ein Typfehler (E0308)
    statt eines Arity-Fehlers (E0061). E0061 trägt seit einigen
    Rust-Versionen einen mehrzeiligen `help: provide the argument`-Block mit
    Platzhalter-Kommentar, dessen genauer Wortlaut jünger und damit
    wandelbarer ist als die knappe "expected X, found Y"-Form von E0308.
    `tests/compile_fail/session_requires_consent_attestation.stderr` ist
    eingecheckt (ohne eine Datei akzeptiert `trybuild` den Fall nur als
    "wip" und schlägt fehl); ein künftiger Toolchain-Bump kann sie trotzdem
    neu erzeugen müssen — der akzeptierte, in der ganzen `trybuild`-Literatur
    übliche Preis fest gepinnter Compiler-Diagnosen.
  - Zurückgestellt, kein Merge-Blocker: `Session` legt keine
    Pro-Strom-Statistik (Verlustzahl, Discontinuity-/Timestamp-Error-Zähler)
    offen und keinen Leser-Zugriff auf den Ringpuffer — die Spec-Akzeptanz
    dieses Issues nennt das nicht, und das nächste Issue (`cli`,
    „Pro-Strom-Statistik") entscheidet mit eigenem Kontext, welche Form der
    Leser-Zugriff dafür braucht, statt dass dieses Issue rät.
  - Ein Review vor dem Merge (frischer Agent, Opus) deckte zwei echte
    Lebenszyklus-Fehler auf, beide vor dem Merge behoben:
    - **Der Fund:** `StreamCapture` hatte keinen `Drop`-Impl. Eine `Session`,
      die ohne `stop()` fallen gelassen wird — ein Panic zwischen `start` und
      `stop`, ein früher `?`-Rückgabepfad eines Aufrufers — ließ den
      Capture-Thread unentwegt weiterlaufen (die `Arc`s hielten `RingBuffer`
      am Leben) und nullte nie. Belegt durch einen reproduzierten Lauf: 12
      auf 40 gepullte Frames 150 ms nach dem Fallenlassen, in einem Puffer,
      den keine API mehr erreichte. Behoben durch `impl Drop for
      StreamCapture`, das `signal_stop` → `join` → `zeroize` idempotent
      nachzieht — belegt durch
      `capture::tests::dropping_without_stop_still_stops_the_thread_and_zeroes_the_buffer`
      mit einer *unbegrenzten* Testton-Quelle, damit nur `Drop`, nie ein
      erschöpfter Frame-Vorrat, für das Thread-Ende verantwortlich sein
      kann.
    - **Der Fund:** `Session::request_stop` signalisierte und jointe jeden
      Strom **sequentiell** über `?` — panicte `remote`s Thread, kehrte die
      Methode zurück, **bevor** `local`s Stop-Flag je gesetzt wurde. Der
      Zustand steht dann bereits auf `Stopping` (nicht wiederholbar über
      `request_stop`), aber `end()` wird akzeptiert und nullt einen Puffer,
      in den `local` noch aktiv schreibt — die Akzeptanz-Zusage „nach `stop`
      ist kein Sample mehr auffindbar" gilt auf diesem Pfad nicht. Belegt
      durch einen reproduzierten Lauf: lokaler Pull-Zähler 19 → 47 in
      150 ms nach einem `request_stop`, der bereits `Err` zurückgegeben
      hatte. Behoben durch Aufspalten von `StreamCapture::request_stop_and_join`
      in `signal_stop` (kann nicht fehlschlagen) und `join`
      (blockierend, fehlerbehaftet): `Session::request_stop` signalisiert
      jetzt **jeden** Strom, bevor es **irgendeinen** joint, und meldet den
      ersten Fehler erst, nachdem alle Joins gelaufen sind — belegt durch
      `session::tests::request_stop_still_stops_local_even_when_remote_panics`
      (eine panische `Remote`-Quelle, ein unbegrenzter `Local`-Strom, dessen
      `elapsed()` sich nachweislich nicht mehr ändert, sobald `request_stop`
      zurückkehrt). Nebenbefund: dieselbe Umstellung entfernt die zuvor
      sequentielle Stop-Latenz von bis zu `2 × PULL_TIMEOUT` auf einen
      parallel signalisierten, nur noch einmal seriell gejointen Ablauf.
    - Zwei weitere, kleinere Korrekturen aus derselben Runde: die
      Tick-Arithmetik in `clock.rs::normalize` nutzt jetzt
      `saturating_sub`/`saturating_mul` statt ungeprüfter `i64`-Subtraktion
      auf geräteseitig gelieferten Werten (erreichbar außerhalb von Tests);
      und ein vergifteter Ring-Mutex in `capture_loop` meldet jetzt
      `SessionEvent::StreamFailed`, statt den Thread stillschweigend zu
      beenden — die beiden anderen Exit-Pfade der Schleife meldeten immer
      schon ein Ereignis, dieser tat es nicht.
    - Zwei Doc-Kommentare behaupteten etwas, was der Code nicht (mehr) tat:
      `tests/consent_gate.rs` behauptete, es gebe bewusst **keine**
      `.stderr`-Datei — es gibt eine, `trybuild` akzeptiert `compile_fail`
      ohne sie nur als „wip" und schlägt fehl. Und die Behauptung, die
      Konstruktionszeilen der `compile_fail`-Fixture seien mit
      `tests/session_lifecycle.rs` „wortgleich" geteilt, war falsch — beide
      Dateien enthielten unabhängige Kopien, die Fixture zusätzlich mit
      `.unwrap()` statt des sonst durchgehaltenen `let ... else { panic!()
      }`-Musters (dort kein Clippy-Verstoß, weil `tests/compile_fail/` kein
      eigenes Test-Target ist, aber ein Bruch der Konvention). Beide
      Kommentare korrigiert, die Fixture auf `let-else` umgestellt.
  - `session` steht nicht in `xtask::audit::GUARDED_CRATES` (nur `audio`,
    `audio-win`, `asr`, `diarize` — die Liste, die `docs/constitution.md`
    beim Namen nennt). Bewusst nicht in diesem Issue ergänzt: die Spec
    dieser Phase listet den Audit-Test nicht unter den Akzeptanzkriterien
    dieses Issues, und `session` hat heute keinen Schreibpfad. Festgehalten
    als offene Frage für die Planung, nicht stillschweigend entschieden:
    `session` besitzt ab jetzt jeden Ringpuffer mit live-PCM und verdient
    denselben Audit-Schutz wie die vier gelisteten Crates.
- 2026-07-30: Eine zweite Review-Runde (frischer Agent, Opus) bestätigte die
  beiden Fixes der ersten Runde als echt und wirksam (per Falsifikation:
  jeweils der alte Code zurückgesetzt, der zugehörige Regressionstest
  schlug reproduzierbar fehl), deckte aber zwei weitere Punkte auf, beide
  vor dem Merge behoben:
  - **Der Fund:** `session.rs` war durch die neuen Regressionstests der
    ersten Runde auf 407 Zeilen gewachsen — über die 400-Zeilen-Grenze der
    Constitution, von keinem Clippy-Lint erfasst (`too_many_lines` misst nur
    Funktionen). Behoben nach dem im Repo etablierten Muster
    (`crates/audio/src/test_tone.rs` + `test_tone/tests.rs`): das
    Testmodul wandert nach `crates/session/src/session/tests.rs`,
    `session.rs` behält nur `#[cfg(test)] mod tests;`. Direkt danach 286
    bzw. 163 Zeilen — spätere Regressionstests (Runden 3 und 4) lassen
    beide seither weiterwachsen, bleiben aber unter der Grenze.
  - **Der Fund:** Der `Drop`-Fix der ersten Runde behob den unbegrenzten
    Fall (Bug 2), führte aber eine **begrenzte** Version derselben
    Fehlerform am Abbruchpfad wieder ein: `Session` selbst hatte keinen
    eigenen `Drop`, also lief Rusts feldweise Reihenfolge (`remote` vor
    `local`) — `remote`s `StreamCapture::drop` signalisiert **und** jointe
    blockierend (bis zu `PULL_TIMEOUT`), bevor `local`s Stop-Flag je gesetzt
    wurde. Behoben durch drei geteilte private Methoden
    (`signal_all_streams_to_stop`, `join_all_streams`, `zero_all_streams`),
    die `request_stop`/`end` jetzt nutzen, plus ein neues `impl Drop for
    Session`, das dieselbe Reihenfolge (signalisieren, dann erst joinen,
    dann nullen) am Abbruchpfad nachzieht — idempotent, weil die
    anschließend automatisch laufenden feldweisen `Drop`-Aufrufe dann nichts
    mehr vorfinden.
  - Beide Korrekturen zusammen deckten einen dritten, echten Fehler auf, den
    keine der beiden Review-Runden fand, sondern ein flackernder Testlauf
    (1 von 5 lokalen Wiederholungen): `capture_loop` aktualisierte
    `position` **vor** dem Schreiben in `ring`, sodass ein Leser, der einen
    neuen, von Null verschiedenen `elapsed()`-Wert sah, den zugehörigen
    Puffereintrag noch nicht zwingend vorfand — kein Sicherheitsproblem für
    die produktive Nutzung von `elapsed()` allein, aber ein echtes
    Race in genau der Prüfung, die der Drop-Regressionstest braucht
    (Ringpuffer real beschrieben, *bevor* gedroppt wird). Behoben durch
    Vertauschen der Reihenfolge (`ring` zuerst, `position` danach) — die
    beiden `Mutex`e machen daraus eine echte Happens-before-Garantie
    (Freigabe von `ring` ist sequenced-before dem Erwerb von `position`
    im selben Thread; Freigabe von `position` synchronisiert sich mit
    jedem späteren Erwerb durch einen Leser), nicht nur eine meist
    zutreffende Reihenfolge. Belegt durch 30 wiederholte Läufe des zuvor
    flackernden Tests ohne einen weiteren Fehlschlag.
- 2026-07-30: Eine dritte Review-Runde (frischer Agent, Opus) bestätigte
  alle drei Korrekturen der zweiten Runde per Falsifikation (jeweils
  zurückgesetzt, Fehlschlag reproduziert, wieder hergestellt — u. a. 240
  Läufe des Ringpuffer-Race-Tests unter 16-facher Parallelität, 5 Treffer
  ohne die Korrektur, 0 mit ihr), fand aber eine echte Lücke: `impl Drop
  for Session` hatte **keinen eigenen** Regressionstest — mit leerem
  `drop`-Rumpf blieb die gesamte Suite grün, weil Rounds 1s Test
  (`capture::tests::dropping_without_stop_...`) nur `StreamCapture` prüft,
  nicht die `Session`-Ebene, auf der Round 2s Fund saß. Genau die
  Lücke, durch die der Fund selbst erst entstand — unbeobachtet bliebe er
  bei einem künftigen „Aufräumen, `StreamCapture` droppt sich doch schon
  selbst"-Commit wieder. Behoben durch
  `session::tests::dropping_a_session_signals_every_stream_before_joining_any`:
  ein `Remote`, dessen `pull` 250 ms blockiert (`SlowRemote`), und ein
  `Local`, das seine `pull`-Aufrufe zählt (`CountingLocal`) — fällt
  `local`s Stop-Signal erst nach `remote`s blockierendem Join, klettert der
  Zähler während der 250 ms um Tausende (per Falsifikation belegt: 4455–4976
  bei leerem `drop`-Rumpf über drei Läufe), mit der Korrektur um 0. Dazu
  eine kleine Ungenauigkeit in einem Kommentar in `capture.rs` korrigiert
  (die Happens-before-Kette bezieht sich auf die *Freigabe*, nicht den
  *Erwerb*, von `position`, und gilt nur, solange ein Leser dieselbe
  Reihenfolge einhält — jetzt so benannt).
- 2026-07-30: Eine vierte Review-Runde (frischer Agent, Opus) prüfte die
  gesamte PR noch einmal von vorn, nicht nur den letzten Patch, und deckte
  einen Fund auf, den keine der drei vorherigen Runden sah: **zwei der
  fünf `SessionEvent`-Varianten waren strukturell unzustellbar.**
  `MicrophoneUnavailable` und `StreamDegraded` werden ausschließlich
  innerhalb von `Session::start` gesendet — bevor `start` zurückkehrt und
  damit bevor irgendein Aufrufer `Session::subscribe` überhaupt hätte
  rufen können. `tokio::sync::broadcast` puffert nie für einen Empfänger,
  der erst **nach** einem `send` entsteht; der ursprüngliche Empfänger aus
  `broadcast::channel(..)` wurde in `start` sofort verworfen
  (`let (events, _receiver) = ...`). Belegt durch eine Probe des Reviewers:
  ein Abonnent direkt nach `start` sah eine leere Ereignisliste. Drei
  Doc-Kommentare (auf `Session::start`, `Session::start_local` und
  `SessionEvent::MicrophoneUnavailable` selbst) behaupteten das Gegenteil —
  wieder die Doc-Drift-Klasse, die diese PR schon zweimal korrigiert hat,
  dieses Mal aber am Verhalten selbst, nicht nur am Kommentar. Kein
  Test deckte die Zustellung ab, nur die Zustands-Accessor (`has_local_stream`,
  `*_degradation`), was den Fund unsichtbar hielt.

  Von den drei vom Reviewer vorgeschlagenen Optionen (Startempfänger
  aufheben und dem ersten Abonnenten geben; die beiden Sends aus `start`
  herausziehen und erst bei der ersten Anmeldung nachliefern; die beiden
  Varianten ganz entfernen und auf die längst vorhandenen, längst
  getesteten Accessor verweisen) gewählt: **die erste.** `Session` trägt
  jetzt `startup_receiver: Option<broadcast::Receiver<SessionEvent>>`, mit
  dem in `start` erzeugten Empfänger befüllt; `subscribe(&mut self)`
  (vorher `&self` — die Mutation, um ihn per `Option::take` zu entnehmen,
  ist die einzige Signaturänderung) gibt ihn beim ersten Aufruf zurück und
  fällt danach auf ein gewöhnliches `events.subscribe()` zurück. Gewählt
  statt der beiden anderen Optionen, weil sie ohne neue Interna am
  bestehenden `broadcast`-Kanal auskommt und die beiden Varianten als
  eigenständige, dem Nutzer sichtbare Ereignisse erhält (Variante 3 hätte
  das Vokabular verkleinert, das `docs/design.md`s künftiger
  Konsent-/Status-Anzeige eventuell nützt). Belegt durch
  `session::tests::the_first_subscriber_still_sees_an_event_published_during_start`
  (per Falsifikation: mit der naiven `events.subscribe()`-Implementierung
  schlägt der Test zuverlässig fehl, mit der Korrektur nicht mehr) — der
  bestehende Test `a_missing_microphone_warns_instead_of_aborting` prüfte
  weiterhin nur die Accessor, deckte die Zustellungslücke deshalb nie auf.
- 2026-07-30: Issue #12 (`audio-win`-Loopback-Erfassung, `open_remote`) legt
  eine Entscheidung fest, die kein am 2026-07-30 geprüfter `wasapi`-Quelltext
  aufgedeckt hatte, weil sie nicht die Sicherheit eines einzelnen Aufrufs
  betrifft, sondern die Komposition der zurückgegebenen Typen mit einem
  bereits fixierten Trait aus einer anderen Crate:
  - **Der Fund:** `wasapi::AudioClient`, `AudioCaptureClient` und `Handle`
    kapseln je einen rohen COM- bzw. Win32-Zeiger
    (`windows_core::IUnknown`s `NonNull<c_void>`, `HANDLE`s `*mut c_void`)
    ohne `Send`-Implementierung — geprüft nicht durch Lektüre, sondern durch
    den tatsächlichen Compiler-Fehler beim ersten Bauversuch
    (`` `NonNull<c_void>` cannot be sent between threads safely ``). `audio`s
    `AudioSource: Send` (Issue #7, bereits gemergt, nicht Gegenstand dieses
    Issues) verlangt aber `Send` von jedem Typ, der das Trait implementiert
    — unabhängig davon, ob eine konkrete Sitzung die Quelle je über einen
    Thread hinweg bewegt. Ohne Gegenmaßnahme bräuchte ein direktes Halten
    dieser drei Felder in einem `LoopbackSource`-Typ ein
    `unsafe impl Send for LoopbackSource {}` — genau die Ausnahme, die
    `#![forbid(unsafe_code)]` in `audio-win` verbietet und die laut Auftrag
    dieses Issues eskaliert werden muss, statt sie selbst zu ziehen.
  - **Die Auflösung, ohne die Ausnahme zu ziehen:** die drei nicht-`Send`
    WASAPI-Objekte leben vollständig auf einem einzigen, von
    `LoopbackSource::open` gestarteten Worker-Thread (`sources::
    loopback_client::run_worker`) und verlassen ihn nie — sie werden dort
    erzeugt, dort gelesen, dort fallengelassen. `LoopbackSource` selbst
    (der Typ, der `AudioSource` implementiert und die Trait-Grenze
    überschreitet) hält nur noch Kanal-Enden
    (`SyncSender<Duration>`/`Receiver<Result<AudioSourceEvent, String>>`)
    und reine Daten — beides `Send` aus eigenem Recht, ganz ohne
    `unsafe`. `pull` schickt seinen `timeout` über einen
    Rendezvous-Kanal (`sync_channel(0)`, kein Puffer über ein Element
    hinaus) an den Worker und blockiert auf die Antwort; das hält die
    „kein unbegrenzter Puffer"-Zusage dieser Phase auch über die
    Thread-Grenze hinweg ein, nicht nur innerhalb eines `pull`-Aufrufs.
  - Diese Form macht `initialize_mta() runs on each capture thread`
    (Akzeptanzkriterium dieses Issues) strukturell wahr, statt es durch
    einen defensiven Aufruf in jedem `pull()` zu erzwingen: der Worker-Thread
    *ist* jetzt der Erfassungs-Thread, lebt für die gesamte Lebensdauer der
    Quelle, und ruft `initialize_mta()` genau einmal, an seinem Anfang, vor
    jedem weiteren WASAPI-Aufruf.
  - Beim Sitzungsende (`Drop for LoopbackSource`) wird zuerst der
    Request-Sender auf `None` gesetzt (schließt den Kanal, der Worker
    verlässt seine `recv()`-Schleife) und danach der Thread über
    `JoinHandle::join` eingesammelt — in dieser Reihenfolge, sonst blockiert
    `join()` auf einem Worker, der auf eine Nachricht wartet, die nie kommt.
    Begrenzt durch das jeweils laufende `pull`-Timeout, falls der Worker
    genau in `wait_for_event` hängt; nie unbegrenzt, weil dieser Aufruf selbst
    immer spätestens nach seinem Timeout zurückkehrt.
  - `get_next_packet_size()` bleibt die einzige Quelle der Chunk-Größe,
    `get_buffer_size()` wird nirgends aufgerufen; alle drei `BufferFlags`
    werden über die reinen Funktionen in `crate::loopback`
    (`frame_for_packet`, mit `data_discontinuity` vor `timestamp_error`
    geprüft, falls beide je gleichzeitig gesetzt wären) auf getrennte
    Zähler abgebildet, `silent` wird unabhängig vom tatsächlichen
    Puffer-Inhalt als Nullen materialisiert. Diese Zuordnung, die
    Byte-Dekodierung (`bytes_to_f32_samples`) und die Timeout-Umrechnung
    (`timeout_millis`) sind das gerätefreie Kernstück dieses Issues und
    tragen alle Logiktests; die WASAPI-Kanten (`OpenClient`, `run_worker`,
    `LoopbackSource`) bekommen wie von der Spec vorgesehen nur
    Compile-Prüfung.
  - `sources.rs` wurde aufgeteilt: die WASAPI-Kante für die
    Loopback-Erfassung (`LoopbackSource`, `OpenClient`, `run_worker`) liegt
    jetzt in `sources/loopback_client.rs`, damit beide Dateien unter der
    400-Zeilen-Grenze der Constitution bleiben — dieselbe Begründung, die
    Issue #8 schon für den Audit-Test dokumentiert hat. Weiterhin **eine**
    Erfassungs-Kante im Sinn der Architektur, nur auf zwei Dateien verteilt.
  - `identity_still_matches` (Issue #11) ist jetzt über
    `verify_subject_identity` (`sources.rs`) tatsächlich verdrahtet: ein
    frischer `sysinfo`-Scan zum Zeitpunkt von `open_remote`, verglichen
    gegen `subject`; ein fehlender Prozess an der gemerkten `root_pid` zählt
    als Mismatch, nicht als Sonderfall — es gibt nichts mehr zu
    identifizieren. `open_local`s Stub bleibt unverändert (Issue #13).
- 2026-07-30: Issue #13 (`audio-win`-Mikrofon-Erfassung mit
  betriebssystemseitiger Echokompensation, `open_local`) legt drei
  Detailentscheidungen fest, die die Spec offen ließ:
  - **Wiederverwendung statt Divergenz:** `MicrophoneSource`
    (`sources/microphone_client.rs`) übernimmt Issue #12s
    Worker-Thread-Muster unverändert (derselbe Grund: `AudioClient`,
    `AudioCaptureClient` und `Handle` sind nicht `Send`,
    `#![forbid(unsafe_code)]` bleibt gezogen) und importiert
    `crate::loopback`s reine Paket-Logik (`bytes_to_f32_samples`,
    `frame_for_packet`, `timeout_millis`, `PacketFlags`, `GapCounters`)
    direkt, statt sie zu duplizieren — Byte-Dekodierung, Lücken-Zählung und
    Timeout-Umrechnung sind für einen gewöhnlichen Erfassungs-Client
    identisch zum Loopback-Client. Neu und mikrofon-eigen sind nur die
    Geräte-Auflösung und die AEC-Kette, in einem eigenen reinen Modul
    `crate::microphone` (`aec_degradation`, `microphone_absent`) nach
    demselben Muster wie `crate::loopback`: frei von `wasapi`/`windows`,
    damit beide auf einem geräte­losen CI-Runner testbar bleiben. Das
    Modul `crate::loopback` wird nicht umbenannt — die Umbenennung auf
    einen generischeren Namen hätte Issue #12s bereits gemergten Code
    berührt, ohne dass dieses Issue das bräuchte. Die WASAPI-Kante selbst ist auf
    zwei Dateien verteilt (`sources/microphone_client.rs` und
    `sources/microphone_client/open_client.rs`), aus demselben
    400-Zeilen-Grund, den Issue #12 schon für `sources.rs` dokumentiert hat.
  - **Die Abwesenheits-Erkennung ist ein Integer-Vergleich, kein
    `wasapi`-Typ-Vergleich:** `microphone_absent(hresult: i32)` vergleicht
    gegen `0x8007_0490` (`HRESULT_FROM_WIN32(ERROR_NOT_FOUND)`), das
    dokumentierte `E_NOTFOUND`, das `IMMDeviceEnumerator::
    GetDefaultAudioEndpoint` liefert, wenn kein Gerät der angefragten
    Rolle/Richtung existiert. Rechnerisch aus der Win32-HRESULT-Formel
    hergeleitet und im Test (`FACILITY_WIN32 << 16 | 0x8000_0000 |
    ERROR_NOT_FOUND`) unabhängig von der Implementierungs-Konstante neu
    zusammengesetzt, nicht nur derselbe Literal doppelt hingeschrieben —
    ein Review vor dem Merge deckte auf, dass eine erste Testfassung genau
    das tat und damit eine vertauschte Ziffer in der Konstante nie hätte
    auffangen können. Nicht empirisch an einem Rechner ohne Mikrofon
    beobachtet — dafür gibt es in dieser Umgebung kein Gerät, das sich
    sitzungsweise abstecken ließe. Ein zweiter Test belegt, dass ein
    anderer HRESULT-Wert (`E_ACCESSDENIED`) nicht als Abwesenheit
    missverstanden wird, sondern als echter Fehler durchschlägt; die
    Windows-Mikrofon-Datenschutzeinstellung selbst durchläuft diesen
    Vergleich in der Praxis gar nicht — sie schlägt erst später, an
    `IAudioClient::Initialize`, fehl und damit ohnehin als gewöhnlicher
    `Err` in `OpenClient::open`.
  - **Ein Fehler in der AEC-Kette nach einer `true`-Probe wird nicht als
    zweite Degradierung gefaltet, sondern als echter Fehler behandelt:**
    Die Spec entscheidet „`is_aec_supported() == false`" → warnen und
    weiterlaufen. Schlägt danach `get_aec_control()` oder
    `set_echo_cancellation_render_endpoint` fehl, ist das eine
    Inkonsistenz, die die Spec nicht adressiert, kein zweiter Fall von
    „AEC fehlt". Bewusst als `Err` durchgereicht statt stillschweigend zu
    `EchoCancellationUnavailable` heruntergestuft, damit ein echtes
    Plattformproblem sichtbar bleibt statt hinter einer normal aussehenden
    Degradierung zu verschwinden.
- 2026-07-30: Ein Review vor dem Merge (frischer Agent, Opus) deckte zwei
  echte Fehler in der AEC-Kette auf, beide vor dem Merge behoben, plus die
  oben schon eingearbeitete Test-Schwäche:
  - **Der erste Fund:** Der ursprüngliche Entwurf löste den
    Referenz-Wiedergabegerät für `set_echo_cancellation_render_endpoint`
    über `enumerator.get_default_device(&Direction::Render)` auf — das
    `Console`-Rollen-Standardgerät, exakt wie im geprüften `wasapi`-Beispiel
    (`examples/aec.rs`). Aber dieselbe Funktion wählt auf der Aufnahmeseite
    bewusst `Role::Communications`, mit der Begründung „das Gerät, das der
    Call-Client selbst benutzt" (Issue-Text). Dieselbe Begründung gilt für
    die Wiedergabeseite genauso, und es gibt keine Zusicherung, dass beide
    Rollen auf dasselbe physische Gerät zeigen (z. B. Desktop-Lautsprecher
    als Communications-Standard, ein zweiter DAC als Console-Standard).
    Zeigt die AEC-Referenz auf das falsche Gerät, kompensiert sie nichts,
    während `is_aec_supported()` weiterhin `true` meldet und
    `degradation()` `None` bleibt — der Nutzer bekäme keine Warnung für
    genau den Fall, den dieses Issue lösen soll. Behoben, indem
    `enable_echo_cancellation` `None` an
    `set_echo_cancellation_render_endpoint` übergibt, statt selbst ein
    Gerät zu wählen — laut `wasapi`-Doc-Kommentar wählt Windows dann selbst
    das Referenzgerät.
  - **Der zweite, unabhängige Fund, der dieselbe Korrektur zusätzlich
    erzwingt:** `wasapi` 0.23.0s eigene
    `set_echo_cancellation_render_endpoint`-Implementierung
    (`src/api.rs:1898-1912`) baut im `Some(id)`-Zweig ein `HSTRING` aus der
    übergebenen `String`, nimmt per `.as_ptr()` einen rohen `PCWSTR`
    heraus und übergibt ihn erst in der **nächsten** Anweisung an den
    COM-Aufruf — das `HSTRING` selbst ist zu diesem Zeitpunkt aber bereits
    freigegeben (Rusts Temporary-Scope-Regel verlängert seine Lebensdauer
    nicht über `.as_ptr()` hinweg). Ein waschechter Dangling-Pointer-Bug in
    der gepinnten Abhängigkeit selbst, nur auf dem `Some`-Zweig erreichbar
    — nicht durch Lektüre vermutet, sondern am Quelltext nachvollzogen.
    Dieselbe Korrektur wie oben (immer `None` übergeben) umgeht ihn
    vollständig, weil dieser Zweig dann nie gebaut wird. Im
    Doc-Kommentar von `enable_echo_cancellation` festgehalten, direkt neben
    der Versions-Pinnung auf `0.23.x`, damit ein künftiger Versions-Bump
    prüft, ob der Bug behoben wurde, statt ihn erneut zu entdecken.
- 2026-07-30: Issue #10 (gerätefreier Sitzungs-FS-Audit) legt die Kindprozess-Route
  und die Vakuität-Wächter fest:
  - **Route:** ein zusätzliches `[[bin]]`-Target, `crates/session/src/bin/
    fs_audit_child.rs` — von Cargo automatisch als Binärziel `fs_audit_child`
    erkannt, ohne eine Änderung an `Cargo.toml`. Der Parent-Test
    (`crates/session/tests/fs_audit.rs`) startet es über
    `env!("CARGO_BIN_EXE_fs_audit_child")`, den Pfad, den Cargo für
    Integrationstests **desselben** Pakets automatisch setzt, sobald es das
    Binärziel gebaut hat — kein `cargo run` als Unterprozess, kein
    String-Pfad-Raten. Gegenüber den beiden Alternativen: ein `examples/`-Binary
    hätte keinen äquivalenten, offiziell zugesicherten `CARGO_..._EXE_`-Pfad
    (Cargo dokumentiert das Env-Var nur für `[[bin]]`-Ziele); den Test-Binary
    selbst mit einer Marker-Variable wiederzuverwenden hätte `trybuild`s eigene
    `compile_fail`-Fixtures und die übrigen `session`-Tests in denselben
    Prozessraum wie die FS-Beobachtung gezogen, ohne einen Vorteil zu bieten.
    Der Kindprozess führt eine **ganze** Sitzung: `TestToneSources` mit
    `with_total_frames`, `Session::start`, 100 ms Erfassungsfenster, dann
    `Session::stop()` — nicht nur Start und sofortiges Beenden. (Der
    Erfassungsfenster-Wert wurde in der Review-Runde unten von 200 ms auf
    100 ms gesenkt; dieser Absatz nennt den aktuellen Stand.)
  - **Kein neuer Parameter auf `Session::start`.** Das „frische Arbeitsverzeichnis,
    das die Sitzung bekommt" wird durch `Command::current_dir` auf den
    Kindprozess realisiert, nicht durch eine Signaturänderung an `Session`
    oder `CapturePlan` — beide sind über mehrere Review-Runden (#9) stabilisiert
    und dieses Issue ändert keine ihrer Zeilen. „Prozess-eigen" heißt hier:
    eigen für den Kindprozess, dessen einzige Aufgabe die eine Sitzung ist.
  - **Wächter gegen einen vakuosen Durchlauf**, in der Reihenfolge der
    Vorgabe:
    1. *Der Kindprozess muss echte Arbeit geleistet haben.* Jede der beiden
       Quellen wird in einen zählenden `AudioSource`-Wrapper
       (`CountingSource`) gehüllt, der jedes tatsächlich gepullte
       `AudioSourceEvent::Frame` zählt; der Kindprozess druckt
       `remote_frames=<n>` / `local_frames=<n>` auf stdout, der Parent-Test
       parst beide Zeilen und verlangt `n > 0` für **beide** Ströme — nicht
       nur den Exit-Code. Eine Sitzung, die startet, aber nichts pullt,
       hätte sonst „0 neue Dateien" **und** Exit-Code 0 vorgetäuscht (der
       `fail()`-Pfad selbst beendet immer mit Code 2, deckt diesen Fall
       also nicht ab); mit dem Frame-Zähler scheitert sie stattdessen laut.
    2. *Rekursiv, nicht nur oberste Ebene.* `snapshot()` steigt in jedes
       Unterverzeichnis ab. Durch Mutationstest belegt: mit der Rekursion
       stillgelegt schlägt `detects_a_file_written_into_either_watched_directory`
       reproduzierbar fehl (`expected exactly the nested evidence file, got
       []`), mit ihr wieder grün.
    3. *Während der Sitzung beobachten, nicht nur davor/danach.* Der
       Parent-Test nimmt eine Aufnahme, **bevor** der Kindprozess gestartet
       wird, pollt danach beide Verzeichnisse alle 2 ms, solange er läuft
       (`Child::try_wait`), und nimmt eine letzte Aufnahme nach dem Exit.
       Das erhöht die Chance, eine Datei zu sehen, die während der Sitzung
       geschrieben und vor deren Ende wieder gelöscht wird, gegenüber einem
       reinen Vorher/Nachher-Paar erheblich — ist aber **keine** Garantie für
       jede denkbare Dauer einer solchen Datei; die Review-Runde unten
       präzisiert das.
    4. *Der Detektor erkennt tatsächlich etwas.* Der zweite Test
       (`detects_a_file_written_into_either_watched_directory`) schreibt
       direkt (nicht über den Kindprozess) je eine Datei in eine verschachtelte
       Unterstruktur des Arbeitsverzeichnisses und in das Temp-Verzeichnis und
       verlangt, dass genau diese eine Datei als neu erscheint — dieselbe
       `snapshot`/Differenz-Funktion, die der Haupttest auf „leer" prüft, wird
       hier auf „nicht leer" geprüft.
    5. *Kein stillschweigend verschlucktes Scheitern.* Jeder Fehlerpfad im
       Kindprozess (`fail()`) druckt auf stderr und beendet mit Exit-Code 2,
       nie mit Panik im Erfassungs-Thread; der Parent-Test hängt das
       Kind-stderr an jede fehlgeschlagene Status-Assertion an.
  - **`TMP`/`TEMP`/`TMPDIR`** werden ausschließlich über `Command::env` auf
    den Kindprozess gesetzt, nie im laufenden Testprozess selbst
    (`std::env::set_var` ist in Edition 2024 `unsafe`, `forbid(unsafe_code)`
    gilt auch für Testcode). Das beobachtete Verzeichnis ist eine frisch
    erzeugte, eindeutig benannte Unterstruktur unter dem geteilten
    System-Temp-Wurzelverzeichnis (`std::env::temp_dir()` **im Parent-Prozess**
    aufgerufen, dessen eigene `TMP`/`TEMP` unverändert bleiben) — nie das
    geteilte Wurzelverzeichnis selbst, das Cargo und fremde Prozesse
    beschreiben und das den Audit sonst sporadisch rot gemacht hätte
    (spec's Begründung). `TMPDIR` wird zusätzlich gesetzt, ohne dass es die
    Windows-CI dieser Phase braucht — eine kleine Portabilitätsreserve, falls
    dieser Test je auf einer anderen Zielplattform läuft.
  - **Aufräumen:** ausschließlich `fs::remove_dir_all` auf genau die beiden
    Pfade, die dieselbe Testfunktion zuvor selbst über `fresh_dir()` angelegt
    hat — nie auf das geteilte Temp-Wurzelverzeichnis. Läuft auch auf dem
    Timeout- und dem Poll-Fehlerpfad, nicht nur beim Normalpfad.
  - Zurückgestellt, keine Entscheidung dieses Issues: ob `session` in
    `xtask::audit::GUARDED_CRATES` aufgenommen wird, bleibt die offene, an die
    Planung weitergereichte Frage aus Issue #9s Decision-Log-Eintrag — dieses
    Issue fügt `session` dort nicht hinzu. **Nachtrag aus der zweiten
    Review-Runde unten:** `crates/session/src/bin/fs_audit_child.rs` selbst
    enthält drei `fs::write`-Aufrufe (die env-gated Leak-/Transient-Write-
    Simulation für die Detektor-Tests). Wer diese Entscheidung aufgreift,
    muss diese Datei ausnehmen oder verschieben, sonst bricht die Aufnahme
    von `session` in `GUARDED_CRATES` sofort an ihrem eigenen Test-Harness.
  - Ein Review vor dem Merge (frischer Agent, Opus), empirisch statt nur
    lesend — es reproduzierte tatsächliche Fehlschläge, nicht nur
    Vermutungen —, deckte drei echte Funde auf, alle behoben:
    - **Der Fund:** die erste Fassung behauptete, das Poll-während-der-Sitzung
      fange „die Verteidigung gegen eine Datei, die während der Sitzung
      geschrieben und vor deren Ende wieder gelöscht wird" — eine Garantie,
      die sie nicht einlöste. Mit dem damaligen Poll-Intervall (15 ms) und
      einer Sitzung, deren `TestToneSource` 48 000 Frames in 13–23 ms ohne
      reale Geräte-Wartezeit abarbeitet, blieb für eine geschriebene und
      wieder gelöschte Datei nur ein 1–2 Polls breites Fenster; eine
      injizierte Schreib-Warte-Lösch-Sequenz mit 5 ms Haltezeit entkam
      reproduzierbar 10 von 10 Läufen. Behoben: das Poll-Intervall sinkt von
      15 ms auf 2 ms (`POLL_INTERVAL`) — und **das allein** ist der Hebel.
      Eine zweite Review-Runde (ebenfalls Opus, empirisch) stellte eine
      Gegenprobe an, die die erste Fassung dieses Eintrags nicht gemacht
      hatte: 48 000 Frames (alter Wert) mit 2 ms Poll-Intervall fängt die
      Sequenz weiterhin zuverlässig, während 2 000 000 Frames mit dem alten
      15-ms-Intervall sie nahezu durchgehend verfehlt (eine dritte
      Review-Runde maß dort 1 von 10 Treffern statt 0 von 10 — die
      Richtung und die Schlussfolgerung ändert das nicht). `TOTAL_FRAMES`
      trägt zur Erkennung dieser konkreten Sequenz **nichts** bei — sie läuft
      synchron auf dem Haupt-Thread des Kindprozesses, direkt nach
      `Session::start`, entkoppelt von der Auslastung der
      Erfassungs-Threads, die `TOTAL_FRAMES` steuert. Der Wert bleibt trotzdem
      bei 2 000 000, aus einem eigenständigen, von diesem Fund unabhängigen
      Grund: er hält beide Erfassungs-Threads über einen deutlich größeren
      Teil der Kindprozess-Laufzeit tatsächlich mit echten
      `RingBuffer`-Schreibzugriffen beschäftigt, was dem **Haupttest**
      (nicht dem Transient-Write-Test) mehr Poll-Gelegenheiten während
      echter Thread-Aktivität gibt — falls ein künftiger Fehler dort
      transient in den Ring-Puffer-Pfad schriebe. Mit dem korrigierten
      2-ms-Intervall allein fing der Test dieselbe injizierte Sequenz in
      eigenen Wiederholungsläufen **10 von 10 Malen bei 5 ms Haltezeit,
      10 von 10 bei 1 ms und 15 von 15 sogar bei 0 ms** (Schreiben und
      sofortiges Löschen, ohne Wartezeit dazwischen) — ein deutlich
      stärkeres empirisches Ergebnis als erwartet, aber ausdrücklich
      **keine** mathematische Garantie: kein aus einem separaten Prozess
      pollendes Verfahren kann das ohne einen Betriebssystem-Dateisystem-Watch
      zusichern, den diese Phase bewusst nicht einführt (neue Abhängigkeit,
      zusätzliche Komplexität, unverhältnismäßig für ein gerätefreies
      Phase-1-Gate). Der wahrscheinlichste Grund, warum 2 ms fängt, was
      15 ms verfehlte: `fs::write` und `fs::remove_file` selbst brauchen
      messbare Wall-Clock-Zeit (Kernel-Overhead, auf Windows zusätzlich
      durch Virenscanner-Filtertreiber verlängerbar), in der ein enges
      Poll-Intervall die Datei noch sieht. Das ist plattform- und
      umgebungsabhängig, keine von uns erzwungene Eigenschaft — der Grund,
      warum dies eine empirische Verbesserung bleibt, keine Garantie, und
      warum die konkreten Trefferquoten oben als Beleg für **dieses**
      Setup gelten, nicht als portable Kennzahl. Der Modul-Kommentar in
      `tests/fs_audit.rs` benennt beide Punkte jetzt explizit (die Grenze
      und dass `POLL_INTERVAL`, nicht `TOTAL_FRAMES`, sie verschiebt), und
      ein neuer, über einen echten Kindprozess laufender Test
      (`catches_a_transient_write_that_outlives_the_poll_interval`, mit 50 ms
      Haltezeit — großzügiger Sicherheitsabstand für einen langsameren
      CI-Runner) belegt die Seite der Zusage, die tatsächlich eingelöst wird,
      statt der überzogenen.
    - **Der Fund:** die erste Fassung behauptete ebenfalls, die Verzeichnisse
      würden „vor dem Start" aufgenommen — tatsächlich lag die erste Aufnahme
      im ursprünglichen Code bereits **innerhalb** der Poll-Schleife, also
      nach dem Spawn. Harmlos, solange `fresh_dir()` leere Verzeichnisse
      liefert, aber eine falsche Behauptung, die dieselbe Lücke wie oben
      unbemerkt vergrößert hätte. Behoben: `run_and_watch` nimmt jetzt
      tatsächlich eine erste Aufnahme, bevor `spawn_child` überhaupt läuft.
    - **Der Fund:** der Detektor-Test
      (`detects_a_file_written_into_either_watched_directory`) bewies nur,
      dass die reine `snapshot`/Differenz-Funktion einen Fund melden kann —
      er rief nie `run_and_watch` auf. Ein Fehler, der `run_and_watch` dazu
      gebracht hätte, das falsche Verzeichnis zu beobachten oder `snapshot`
      nie aufzurufen, wäre von keinem Test in dieser Datei aufgefallen.
      Behoben durch einen neuen, echten Kindprozess-Test
      (`the_real_harness_notices_a_leak_from_a_real_child_process`):
      `fs_audit_child` hinterlässt nur unter der env-Variable
      `FS_AUDIT_CHILD_LEAK_EVIDENCE` (nie im Haupt-Audit) eine echte,
      bleibende Datei in beiden beobachteten Verzeichnissen, und der Test
      verlangt, dass `run_and_watch` genau diese über den echten Prozess
      bemerkt — dieselbe Funktion, auf die sich der Haupttest verlässt.
    - Vier kleinere Korrekturen aus derselben Runde: `CountingSource::
      degradation()` reichte zuvor nicht an die innere Quelle durch (nutzte
      den Trait-Default `None`) — heute ohne Verhaltensunterschied, weil
      `TestToneSource` selbst immer `None` liefert, aber ein unvollständiger
      Decorator, während `session`s eigener `DegradedSource`-Test-Double
      genau deshalb existiert. `fresh_dir()` nutzt jetzt `fs::create_dir`
      statt `create_dir_all`, damit eine Namenskollision (z. B. ein
      liegengebliebenes Verzeichnis eines abgebrochenen Laufs bei
      wiederverwendeter PID) laut scheitert statt ein möglicherweise nicht
      leeres Verzeichnis still weiterzuverwenden. `spawn_child` räumt
      `work_dir`/`temp_dir` jetzt auch auf seinem eigenen Panik-Pfad auf
      (ein Spawn-Fehlschlag hätte sonst beide Verzeichnisse zurückgelassen).
      Und der Haupttest bekam dieselbe „frisches Verzeichnis ist leer"-
      Sanity-Assertion, die der Detektor-Test schon hatte.
  - Eine zweite Review-Runde (wieder ein frischer Agent, Opus, wieder
    empirisch — sie stellte die Gegenprobe an, die die erste Runde nicht
    gemacht hatte) bestätigte die drei strukturellen Fixe der ersten Runde
    als echt und wirksam, fand aber, dass der Decision-Log-Eintrag selbst
    drei falsche Tatsachenbehauptungen über die eigene Korrektur enthielt —
    derselbe Fehlertyp wie Runde 1, nur auf der Dokumentationsebene statt im
    Code:
    - Die Behauptung, `TOTAL_FRAMES` trage neben `POLL_INTERVAL` zur
      Transient-Write-Erkennung bei, war falsch (siehe die korrigierte
      Fassung des Fund-Absatzes oben — jetzt mit der Gegenprobe belegt, die
      das aufdeckte).
    - Ein „200 ms Erfassungsfenster" in der Routen-Beschreibung oben widersprach
      dem tatsächlichen, in derselben Runde auf 100 ms gesenkten
      `CAPTURE_WINDOW` — korrigiert.
    - Die Behauptung, der Haupttest habe dieselbe Sanity-Assertion wie der
      Detektor-Test bekommen, war zum Zeitpunkt des Schreibens schlicht
      nicht wahr — die Assertion war nie hinzugefügt worden. Jetzt nachgezogen
      (siehe oben).
    Zusätzlich zwei echte, bis dahin unbenannte Lücken: der Modul-Kommentar
    behauptete „Machine proof … für den ganzen `session`-Orchestrator",
    obwohl der Audit ausschließlich zwei benannte Verzeichnisse beobachtet —
    ein Schreibzugriff auf einen absoluten Pfad außerhalb beider (z. B.
    `%LOCALAPPDATA%`) bliebe unsichtbar, und `session` hat mangels
    `GUARDED_CRATES`-Mitgliedschaft auch keinen Symbol-Level-Rückfallschutz
    dagegen. Der Modul-Kommentar in `tests/fs_audit.rs` benennt diese Grenze
    jetzt als eigenes „Honest limit #2" neben der Timing-Grenze. Und:
    `crates/session/src/bin/fs_audit_child.rs` enthält selbst drei
    `fs::write`-Aufrufe (die env-gated Leak- und Transient-Write-Simulation)
    — genau das Symbol, das `xtask`s Blockliste für bewachte Crates ächtet.
    Träfe die oben zurückgestellte Entscheidung, `session` in
    `xtask::audit::GUARDED_CRATES` aufzunehmen, bräche dieser Test-Kindprozess
    den Audit sofort, sofern er nicht ausdrücklich ausgenommen oder verschoben
    wird — festgehalten hier, damit die Planung diese Abhängigkeit kennt,
    statt sie erst beim Bauen zu entdecken. Schließlich, als günstige
    Härtung ohne eigenen Fund dahinter: `spawn_child` entfernt jetzt beide
    Detektor-Umgebungsvariablen explizit (`Command::env_remove`), bevor es
    `extra_env` anwendet, damit eine zufällig in der aufrufenden Shell
    gesetzte Variable nicht in den Haupt-Audit durchschlagen könnte (die
    einzige mögliche Richtung wäre ohnehin ein lauteres Scheitern, nie ein
    stilles Bestehen — trotzdem sauberer, sich nicht darauf zu verlassen).
    Ein Selbst-Fund direkt danach, von keiner der beiden Runden benannt: die
    zwei neuen Tests plus die korrigierte Dokumentation trieben
    `fs_audit.rs` auf 425 Zeilen — über die 400-Zeilen-Modulgrenze der
    Constitution, von `too_many_lines` nicht erfasst (der Lint misst nur
    Funktionslänge, nicht Moduldateigröße; dieselbe Lücke, die Issue #9
    schon einmal für `session.rs` aufgedeckt hatte). Behoben nach demselben
    Muster wie `xtask/tests/no_write_paths.rs` + `audit/mod.rs`: die
    gemeinsame Spawn-/Poll-/Snapshot-Maschinerie wandert nach
    `crates/session/tests/harness/mod.rs` (Unterverzeichnis plus `mod.rs`,
    nicht ein blankes `harness.rs` direkt unter `tests/`, aus demselben
    Grund, den Issue #8 für `audit/mod.rs` dokumentiert: sonst entdeckt
    Cargo die Datei als zweites, eigenständiges Test-Target). `fs_audit.rs`
    behält nur `mod harness;`, die vier `#[test]`-Funktionen und den
    Modul-Kommentar — danach 233 bzw. 211 Zeilen.
- 2026-07-31: Issue #14 (`cli`-Harness und Composition Root) legt die
  verbleibenden offenen Entscheidungen der Phase fest:
  - **Zweisprachigkeit:** ein `--lang <de|en>`-Flag an beiden Subcommands,
    Default `de` (passend zur deutschsprachigen Projekt-Dokumentation), statt
    beide Sprachen gleichzeitig auszugeben — der Volltext der Attestation ist
    schon lang genug, zwei Fassungen gleichzeitig hätten den Bestätigungs-Akt
    eher verschleiert als geklärt. Die Bestätigung selbst verlangt das exakte,
    sprachspezifische Wort ("ja" / "yes"), case-insensitive, getrimmt — jede
    Abweichung (leer, Whitespace, EOF, "y", "n", "yes please", das falsche
    Sprachwort) ist eine Ablehnung, nie eine Bestätigung. Der Attestation-Text
    selbst kommt unverändert aus `ATTESTATION_V1_DE`/`_EN` in `core` — die CLI
    tippt ihn nicht neu und kürzt ihn nicht.
  - **Pro-Strom-Statistik, additiv in `session`:** Issue #9 hatte Leser-Zugriff
    und Statistik bewusst zurückgestellt. Neu: `crates/session/src/stats.rs`
    mit `StreamStats` (frame_count, duration, level_dbfs, loss_count,
    gap_count, degradation) und `StatsReader`, der bei
    `StreamCapture::spawn` einen eigenen `RingBuffer`-Leser registriert —
    *vor* dem Start des Erfassungs-Threads, damit er ab dem ersten Sample
    mitliest. `Session::stream_stats(identity)` legt das offen, analog zu
    `Session::elapsed`. `frame_count` ist die Zahl der interleaved Samples
    geteilt durch die Kanalzahl (ein "Frame" = ein Sample je Kanal);
    `gap_count` fasst `discontinuity_count` und `timestamp_error_count`
    zu einer Zahl zusammen (beide bleiben intern getrennte Zähler, wie von
    Issue #5 verlangt — nur die CLI-Anzeige addiert sie). `level_dbfs` ist der
    Spitzenpegel der seit dem letzten Poll gelesenen Samples, in dBFS, mit
    einer Stille-Untergrenze von -96 dBFS (0-Amplitude hat keinen endlichen
    Dezibel-Wert); ohne neue Samples seit dem letzten Poll bleibt der Wert
    stehen, statt auf die Untergrenze zurückzufallen — sonst würde ein Meter
    zwischen zwei Polls flackern. Der Leser drainiert bei jedem Poll
    vollständig (Schleife bis `read()` 0 zurückgibt), damit er nur bei einem
    echten Stillstand des Aufrufers Verlust meldet, nie durch sein eigenes
    Poll-Intervall. `capture.rs` wuchs dadurch über die 400-Zeilen-Grenze;
    der Testblock wanderte nach `capture/tests.rs`, dem in dieser Spec
    etablierten Muster folgend.
  - **Zwei Subcommands, kein Bypass:** `list-sources [--lang]` und
    `capture --pid <PID> [--lang]`. Kein `--yes`/`--force`-Schalter — jeder
    Weg, das Consent-Gate zu umgehen, wäre ein Bruch des Kernversprechens.
    `capture` löst die `--pid` gegen eine **frische**
    `SourceFactory::list_subjects()`-Abfrage auf und zeigt Prozessname und
    Wurzel-PID **vor** der Attestation. Die spec-verbindliche zweite
    Identitätsprüfung (Name **und** Startzeit) läuft weiterhin dort, wo sie
    seit Issue #12 bereits sitzt — innerhalb von `SourceFactory::open_remote`
    (`audio-win`s `verify_subject_identity`) — und wird von `cli` nur durch
    den normalen Aufruf des Traits erreicht, nicht dupliziert.
  - **Das Consent-Gate ist testbar, weil es eine reine Funktion ist:**
    `capture_command::resolve_and_start` nimmt `SourceFactory`, `BufRead` und
    `Write` generisch entgegen und liefert `Ok(None)` — nie eine `Session` —
    sobald die Bestätigung ausbleibt. Mit `TestToneSources` als Fabrik belegen
    Unit-Tests beide Richtungen: Ablehnung konstruiert nachweislich keine
    `Session`, Bestätigung schon. Bei Ablehnung druckt die CLI eine Meldung,
    die "Gegenseite: 0 Frames, Ich (Mikrofon): 0 Frames" wörtlich nennt —
    ohne dass je eine `Session` existiert hätte, die das hätte melden können.
  - **Stop per Enter, nicht per Ctrl+C:** die laufende Erfassung endet, sobald
    der Nutzer eine Zeile auf stdin bestätigt (ein eigener Thread blockiert
    auf `read_line`). Ctrl+C wurde bewusst **nicht** abgefangen: ein
    Signal-Handler bräuchte entweder eine neue, in dieser Phase nicht
    vorgesehene Abhängigkeit (z. B. `ctrlc`) oder `unsafe`-FFI — beides
    außerhalb des Scopes dieses Issues. Bekannte Lücke, hier festgehalten
    statt stillschweigend offen gelassen: ein Kill per Ctrl+C überspringt
    `Session::drop`s explizites Nullen; das Betriebssystem gibt den Speicher
    beim Prozessende ohnehin frei, der explizite Zeroize-Schritt davor entfällt
    aber. Für die Planung als möglicher Folge-Punkt vorgemerkt, nicht als
    Issue angelegt.
  - **Kein Schreibpfad:** `cli` bietet keine `--output`-Datei und keine
    Log-Datei an; jede Ausgabe geht nach stdout/stderr. Smoke-Test am
    2026-07-31 gegen die echte Windows-Maschine: `list-sources` fand ohne
    laufenden Ton-Erzeuger keine Quelle, und mit einer im Hintergrund
    abgespielten `.wav`-Datei genau eine — `PID … WindowsTerminal.exe`,
    aufgelöst über die bereits bekannte, nicht neu zu verhandelnde
    Shell-Wurzel-Ausnahme (siehe die Liste der bereits gerouteten Punkte).
