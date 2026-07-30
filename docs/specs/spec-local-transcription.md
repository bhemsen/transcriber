# Spec: Phase 2 — Lokale Transkription

> Created: 2026-07-30

Diese Spec liefert die lokale Spracherkennung: whisper.cpp über `whisper-rs` hinter
einem `SpeechToText`-Trait, eine GPU-/CPU-Ladder, ein Fenster-Scheduling, das
Live-Rohtext höchstens fünf Sekunden hinter dem Gesprochenen ausgibt, und eine
WER-Messung gegen je ein definiertes deutsches und englisches Referenzsample. Dazu
das Minimum an Modellbereitstellung, ohne das nichts davon läuft.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] Aus dem `Remote`-Strom entsteht während der Sitzung fortlaufend Text; der
      Live-Rohtext liegt **≤ 5 s** hinter dem Gesprochenen (gemessen, nicht geschätzt).
- [ ] Auf einer GPU mit 8 GB VRAM läuft die Erkennung schneller als Echtzeit; ohne
      nutzbare GPU greift die CPU-Ladder und die Sitzung bleibt benutzbar.
- [ ] Die WER liegt auf dem definierten **englischen** Referenzsample **≤ 15 %**.
- [ ] Die WER auf dem definierten **deutschen** Referenzsample ist gemessen und
      dokumentiert; ob sie das 15-%-Ziel trägt, entscheidet die Sample-Wahl unten.
- [ ] Das Modell wird nicht aus dem Repository und nicht aus dem Installer geladen,
      sondern über eine versionierte Allowlist mit SHA-256-Prüfung in ein
      Cache-Verzeichnis.
- [ ] Jedes Transkript trägt seine Herkunft: Modell, Quantisierung, Backend und
      Sprache — die Grundlage dafür, dass der Protokollkopf in Phase 4 die
      Textqualität belegen kann.
- [ ] `asr` hat weiterhin keinen Schreib- und keinen Netzpfad; der Audit-Test bewacht
      es ab dieser Phase mit.
- [ ] `cargo xtask verify` bleibt grün, lokal und in CI — mit gemessener und in
      `docs/workflow.md` korrigierter Laufzeit, weil hier erstmals eine native
      Bibliothek aus den Quellen gebaut wird.

## Scope

### In scope

- Crate `asr`: `SpeechToText`-Trait, `whisper-rs`-Implementierung, Fenster-Scheduling
  auf dem Live-Strom, GPU-/CPU-Ladder, Sprachwahl.
- Crate `provisioning` (neu, minimal): versionierte Modell-Allowlist,
  SHA-256-Prüfung, Cache-Verzeichnis. Der **einzige** Netzzugriff im Workspace.
- Crate `core`: `TranscriptSegment { stream, t0, t1, text, state }` und
  `TranscriptProvenance` (Modell, Quantisierung, Backend, Sprache).
- Crate `session`: `asr` als Leser am Ringpuffer-Cursor aus Phase 1 anhängen und
  Transkript-Ereignisse über den bestehenden Event-Bus ausgeben.
- Crate `cli`: Live-Textansicht und die Ausgabe der Herkunftsangaben.
- `xtask`: eine **opt-in** WER-Messung (`cargo xtask wer`) und die Erweiterung des
  Quell- und Manifest-Audits auf `asr`.
- `docs/workflow.md`: die Verify-Laufzeit neu messen und die dort hinterlegte
  Erwartung korrigieren — die Datei fordert das ausdrücklich für nach Phase 2.

### Out of scope

- VAD, Segmentierung, Embeddings, Sprecherzuordnung — Phase 3. Das
  Fenster-Scheduling dieser Phase ist **zeitgesteuert**, nicht VAD-gesteuert; die
  beiden Konsumenten kennen einander laut `docs/architecture.md` nicht.
- Protokoll-Ausgabe und der Protokollkopf — Phase 4. Diese Phase erzeugt die
  Herkunftsangaben, schreibt sie aber nicht.
- Erststart-Onboarding und Installer-Integration der Modellbereitstellung — Phase 6.
  Hier entsteht nur der Mechanismus (Allowlist, Prüfsumme, Cache), nicht die
  Benutzerführung.
- Transkription des `Local`-Stroms als eigene Qualitätsfrage. Der Mikrofonstrom läuft
  durch dieselbe Pipeline; ein eigenes Referenzsample dafür gibt es nicht.
- Übersetzung, Zeichensetzungs-Nachbearbeitung, LLM-Nachkorrektur — `docs/vision.md`,
  Out und Non-goals.
- Weitere STT-Engines. Der Trait existiert, damit Parakeet später ohne Änderung an
  der Pipeline dazukommen kann (`docs/prior-art.md`); implementiert wird er nicht.

## Constraints

- `#![forbid(unsafe_code)]` in `asr` und `provisioning`. `whisper-rs` kapselt das FFI;
  unsere Crates rufen nur safe Funktionen.
- `asr` hat keinen Schreibpfad und keinen HTTP-Client. Das **Lesen** von
  Modelldateien über Pfade ist erlaubt (`docs/constitution.md`, Architecture
  principles). Nur `provisioning` greift aufs Netz.
- Abhängigkeitsrichtung: `asr` → `core` + `audio`; `provisioning` → `core`. `asr`
  kennt `provisioning` **nicht** — es bekommt einen fertigen Modellpfad übergeben,
  sonst hätte das Crate ohne Schreib-/Netzpfad eine Kante dorthin.
- Kein Netzzugriff außer dem Download aus der versionierten Allowlist mit
  SHA-256-Prüfung. Jede weitere URL im Code ist ein Merge-Blocker.
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul; kein `unwrap()`/
  `expect()` außer in Tests und `main`.
- **Lizenz-Gate:** `whisper-rs` und `whisper-rs-sys` stehen unter **Unlicense**, das
  in `deny.toml` **nicht** auf der Allowlist steht — der Merge blockiert, bis der
  Eintrag existiert. Das vendorte `whisper.cpp` und `ggml` sind MIT und damit gedeckt.
- **Build-Voraussetzungen ab hier scharf:** `whisper-rs-sys` baut `whisper.cpp` aus
  den Quellen, braucht also CMake und die Visual-Studio-C++-Build-Tools. Die Spec von
  Phase 1 hat diese Anforderung ausdrücklich auf „ab Phase 2" eingegrenzt; hier
  greift sie.
- Die GitHub-`windows-latest`-Runner haben **kein** Audiogerät und **keine** nutzbare
  GPU. CI prüft den CPU-Pfad; die GPU-Ladder wird lokal am QA-Gate geprüft.

## Prior art

- [Local speech-to-text engine (Phase 2)](../prior-art.md#local-speech-to-text-engine-phase-2)
  — begründet `large-v3-turbo` als Standard, kleinere Modelle als CPU-Ladder, und den
  Trait, hinter dem Parakeet später Platz hat. Ebenfalls das AVOID, Gewichte in den
  Installer zu legen.
- [Produktgestalt und Desktop-Stack (Phase 5)](../prior-art.md#produktgestalt-und-desktop-stack-phase-5)
  — die GPU-Backend-Matrix als **Form** unserer Ladder. Der Punkt, an dem wir davon
  abweichen, steht unten als offene Entscheidung: ein Installer trägt keine drei
  GPU-Backends.
- [Auslieferung und Modellbereitstellung (Phase 6)](../prior-art.md#auslieferung-und-modellbereitstellung-phase-6)
  — Modelle beim ersten Start laden statt paketieren; trägt direkt das
  Time-to-first-protocol-Kriterium und damit die Quantisierungs-Wahl unten.

## Human prerequisites

- [ ] CMake und die Visual-Studio-C++-Build-Tools lokal installiert. Ohne sie
      scheitert ab dieser Phase schon `cargo build`, nicht erst ein Test.
- [ ] Eine GPU mit ≥ 8 GB VRAM auf der Entwicklungsmaschine für den GPU-Zweig des
      QA-Gates — oder die ausdrückliche Feststellung, dass nur der CPU-Pfad geprüft
      werden kann.
- [ ] Bestätigung, dass das Herunterladen der öffentlichen Referenzsamples in ein
      Cache-Verzeichnis außerhalb des Repositories in Ordnung ist (siehe die
      Entscheidung „Referenzsamples" unten — sie berührt die Null-Audio-Zusage und
      wird deshalb nicht stillschweigend getroffen).
- [ ] Falls die Wahl auf ein selbst aufgenommenes deutsches Meeting-Sample fällt:
      das Sample samt Referenztranskript und die Einverständnisse der Sprechenden.
      Nur dann; siehe offene Entscheidung.
- [ ] Keine Secrets, keine Accounts, keine API-Keys. Die Modell- und Sample-Quellen
      sind öffentlich und unauthentifiziert.

## Prior decisions

### Engine, Modell und die Ladder

| Decision | Rationale | Date |
|---|---|---|
| `whisper-rs` 0.14.x als Bindung, `whisper.cpp` gevendort darin | `docs/constitution.md` legt die Engine fest. Version am 2026-07-30 im Quelltext geprüft: 0.14.3, Bindung unter Unlicense, `whisper.cpp` und `ggml` MIT | 2026-07-30 |
| Standardmodell `large-v3-turbo` in **q5_0**-Quantisierung, nicht f16 | `docs/prior-art.md` legt `large-v3-turbo` als Qualitäts-/Geschwindigkeits-Standard fest. Die Quantisierung entscheidet vor allem die **Downloadgröße** (~570 MB gegen ~1,6 GB), und das Kriterium „Time-to-first-protocol ≤ 15 Minuten" ist download-dominiert, nicht rechen-dominiert | 2026-07-30 |
| Die Ladder wechselt **Backend und Modell zusammen**, nicht nur das Backend: (1) GPU + `large-v3-turbo`, (2) CPU + `small`, (3) CPU + `base` | `large-v3-turbo` auf CPU ist für Echtzeit zu langsam — eine reine Backend-Ladder würde das Latenzkriterium auf der CPU zwangsläufig verfehlen. `docs/prior-art.md` sagt genau das: kleinere Modelle **sind** die CPU-Ladder | 2026-07-30 |
| Der Laufzeit-Fallback nutzt `WhisperContextParameters::use_gpu(false)`, nachdem der GPU-Versuch fehlgeschlagen ist | Am Quelltext geprüft: `use_gpu` ist per Default `cfg!(feature = "_gpu")` und zur Laufzeit umschaltbar. Ein **einziges** GPU-fähiges Binary kann damit echt auf CPU zurückfallen — die Ladder braucht keine zweite Auslieferung | 2026-07-30 |
| Welche Stufe gegriffen hat, wird als `TranscriptProvenance` geführt und von der CLI ausgegeben | Ohne diese Angabe ist eine WER-Zahl nicht interpretierbar, und der Protokollkopf in Phase 4 könnte die Textqualität nicht belegen | 2026-07-30 |
| Das ggml-Logging wird über das Feature `log_backend` in `log` geleitet, nicht auf stderr gelassen | whisper.cpp schreibt sonst ungefiltert auf stderr. Kein Dateipfad, keine Verletzung — aber eine Ausgabe, die wir sonst nicht kontrollieren | 2026-07-30 |
| Sprachwahl je Sitzung: `de`, `en` oder `auto`. `auto` erkennt **einmal** auf den ersten Sekunden und wird dann festgeschrieben | Whisper pro Fenster neu erkennen zu lassen führt zu Sprachwechseln mitten im Gespräch, die den Text schlechter machen als eine falsche, aber stabile Wahl. Die Sitzung ist die richtige Einheit | 2026-07-30 |

### Fenster-Scheduling

| Decision | Rationale | Date |
|---|---|---|
| Zeitgesteuertes Schiebefenster: **15 s Kontext, 3 s Hop**, beide konfigurierbar | Die Hop-Länge ist die Untergrenze der Latenz; bei 5 s Hop wäre das 5-s-Kriterium schon ohne Rechenzeit ausgeschöpft. 3 s lässt rund 2 s für die Inferenz. 15 s Kontext ist der Kompromiss zwischen Whispers Vorliebe für lange Fenster und der Rechenzeit je Hop. Konfigurierbar, weil die WER-Messung genau hier vergleichen muss | 2026-07-30 |
| Text aus dem überlappenden Bereich wird **dedupliziert**, und Segmente tragen einen Zustand: `Provisional` für den jüngsten Hop, `Final` sobald sie aus dem Kontextfenster gelaufen sind | Ohne Zustand müsste die Oberfläche entweder flackernden Text zeigen oder auf `Final` warten und das Latenzkriterium verfehlen. Mit Zustand kann sie provisorischen Text zeigen und in Ruhe ersetzen | 2026-07-30 |
| Das Scheduling liest über einen **eigenen Ringpuffer-Cursor** aus Phase 1 und meldet Verlust, statt zu blockieren | Genau der Zweck der Cursor-API aus Phase 1. Bleibt die Inferenz hinter dem Strom zurück, ist der gemeldete Verlust das sichtbare Symptom — kein stilles Auslassen | 2026-07-30 |
| Fällt die Inferenz dauerhaft hinter den Hop zurück, wird der Hop **nicht** stillschweigend verlängert: die Sitzung meldet den Rückstand und die Ladder stuft ab | Ein still wachsender Hop würde das Latenzkriterium unbemerkt verletzen. Abstufen ist die ehrliche Reaktion: schlechterer Text, gehaltene Latenz | 2026-07-30 |

```
Zeit ────────────────────────────────────────────────────────────────▶

Ringpuffer   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  30 s, fest

Fenster n        ├────────── 15 s Kontext ──────────┤
Fenster n+1            ├────────── 15 s Kontext ──────────┤
Fenster n+2                  ├────────── 15 s Kontext ──────────┤
                       ├ 3 s ┤ Hop

Segmente     ├─── Final ───┤├─── Final ───┤├── Provisional ──┤
                                            (wird beim nächsten
                                             Hop ersetzt)
```

### Referenzsamples und WER-Messung

| Decision | Rationale | Date |
|---|---|---|
| Englisches Referenzsample: ein fest benannter Auszug aus dem **AMI Meeting Corpus** (CC-BY-4.0) | Am 2026-07-30 recherchiert: 100 h echte Mehrsprecher-Besprechungen, permissiv lizenziert, in der ASR-Forschung der kanonische Meeting-Benchmark. Das ist ein echtes Meeting-Sample, kein Ersatz | 2026-07-30 |
| Die WER-Messung ist **opt-in** (`cargo xtask wer`) und **nicht** Teil von Verify | Verify ist das Gate je Iteration und muss schnell bleiben. Modell laden, Sample holen und Inferenz laufen lassen kostet Minuten und braucht Netz — in jeder Iteration wäre das eine Bremse ohne Gegenwert. Gelaufen wird sie am Milestone-QA-Gate und bei jeder Änderung am ASR-Pfad | 2026-07-30 |
| Die Referenzsamples liegen **nicht** im Repository, sondern werden über `provisioning` mit SHA-256-Prüfung ins Cache-Verzeichnis geladen — **bewusst offengelegte Abweichung** von `docs/constitution.md`, Don'ts („Kein Speichern von Roh-Audio, auch nicht temporär") | Der Zweck jener Regel ist der Erfassungspfad: **Audio aus der Sitzung des Nutzers** darf nirgends landen. Ein öffentliches, lizenziertes Benchmark-Sample in einem Entwickler-Cache entsteht nicht aus einer Sitzung, wird von der Anwendung zur Laufzeit nie geschrieben und ist Voraussetzung dafür, das WER-Kriterium überhaupt zu prüfen. Die Zusage der Anwendung bleibt unberührt — der Audit-Test bewacht `asr` weiter. Offengelegt statt vorausgesetzt, weil es die Kernzusage berührt | 2026-07-30 |
| WER wird nach der üblichen Normalisierung gemessen (Kleinschreibung, Satzzeichen entfernt, Zahlwörter vereinheitlicht) und die Normalisierung mit dem Ergebnis dokumentiert | Ohne festgeschriebene Normalisierung ist eine WER-Zahl nicht vergleichbar und das 15-%-Kriterium nicht überprüfbar — man kann sie durch die Wahl der Regeln um mehrere Punkte verschieben | 2026-07-30 |

### Bereitstellung und Gates

| Decision | Rationale | Date |
|---|---|---|
| `provisioning` entsteht **in dieser Phase**, minimal: Allowlist mit Version, URL und SHA-256, Prüfung, Cache-Verzeichnis im Nutzerprofil | `asr` ist ohne Modell nicht lauffähig, also braucht diese Phase den Download. Die Constitution erlaubt Netzzugriff nur „aus einer versionierten Allowlist mit SHA-256-Prüfung" — dieser Mechanismus muss beim **ersten** Download existieren, nicht erst in Phase 6 | 2026-07-30 |
| `asr` bekommt einen fertigen Modellpfad übergeben und kennt `provisioning` nicht | Sonst hätte das Crate ohne Netzpfad eine Kante zum einzigen Crate mit Netzpfad — die Trennung wäre nur noch eine Absichtserklärung statt eine Eigenschaft des Abhängigkeitsgraphen | 2026-07-30 |
| Der Quell- und Manifest-Audit aus Phase 1 wird auf `asr` erweitert; `provisioning` wird **nicht** bewacht, ist aber das einzige Crate, in dem ein HTTP-Client erlaubt ist — der Audit prüft zusätzlich, dass kein **anderes** Crate einen mitbringt | Die Blockliste der Constitution nennt `reqwest` und `ureq`. Ohne die Gegenprobe „nur in `provisioning`" wäre die Regel an der falschen Stelle durchsetzbar | 2026-07-30 |
| Die Verify-Laufzeit wird nach dem ersten nativen Build gemessen und in `docs/workflow.md` eingetragen; CI cacht das Build-Verzeichnis, damit die Ladder nicht bei jedem Lauf neu kompiliert | `docs/workflow.md` fordert die Neumessung nach Phase 2 ausdrücklich. Ohne Cache wird jeder CI-Lauf um den whisper.cpp-Build teurer, und ein Gate, das zu lange dauert, wird umgangen | 2026-07-30 |
| Kein `/loopkit:design`-Zyklus. Das einzige Konzept, das ein Bild braucht — das Fenster-Scheduling — steht als Diagramm oben **in dieser Spec** | Phase 2 hat keine UI-Fläche; die Live-Textansicht der CLI ist Textausgabe. Das Diagramm ist eine im Repo committete Datei und erfüllt damit die Durable-form-Regel aus `docs/design.md`, ohne einen Artifact-Zyklus für eine Zeitachse zu starten | 2026-07-30 |
| OPEN — GPU-Ladder: die Backends von `whisper-rs` sind **Compile-Time-Features**, also trägt ein Binary genau eines. `docs/constitution.md` nennt „CUDA-/Vulkan-/CPU-Ladder", was so nicht in einen Installer passt | resolved at the spec-acceptance gate | — |
| OPEN — deutsches Referenzsample: es existiert kein permissiv lizenziertes deutsches **Meeting**-Korpus. Damit ist das Vision-Kriterium „je einem definierten deutschen und englischen Meeting-Referenzsample" auf der deutschen Seite nur über einen Ersatz oder eine eigene Aufnahme erreichbar | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 2 — Lokale Transkription (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist — eines je
  implementierbarem Schritt
- Diese Phase baut auf Phase 1 auf; der Milestone trägt
  `Depends on milestone: #1`, und die Issues, die den Ringpuffer und die Sitzung
  berühren, tragen die entsprechenden Cross-Milestone-Kanten.

Jedes Issue verweist im Body auf diesen Spec-Pfad.

## Verification

Maschinell, in Verify und in CI auf `windows-latest`:

- [ ] `cargo xtask verify` grün; `cargo xtask build` grün.
- [ ] `cargo deny check` grün **mit** dem Unlicense-Eintrag für `whisper-rs` und
      `whisper-rs-sys` in der Allowlist.
- [ ] Der Quell- und Manifest-Audit bewacht `asr` und findet dort keinen Schreib-,
      Netz- oder `serde`-Pfad; er belegt zusätzlich, dass ein HTTP-Client nur in
      `provisioning` vorkommt.
- [ ] `provisioning`-Test: ein Download mit falscher SHA-256 wird abgewiesen und
      nichts landet im Cache; eine URL außerhalb der Allowlist wird abgewiesen.
- [ ] Scheduler-Test ohne Modell und ohne Gerät (Testton-Quelle aus Phase 1): bei
      15 s Kontext und 3 s Hop entstehen die erwarteten Fenstergrenzen, der
      Überlappungsbereich wird dedupliziert, und Segmente wechseln von `Provisional`
      zu `Final`.
- [ ] Rückstands-Test: eine künstlich verlangsamte `SpeechToText`-Attrappe führt zu
      einem gemeldeten Rückstand und einer Abstufung — nicht zu einem still
      verlängerten Hop.
- [ ] CPU-Pfad-Test in CI: die Ladder findet keine GPU, wählt Stufe 2 und
      transkribiert das synthetische Signal ohne Absturz.

Manuell am Milestone-QA-Gate (Smoke-Test nach `docs/workflow.md`):

- [ ] **Latenz:** in einem echten Call gesprochener Satz erscheint ≤ 5 s später als
      Rohtext. Gemessen, mit dem gewählten Hop und der gemessenen Inferenzzeit
      protokolliert.
- [ ] **GPU-Pfad:** auf der 8-GB-GPU greift Stufe 1, die Inferenz je Fenster ist
      kürzer als der Hop, und der Call läuft ohne Ton-Ausfall weiter.
- [ ] **CPU-Fallback:** GPU deaktiviert → die Sitzung läuft auf Stufe 2 weiter, die
      Herkunftsangabe zeigt den Wechsel.
- [ ] **WER Englisch:** `cargo xtask wer` auf dem AMI-Auszug ergibt ≤ 15 %, mit
      dokumentierter Normalisierung.
- [ ] **WER Deutsch:** gemessen und dokumentiert auf dem am Gate gewählten Sample.
- [ ] **Modellbereitstellung:** auf einer Maschine ohne Cache wird das Modell geladen,
      die Prüfsumme verifiziert, und die Zeit bis zum ersten Text notiert — der
      Eingangswert für das 15-Minuten-Kriterium in Phase 6.
- [ ] **Null Bytes bleibt gültig:** der Sitzungs-FS-Audit aus Phase 1 läuft weiter
      grün, jetzt mit angehängtem ASR-Konsumenten.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Die WER auf dem deutschen Sample verfehlt 15 %, weil das Sample kein Meeting ist (Parlamentsrede statt Gespräch) | Die Sample-Wahl ist eine offene Entscheidung am Gate, nicht eine Annahme dieser Spec. Das Ergebnis wird mit der Sample-Herkunft dokumentiert; verfehlt es das Ziel, ist die Frage „falsches Sample oder falsches Modell", nicht „Kriterium gerissen" |
| 3 s Hop plus Inferenz reißt das 5-s-Kriterium auf schwächerer Hardware | Hop und Kontext sind konfigurierbar, der Rückstand wird gemeldet, und die Ladder stuft ab statt still zu verzögern. Die tatsächlichen Zahlen entstehen am QA-Gate und korrigieren die Standardwerte |
| Der whisper.cpp-Build macht Verify und CI deutlich langsamer | Build-Cache in CI, Neumessung der Verify-Dauer und Korrektur in `docs/workflow.md` sind eigene Akzeptanzpunkte. Wird Verify unzumutbar, ist das ein Befund dieser Phase, nicht ein Ärgernis der nächsten |
| Der Vulkan-Pfad funktioniert auf einer Zielmaschine nicht, und der CPU-Fallback ist für Echtzeit zu langsam | Die Ladder wechselt Modell **und** Backend, damit die schwächste Stufe realistisch bleibt. Reicht auch `base` nicht, ist das eine belegte Aussage über die Mindest-Hardware, die in die README von Phase 6 gehört |
| Das Herunterladen der Referenzsamples wird als Bruch der Null-Audio-Zusage gelesen | Ausdrücklich als Abweichung offengelegt, mit Begründung und Grenze: die **Anwendung** schreibt weiter kein Audio, nur die Entwickler-Messung holt ein öffentliches Sample. Am Gate vorgelegt, nicht vorausgesetzt |
| `whisper-rs` steht unter Unlicense, das manche Organisationen als Lizenzrisiko führen | Der Eintrag in der Allowlist ist ein bewusster, dokumentierter Schritt und kein Versehen. Das gevendorte `whisper.cpp` selbst ist MIT; die Unlicense betrifft nur die Bindungsschicht |

## Decision log

- 2026-07-30: `whisper-rs` v0.14.3 in einem lesenden, wegwerfbaren Klon außerhalb des
  Repos geprüft. Belegt: die GPU-Backends sind **Compile-Time-Features**
  (`cuda`, `vulkan`, `metal`, `hipblas`, `intel-sycl`, `coreml`), es gibt keinen
  Streaming-Beispielpfad in der Bindung, und `use_gpu` ist zur Laufzeit umschaltbar
  mit `cfg!(feature = "_gpu")` als Default. Daraus folgen zwei Dinge: die Ladder kann
  aus **einem** Binary echt auf CPU zurückfallen, aber sie kann nicht drei
  GPU-Backends tragen — deshalb die offene Entscheidung. Kein externer Code ins Repo
  übernommen.
- 2026-07-30: Lizenz von `whisper-rs`/`whisper-rs-sys` ist **Unlicense** und fehlt in
  der `deny.toml`-Allowlist. Ohne Eintrag blockiert `cargo deny check` den Merge —
  eine echte, vorab gefundene Blockade statt einer Überraschung im ersten
  Implementierungs-PR.
- 2026-07-30: Recherche zu den Referenzsamples. Englisch ist gelöst (AMI Meeting
  Corpus, CC-BY-4.0, echte Besprechungen). Deutsch nicht: ein permissiv lizenziertes
  deutsches Meeting-Korpus existiert nicht. VoxPopuli DE (CC0, 282 h) ist
  spontane Mehrsprecher-Sprache, aber parlamentarisch, also ein Ersatz und kein
  Treffer; die vorhandenen deutschen Gesprächskorpora sind rar oder nicht permissiv.
  Das Vision-Kriterium ist auf der deutschen Seite damit nur über einen offengelegten
  Ersatz oder eine eigene Aufnahme erreichbar — vorgelegt als offene Entscheidung
  statt stillschweigend auf den Ersatz gesetzt.
