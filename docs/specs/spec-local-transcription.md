# Spec: Phase 2 — Lokale Transkription

> Created: 2026-07-30

Diese Spec liefert die lokale Spracherkennung: whisper.cpp über `whisper-rs` hinter
einem `SpeechToText`-Trait, eine Ladder, die Backend **und** Fensterparameter
gemeinsam abstuft, ein zeitgesteuertes Fenster-Scheduling für den Live-Rohtext, und
eine WER-Messung gegen je ein definiertes deutsches und englisches Referenzsample.
Dazu das Minimum an Modellbereitstellung, ohne das nichts davon läuft.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] Aus **beiden** Strömen entsteht während der Sitzung fortlaufend Text, je mit
      absoluten Zeitstempeln auf der Sitzungs-Zeitachse aus Phase 1.
- [ ] Auf dem GPU-Pfad liegt der Live-Rohtext des `Remote`-Stroms **≤ 5 s** hinter
      dem Gesprochenen — gemessen als `Hop + Inferenzzeit + Pipeline-Overhead`, mit
      einer harten Schranke von **≤ 2,0 s Inferenz je Fenster**.
- [ ] Auf einer GPU mit 8 GB VRAM greift Stufe 1 der Ladder; ohne nutzbare GPU greift
      eine CPU-Stufe und erzeugt korrekten Text (die Latenz dort: siehe die offene
      Entscheidung unten).
- [ ] Die WER liegt auf dem definierten **englischen** Referenzsample **≤ 15 %**.
- [ ] Die WER liegt auf dem definierten **deutschen** Referenzsample **≤ 15 %**.
- [ ] Das Modell wird nicht aus dem Repository und nicht aus dem Installer geladen,
      sondern über eine versionierte Allowlist mit SHA-256-Prüfung in ein
      Cache-Verzeichnis.
- [ ] `session` hält das Transkript autoritativ im Arbeitsspeicher; ein
      zurückgefallener Event-Abonnent verliert **keinen** Protokolltext.
- [ ] Jedes Segment trägt seine Herkunft: Modell, Quantisierung, Backend, Sprache und
      die Ladder-Stufe.
- [ ] `asr` hat weiterhin keinen Schreib- und keinen Netzpfad; der Audit-Test bewacht
      es ab dieser Phase mit und belegt, dass ein HTTP-Client nur in `provisioning`
      vorkommt.
- [ ] `cargo xtask verify` bleibt grün, lokal und in CI, **ohne** ein Modell zu laden
      — mit gemessener und in `docs/workflow.md` korrigierter Laufzeit.
- [ ] Kein Foundation-Dokument widerspricht mehr dem Code (Liste in In scope).

## Scope

### In scope

- Crate `asr`: `SpeechToText`-Trait, `whisper-rs`-Implementierung, Fenster-Scheduling,
  Ladder, Sprachwahl, Deduplizierung des Überlappungsbereichs.
- Crate `provisioning` (neu, minimal): versionierte **Modell**-Allowlist,
  SHA-256-Prüfung, Cache-Verzeichnis. Der einzige Netzzugriff im ausgelieferten Stand.
- Crate `core`: `TranscriptSegment { id, stream, t0, t1, text, state }`,
  `TranscriptProvenance`, `Language`.
- Crate `session`: `asr` als Leser an je einem Ringpuffer-Cursor aus Phase 1;
  **autoritativer** Transkript-Speicher im RAM; Transkript-Ereignisse als
  Benachrichtigung über den bestehenden Event-Bus.
- Crate `cli`: Composition Root für die Modellbereitstellung, Live-Textansicht,
  Ausgabe der Herkunftsangaben.
- `xtask`: die **opt-in** WER-Messung (`cargo xtask wer`) **einschließlich des
  Referenzsample-Bezugs** — bewusst hier und nicht in `provisioning`, siehe unten.
  Dazu die Erweiterung des Quell- und Manifest-Audits auf `asr`.
- **Foundation-Doc-Korrekturen** — die verbindliche Aufzählung, als **ein** Schritt:
  1. `docs/architecture.md`, Boundaries: Phase 1 hat auf „`session` → `core` +
     `audio`" verengt. Diese Phase hängt `asr` an die Sitzung, also lautet die Kante
     `session` → `core` + `audio` + `asr`.
  2. `docs/architecture.md`, Flow 2: `TranscriptSegment{stream, t0, t1, text}` bekommt
     `id` und `state` — ohne Identität ist „provisorisch durch final ersetzen" nicht
     ausdrückbar.
  3. `docs/workflow.md`, Environment prerequisites: „ohne diese **vier** scheitert
     Bootstrap" wird durch diese Phase unwahr — LLVM/libclang (für `bindgen`) und das
     SDK des gewählten GPU-Backends kommen hinzu.
  4. `docs/prior-art.md`, Auslieferung und Modellbereitstellung: das ADOPT „ein
     Installer, zur Laufzeit gewähltes Backend" ist über `whisper-rs-sys` **nicht**
     erreichbar (die Backends sind Compile-Time-Features, und ggmls dynamische
     Backend-Registry ist durch den statischen CMake-Build nicht zugänglich).
     Selbst whisper.cpp liefert für Windows getrennte Backend-Artefakte. Die Zeile
     wird auf das korrigiert, was gilt.
  5. `docs/workflow.md`: die gemessene Verify-Dauer, die die Datei nach Phase 2
     ausdrücklich neu gemessen haben will.

### Out of scope

- VAD, Segmentierung, Embeddings, Sprecherzuordnung — Phase 3. Das Scheduling hier
  ist **zeitgesteuert**, nicht VAD-gesteuert; die beiden Konsumenten kennen einander
  laut `docs/architecture.md` nicht.
- Protokoll-Ausgabe und Protokollkopf — Phase 4. Diese Phase erzeugt die
  Herkunftsangaben und hält das Transkript, schreibt aber nichts.
- Erststart-Onboarding und Installer-Integration — Phase 6. Hier entsteht nur der
  Mechanismus.
- Übersetzung, Zeichensetzungs-Nachbearbeitung, LLM-Nachkorrektur —
  `docs/vision.md`, Out und Non-goals.
- Weitere STT-Engines. Der Trait existiert, damit Parakeet später ohne Änderung an
  der Pipeline dazukommen kann; implementiert wird er nicht.

## Constraints

- `#![forbid(unsafe_code)]` in `asr` und `provisioning`. `whisper-rs` kapselt das FFI.
- `asr` hat keinen Schreibpfad und keinen HTTP-Client; das **Lesen** von
  Modelldateien über Pfade ist erlaubt (`docs/constitution.md`, Architecture
  principles). Nur `provisioning` greift aufs Netz.
- Abhängigkeitsrichtung: `asr` → `core` + `audio`; `provisioning` → `core`;
  `session` → `core` + `audio` + `asr` (Korrektur 1); `cli` → alles davon. `asr`
  kennt `provisioning` **nicht** — es bekommt einen fertigen Modellpfad, sonst hätte
  das netzfreie Crate eine Kante zum einzigen Netz-Crate.
- Kein Netzzugriff außer dem **Modell**-Download aus der versionierten Allowlist mit
  SHA-256-Prüfung. Der Bezug der Referenzsamples ist deshalb **kein** Teil von
  `provisioning` (siehe die Entscheidung unten).
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul; kein `unwrap()`/
  `expect()` außer in Tests und `main`.
- **Vollständiges Abhängigkeits-Inventar** dieser Phase, alle mit
  `cargo-deny`-Konsequenz:

  | Crate | Zweck | Lizenz | Allowlist |
  | --- | --- | --- | --- |
  | `whisper-rs`, `whisper-rs-sys` | Engine-Bindung | **Unlicense** | **fehlt** — als `[licenses] exceptions` auf genau diese zwei begrenzen, nicht global erlauben |
  | `ureq` | HTTP für den Modell-Download | MIT/Apache-2.0 | gedeckt |
  | `sha2` | Prüfsumme | MIT/Apache-2.0 | gedeckt |
  | `directories` | Cache-Verzeichnis im Nutzerprofil | MIT/Apache-2.0 | gedeckt |
  | Edit-Distanz (WER), nur `dev`/`xtask` | WER-Rechnung | zu prüfen | vor Nutzung prüfen |
  | `bindgen`, `cmake`, `clang-sys` (Build-Deps von `whisper-rs-sys`) | Build | zu prüfen | `cargo deny` prüft sie mit |

  `ureq` statt `reqwest`: `reqwest` mit rustls zieht `ring` nach, den klassischen
  cargo-deny-Ausnahmefall, und eine async-Laufzeit, die der Download nicht braucht.
- **Build-Voraussetzungen ab hier scharf:** `whisper-rs-sys` baut `whisper.cpp` aus
  den Quellen **und** generiert Bindings über `bindgen`. Nötig sind daher CMake, die
  Visual-Studio-C++-Build-Tools **und LLVM/clang mit gesetztem `LIBCLANG_PATH`**,
  dazu das SDK des gewählten GPU-Backends. Landmine: schlägt die Bindgen-Generierung
  unter MSVC fehl, ist `WHISPER_DONT_GENERATE_BINDINGS` der dokumentierte Ausweg.
- Die GitHub-`windows-latest`-Runner haben kein Audiogerät und keine nutzbare GPU.
  **CI lädt kein Modell**: alle Tests dort laufen gegen eine skriptbare
  `SpeechToText`-Attrappe.
- **Versionsstand:** geprüft wurde `whisper-rs` **0.14.3** (Quelltext, 2026-07-30).
  Aktuell ist die 0.16er-Reihe, und `0.15.0` ist **yanked** — `deny.toml` hat
  `yanked = "deny"`. Gepinnt wird auf die aktuelle, nicht-yanked Minor; die zwei
  tragenden Befunde (Compile-Time-Backends, `use_gpu` zur Laufzeit) sind gegen die
  gepinnte Version **erneut zu belegen**, bevor die Ladder gebaut wird.

## Prior art

- [Local speech-to-text engine (Phase 2)](../prior-art.md#local-speech-to-text-engine-phase-2)
  — `large-v3-turbo` als Standard, kleinere Modelle als CPU-Ladder, und der Trait,
  hinter dem Parakeet später Platz hat.
- [Produktgestalt und Desktop-Stack (Phase 5)](../prior-art.md#produktgestalt-und-desktop-stack-phase-5)
  — die GPU-Backend-Matrix als **Form** der Ladder.
- [Auslieferung und Modellbereitstellung (Phase 6)](../prior-art.md#auslieferung-und-modellbereitstellung-phase-6)
  — Modelle beim ersten Start laden statt paketieren. **Achtung:** derselbe Eintrag
  behauptet „ein Installer, zur Laufzeit gewähltes Backend". Das ist mit
  `whisper-rs-sys` nicht erreichbar; Korrektur 4 zieht die Zeile nach. Die offene
  Ladder-Entscheidung unten steht also **nicht** gegen die Prior Art, sondern
  korrigiert sie.

## Human prerequisites

- [ ] CMake, die Visual-Studio-C++-Build-Tools **und LLVM/clang** lokal installiert,
      `LIBCLANG_PATH` gesetzt. Ohne sie scheitert ab dieser Phase schon
      `cargo build` — `whisper-rs-sys` generiert seine Bindings mit `bindgen`.
- [ ] Das SDK des am Gate gewählten GPU-Backends (Vulkan SDK bzw. CUDA Toolkit).
- [ ] Eine GPU mit ≥ 8 GB VRAM für den GPU-Zweig des QA-Gates — oder die ausdrückliche
      Feststellung, dass nur der CPU-Pfad geprüft werden kann.
- [ ] Bestätigung, dass die öffentlichen Referenzsamples von einem
      **Entwickler-Werkzeug** (`cargo xtask wer`) in ein Verzeichnis außerhalb des
      Repositories geladen werden dürfen — die ausgelieferte Anwendung tut das nicht.
- [ ] Falls die Wahl auf ein selbst aufgenommenes deutsches Meeting-Sample fällt: das
      Sample, ein Referenztranskript und die Einverständnisse der Sprechenden.
- [ ] Keine Secrets, keine Accounts, keine API-Keys — die Quellen sind öffentlich und
      unauthentifiziert.

## Prior decisions

### `SpeechToText` — die Nahtstelle

| Decision | Rationale | Date |
|---|---|---|
| `trait SpeechToText { fn transcribe(&mut self, pcm_16k_mono: &[f32], window_start: SessionOffset, language: Language) -> Result<Vec<RawSegment>, AsrError>; fn provenance(&self) -> TranscriptProvenance; }` — `RawSegment` trägt **fensterrelative** Zeiten und Text | `&mut self`, weil die whisper-Implementierung einen `WhisperState` hält. Die Eingabe ist bereits 16 kHz mono, also genau das, was der Resampler aus Phase 1 auf der Leseseite liefert. Fensterrelative Zeiten, weil nur der Scheduler die absolute Achse kennt | 2026-07-30 |
| Die **Ladder liegt über dem Trait**, nicht darin: sie wählt Backend, Modell und Fensterparameter und konstruiert daraus eine `SpeechToText`-Implementierung | Sonst müsste jede künftige Engine die Ladder-Logik nachbauen, und die Zusage „Parakeet kommt ohne Änderung an der Pipeline dazu" wäre unbelegt | 2026-07-30 |
| Beide Ströme werden transkribiert, über **einen** `WhisperContext` mit **zwei** `WhisperState`s, seriell je Hop — nicht parallel | Phase 3 und 4 brauchen den `Local`-Text für das strukturelle „Ich" (`docs/architecture.md`, Flow 3); nur `Remote` zu transkribieren würde diese Phase formal grün abnehmbar machen und Phase 3 einen halben Datenpfad hinterlassen. Ein Kontext spart den doppelten Modellspeicher; seriell, weil zwei gleichzeitige GPU-Läufe die Latenz beider verschlechtern statt einen zu beschleunigen | 2026-07-30 |
| Reicht die Kapazität nicht für beide Ströme, behält **`Remote` Priorität** und `Local` stuft zuerst ab (größerer Hop) | Der Nutzer weiß, was er selbst gesagt hat; die Gegenseite ist der Grund, warum das Werkzeug existiert. Die Abstufung wird gemeldet, nicht verschwiegen | 2026-07-30 |
| Sprachwahl je Sitzung: `de`, `en` oder `auto`. `auto` erkennt **einmal** auf den ersten Sekunden des `Remote`-Stroms und wird dann festgeschrieben | Pro Fenster neu zu erkennen erzeugt Sprachwechsel mitten im Gespräch, die schlechter sind als eine falsche, aber stabile Wahl | 2026-07-30 |

### Fenster-Scheduling und die Latenzrechnung

| Decision | Rationale | Date |
|---|---|---|
| Der **Redundanzfaktor** ist explizit: `Kontext ÷ Hop`. Bei 15 s Kontext und 3 s Hop wird jede Sekunde Audio **fünfmal** encodiert. Über zwei Ströme verdoppelt sich das | Das ist die Größe, die die Machbarkeit bestimmt, und sie fehlte in der ersten Fassung dieser Spec. Ohne sie wirkt „3 s Hop lässt 2 s für die Inferenz" wie eine Rechnung, ist aber keine | 2026-07-30 |
| `audio_ctx` wird **gesetzt** (`set_audio_ctx`), passend zur Kontextlänge — Richtwert 1500 für 30 s, also rund 768 für 15 s | Whispers Encoder hat eine **feste 30-s-Eingabe** und padded kürzeres Audio: ein 15-s-Fenster kostet ohne gesetztes `audio_ctx` genauso viel Encoder-Zeit wie ein 30-s-Fenster. Die Kontextverkürzung kauft also **nichts**, solange dieser Wert nicht gesetzt ist. Er ist qualitätswirksam (Halluzinationsrisiko bei zu kleinen Werten), deshalb: setzen, und den Effekt in der WER-Messung mitmessen | 2026-07-30 |
| Harte Schranke statt „schneller als Echtzeit": **Inferenz ≤ 2,0 s je Fenster**, gemessen. Das QA-Kriterium lautet `Hop + Inferenz + Pipeline-Overhead ≤ 5 s` | „Schneller als Echtzeit" (1×) und „Inferenz kürzer als der Hop" (< 3 s) sind beide erfüllbar, während das 5-s-Kriterium reißt — bei 2,9 s Inferenz und 3 s Hop sind es 5,9 s. Die Akzeptanzpunkte müssen dasselbe messen wie die Vision | 2026-07-30 |
| Absolute Zeitachse: der Scheduler addiert den Fensteranfang auf der **Sitzungs-Zeitachse aus Phase 1** zu den fensterrelativen Zeiten des `RawSegment` | Genau die Nahtstelle, über die `docs/architecture.md`, Flow 3, die Sprecherlabels „über Zeitüberlappung" legt. Fensterrelative Zeiten wären dort unbrauchbar | 2026-07-30 |
| **Dedup-Regel:** für den Überlappungsbereich gewinnt immer das **jüngere** Fenster. `Final` wird ein Segment, sobald es **vollständig vor** dem Commit-Horizont `jetzt − (Kontext − Hop)` liegt; der Schnitt fällt auf die letzte whisper-**Segmentgrenze** vor dem Horizont, nie mitten in ein Segment | Whisper tokenisiert dasselbe Audio in zwei Fenstern unterschiedlich, also ist ein Token-Vergleich (LCS) fragil. Die von whisper selbst gelieferten Segmentgrenzen sind die einzigen stabilen Schnittpunkte. „Jüngeres Fenster gewinnt" ist richtig, weil es mehr rechten Kontext hatte | 2026-07-30 |
| Segmente haben eine **`id`** (monoton je Strom). `Provisional` → `Final` ist ein Ersetzen über diese `id`, kein Anhängen | Ohne Identität kann ein Konsument provisorischen Text nicht ersetzen, sondern nur doppelt anzeigen | 2026-07-30 |
| Bei **Sitzungsende** werden alle noch im Kontextfenster liegenden Segmente in einem letzten Lauf finalisiert (Flush), bevor die Sitzung `Ended` erreicht | Ohne Flush hätte der letzte Satz jeder Sitzung keinen Übergang nach `Final` — und das Vision-Budget „fertiges Protokoll ≤ 60 s nach Sitzungsende" muss diesen Lauf enthalten | 2026-07-30 |
| Fällt die Inferenz dauerhaft hinter den Hop zurück, wird der Hop **nicht** stillschweigend verlängert: die Sitzung meldet den Rückstand und die Ladder stuft ab | Ein still wachsender Hop würde das Latenzkriterium unbemerkt verletzen. Abstufen ist die ehrliche Reaktion: schlechterer Text, gehaltene Latenz | 2026-07-30 |
| Der Scheduler bekommt eine **injizierbare Zeitquelle** | Sonst braucht ein Test für zwei Fenstergrenzen ≥ 18 s Wanduhr-Zeit — in genau dem Verify, dessen Dauer diese Phase neu messen will | 2026-07-30 |

```
Zeit ─────────────────────────────────────────────────────────────────▶
                                                    jetzt ┤
Ringpuffer   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  30 s, fest

Fenster n−1        ├────── 15 s Kontext ──────┤
Fenster n             ├────── 15 s Kontext ──────┤
Fenster n+1              ├────── 15 s Kontext ──────┤
                         ├ 3 s ┤ Hop

Commit-Horizont                       ┤ jetzt − (15 − 3) = jetzt − 12 s
                   ─── Final ─────────┤├───── Provisional ─────┤
                   (aus dem Fenster       (wird bei jedem Hop
                    gelaufen, Schnitt      neu erzeugt, jüngeres
                    auf Segmentgrenze)     Fenster gewinnt)
```

Ein Segment wird also rund 12–15 s nach dem Gesprochenen `Final`, während
provisorischer Text nach `Hop + Inferenz` erscheint. Das Vision-Kriterium ≤ 5 s
bezieht sich auf **Live-Rohtext**, also auf `Provisional`.

### Ladder-Stufen

| Stufe | Backend | Modell | Kontext / Hop | Redundanz | Latenz-Erwartung |
| --- | --- | --- | --- | --- | --- |
| 1 | GPU | `large-v3-turbo` q5_0 | 15 s / 3 s | 5× | ≤ 5 s, Kriterium gehalten |
| 2 | CPU | `small` | 10 s / 5 s | 2× | > 5 s erwartet — siehe offene Entscheidung |
| 3 | CPU | `base` | 10 s / 5 s | 2× | > 5 s erwartet |

| Decision | Rationale | Date |
|---|---|---|
| Die Ladder stuft **Backend, Modell und Fensterparameter gemeinsam** ab | Nur das Modell zu verkleinern lässt den Redundanzfaktor unberührt — die teuerste Größe. Eine CPU-Stufe mit 5× Redundanz ist von vorn herein aussichtslos | 2026-07-30 |
| Standardmodell `large-v3-turbo` in **q5_0** | `docs/prior-art.md` legt `large-v3-turbo` fest. Die Quantisierung entscheidet vor allem die Downloadgröße (~570 MB gegen ~1,6 GB f16), und „Time-to-first-protocol ≤ 15 Minuten" ist download-dominiert. Reißt Englisch dadurch die 15-%-WER, ist f16 der Ausweg — dann ist das Download-Kriterium neu zu bewerten | 2026-07-30 |
| Laufzeit-Fallback über `use_gpu(false)` nach einem fehlgeschlagenen GPU-Versuch | Am Quelltext geprüft: `use_gpu` ist per Default `cfg!(feature = "_gpu")` und zur Laufzeit umschaltbar. Ein einziges GPU-fähiges Binary kann damit echt auf CPU zurückfallen | 2026-07-30 |
| ggml-Logging über das Feature `log_backend` in `log` leiten | whisper.cpp schreibt sonst ungefiltert auf stderr | 2026-07-30 |

### Transkript-Eigentum und Modellbereitstellung

| Decision | Rationale | Date |
|---|---|---|
| **`session` hält das Transkript autoritativ im Arbeitsspeicher.** Der Event-Bus ist ausschließlich **Benachrichtigung**; ein Abonnent mit `Lagged` liest den autoritativen Stand neu | Phase 1 hat den Bus als `tokio::sync::broadcast` entschieden — ein zurückgefallener Abonnent **verliert** Nachrichten. Läge der Protokolltext nur dort, wäre verlorener Text verlorenes Protokoll, und Phase 4 hätte keine Quelle | 2026-07-30 |
| `cli` ist auch hier der **Composition Root**: es ruft `provisioning`, verifiziert das Modell und übergibt den Pfad an die Ladder | Setzt die Entscheidung aus Phase 1 fort. `session` bleibt frei von Bereitstellung, `asr` frei von Netz | 2026-07-30 |
| Fehlt das Modell und scheitert der Download, wird die Sitzung **nicht gestartet** — der Fehler erscheint **vor** dem Consent-Schritt | Anders als beim fehlenden Mikrofon in Phase 1 (dort läuft eine `Remote`-only-Sitzung sinnvoll weiter) wäre eine Sitzung ohne ASR eine Sitzung ohne Ergebnis. Und niemand soll eine Attestation für eine Erfassung bestätigen, die nichts erzeugen kann | 2026-07-30 |
| `provisioning` entsteht in dieser Phase, minimal: Allowlist mit Version, URL und SHA-256, Prüfung, Cache-Verzeichnis via `directories` | `asr` ist ohne Modell nicht lauffähig. Die Constitution erlaubt Netzzugriff nur „aus einer versionierten Allowlist mit SHA-256-Prüfung" — der Mechanismus muss beim **ersten** Download existieren, nicht erst in Phase 6 | 2026-07-30 |

### Referenzsamples, WER und Gates

| Decision | Rationale | Date |
|---|---|---|
| Englisches Referenzsample: ein **namentlich und mit Offsets festgelegter** Auszug aus dem **AMI Meeting Corpus** (CC-BY-4.0), Meeting-ID und Zeitbereich in der Allowlist des WER-Werkzeugs dokumentiert. Die CC-BY-Namensnennung kommt in die NOTICE-Datei | Am 2026-07-30 recherchiert: 100 h echte Mehrsprecher-Besprechungen, Korpus und Annotationen CC-BY-4.0, der kanonische Meeting-Benchmark. Ohne festgelegten Auszug ist die WER-Zahl nicht reproduzierbar, und CC-BY verlangt die Nennung | 2026-07-30 |
| Das **Referenztranskript** wird aus den AMI-Annotationen gewonnen; der Parser dafür ist eingeplante Arbeit des WER-Werkzeugs, keine Nebensache | AMI-Annotationen liegen als NXT-XML bzw. als Parquet-Datensatz vor. Das stillschweigend anzunehmen wäre eine versteckte Aufgabe im Harness | 2026-07-30 |
| Der Bezug der Referenzsamples liegt in **`xtask`**, nicht in `provisioning`, und die Allowlist von `provisioning` bleibt **modellrein** | Die Constitution erlaubt „**kein Netzwerkzugriff außer dem Modell-Download**" und macht jede weitere URL im Code zum Merge-Blocker. Benchmark-Audio ist kein Modell. Läge der Bezug in `provisioning`, bekäme die Allowlist der **ausgelieferten** Anwendung Nicht-Modell-URLs für Sprachaufnahmen. In `xtask` ist es ein Entwickler-Werkzeug, das nie ausgeliefert wird | 2026-07-30 |
| Dass ein öffentliches Benchmark-Sample überhaupt auf Platte landet, ist eine **bewusst offengelegte Abweichung** von `docs/constitution.md`, Don'ts („Kein Speichern von Roh-Audio, auch nicht temporär, auch nicht ‚nur zum Debuggen'") und von `CLAUDE.md`, Regel 1. Die Ausnahme wird **in `docs/constitution.md` geschrieben**, nicht nur hier | Der Zweck jener Regel ist der Erfassungspfad: Audio aus der Sitzung eines Nutzers darf nirgends landen. Ein öffentliches, lizenziertes Benchmark-Sample entsteht nicht aus einer Sitzung und wird von der Anwendung nie geschrieben. Aber der Wortlaut deckt es mit, und der synthetische Testton kann WER nicht messen — also gehört die Ausnahme ins normative Dokument, sonst widerspricht die Regel dem Vorgehen dauerhaft | 2026-07-30 |
| Die WER-Messung ist **opt-in** (`cargo xtask wer`), nicht Teil von Verify. Verify lädt **kein** Modell; alle CI-Tests laufen gegen eine skriptbare `SpeechToText`-Attrappe | Verify ist das Gate je Iteration und muss schnell und netzfrei bleiben. Ein CI-Test, der ein Modell lädt, unterläuft genau die Begründung, mit der die WER-Messung ausgelagert wird | 2026-07-30 |
| WER wird nach festgeschriebener Normalisierung gemessen (Kleinschreibung, Satzzeichen entfernt, Zahlwörter vereinheitlicht), Normalisierung mit dem Ergebnis dokumentiert | Ohne festgeschriebene Normalisierung ist die Zahl nicht vergleichbar — man kann sie über die Regelwahl um mehrere Punkte verschieben | 2026-07-30 |
| Kein `/loopkit:design`-Zyklus. Das Diagramm oben ist Teil der Spec, **nicht** eine Durable form nach `docs/design.md` | Phase 2 hat keine UI-Fläche, und die Live-Textansicht der CLI ist Textausgabe — der Verzicht ist auf diesem Grund legitim. Die Durable-form-Regel gilt für das Ergebnis eines Design-Zyklus; da keiner läuft, gibt es kein Artefakt, das ihr genügen müsste | 2026-07-30 |
| OPEN — GPU-Ladder: die Backends von `whisper-rs` sind Compile-Time-Features, ein Binary trägt genau eines. `docs/constitution.md` nennt „CUDA-/Vulkan-/CPU-Ladder", was so nicht in **einen** Installer passt | resolved at the spec-acceptance gate | — |
| OPEN — deutsches Referenzsample: es existiert kein permissiv lizenziertes deutsches **Meeting**-Korpus. Das Outcome fordert weiter ≤ 15 %; offen ist, **welches** Sample gemessen wird | resolved at the spec-acceptance gate | — |
| OPEN — darf der CPU-Pfad das 5-s-Latenzkriterium verfehlen? `docs/vision.md` führt „Latenz ≤ 5 s" und „funktionierender CPU-Fallback" als **getrennte** Kriterien; die Rechnung oben zeigt, dass eine CPU-Stufe die 5 s realistisch nicht hält | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 2 — Lokale Transkription (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist
- `Depends on milestone: #1`. Die Issues, die Ringpuffer, Sitzung und CLI berühren,
  tragen Cross-Milestone-Kanten auf die entsprechenden Phase-1-Issues.
- **Kontext:** Phase 1 ist als Spec gemergt, aber noch **nicht implementiert**. Jede
  hier zitierte Phase-1-API (Leser-Cursor, Sitzungs-Zeitachse, Event-Bus,
  Testton-Quelle, Audit-Wachstum) ist ein Versprechen, kein Code. Die
  Cross-Milestone-Kanten sind deshalb bindend, nicht dekorativ.

Jedes Issue verweist im Body auf diesen Spec-Pfad.

## Verification

Maschinell, in Verify und in CI auf `windows-latest` — **ohne Modell und ohne Netz**:

- [ ] `cargo xtask verify` grün; `cargo xtask build` grün.
- [ ] `cargo deny check` grün mit dem auf `whisper-rs`/`whisper-rs-sys` begrenzten
      Unlicense-`exceptions`-Eintrag und Einträgen für alle Crates aus dem Inventar.
- [ ] Der Quell- und Manifest-Audit bewacht `asr`, findet dort keinen Schreib-, Netz-
      oder `serde`-Pfad, und belegt, dass ein HTTP-Client **nur** in `provisioning`
      vorkommt.
- [ ] `provisioning`-Test: falsche SHA-256 wird abgewiesen und nichts landet im Cache;
      eine URL außerhalb der Allowlist wird abgewiesen.
- [ ] Scheduler-Test gegen eine **skriptbare** `SpeechToText`-Attrappe mit bekannter
      Ausgabe und **injizierter Zeitquelle**: die Fenstergrenzen entstehen wie
      spezifiziert, im Überlappungsbereich gewinnt das jüngere Fenster, der Schnitt
      fällt auf eine Segmentgrenze, und `Provisional` wird über die `id` durch `Final`
      ersetzt.
- [ ] Zeitachsen-Test: fensterrelative `RawSegment`-Zeiten werden korrekt auf die
      Sitzungs-Zeitachse aus Phase 1 abgebildet.
- [ ] Flush-Test: bei `stop()` erhalten alle noch provisorischen Segmente einen
      `Final`-Zustand, bevor die Sitzung `Ended` erreicht.
- [ ] Rückstands-Test: eine künstlich verlangsamte Attrappe führt zu gemeldetem
      Rückstand und Abstufung — nicht zu einem still verlängerten Hop.
- [ ] Verlust-Test: ein Abonnent, der `Lagged` erhält, kann das vollständige
      Transkript aus dem autoritativen Speicher rekonstruieren.
- [ ] Zwei-Strom-Test: `Local` und `Remote` erzeugen getrennte Segmentfolgen mit
      korrekter `stream`-Zuordnung; unter Kapazitätsdruck stuft `Local` zuerst ab.

Manuell am Milestone-QA-Gate (Smoke-Test nach `docs/workflow.md`):

- [ ] **Latenz GPU:** gesprochener Satz erscheint als provisorischer Rohtext ≤ 5 s
      später. Protokolliert werden Hop, gemessene Inferenzzeit je Fenster und der
      Pipeline-Overhead — die Summe ist das Kriterium, nicht die Inferenz allein.
- [ ] **Inferenz-Schranke:** ≤ 2,0 s je 15-s-Fenster auf der 8-GB-GPU, mit gesetztem
      `audio_ctx`, bei zwei aktiven Strömen.
- [ ] **CPU-Fallback:** GPU deaktiviert → die Sitzung läuft auf Stufe 2 weiter, die
      Herkunftsangabe zeigt den Wechsel, die Latenz wird gemessen und gegen die am
      Gate getroffene Entscheidung bewertet.
- [ ] **WER Englisch:** `cargo xtask wer` auf dem festgelegten AMI-Auszug ergibt
      ≤ 15 %, mit dokumentierter Normalisierung und `audio_ctx`-Wert.
- [ ] **WER Deutsch:** ≤ 15 % auf dem am Gate gewählten Sample, gleiche
      Dokumentation.
- [ ] **Modellbereitstellung:** auf einer Maschine ohne Cache wird das Modell geladen,
      die Prüfsumme verifiziert und die Zeit bis zum ersten Text notiert — der
      Eingangswert für das 15-Minuten-Kriterium in Phase 6.
- [ ] **Fehlendes Modell:** ohne Netz und ohne Cache erscheint der Fehler **vor** dem
      Consent-Schritt und es startet keine Sitzung.
- [ ] **Null Bytes bleibt gültig:** der Sitzungs-FS-Audit aus Phase 1 läuft weiter
      grün, mit angehängtem ASR-Konsumenten (gegen die Attrappe, ohne Modell).

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Die 5-s-Latenz wird trotz gesetztem `audio_ctx` verfehlt, weil zwei Ströme × 5× Redundanz die GPU überfordern | Die Inferenz-Schranke von 2,0 s ist ein eigener, früh messbarer Akzeptanzpunkt. Reißt sie, sind die Hebel in dieser Reihenfolge: `audio_ctx` senken, Kontext auf 10 s verkürzen, `Local` seltener transkribieren. Alle drei sind Konfiguration, keine Umbauten |
| Ein zu kleines `audio_ctx` verschlechtert die Qualität oder erzeugt Halluzinationen | Der gewählte Wert wird mit jeder WER-Zahl dokumentiert, damit Qualität und Latenz gegeneinander sichtbar sind statt einzeln optimiert |
| q5_0 kostet gegenüber f16 genug WER, um das englische 15-%-Ziel zu reißen | f16 ist der benannte Ausweg; dann ist das Download-Kriterium für Phase 6 neu zu bewerten. Die Messung entscheidet, nicht die Annahme |
| Die WER auf dem deutschen Sample verfehlt 15 %, weil das Sample kein Meeting ist | Die Sample-Wahl ist eine Gate-Entscheidung. Das Ergebnis wird mit der Sample-Herkunft dokumentiert; verfehlt es das Ziel, ist die Frage „falsches Sample oder falsches Modell" — und das Kriterium bleibt ≤ 15 %, statt auf „gemessen" abgesenkt zu werden |
| Der whisper.cpp-Build macht Verify und CI deutlich langsamer | Build-Cache in CI, Neumessung der Verify-Dauer und Korrektur in `docs/workflow.md` sind eigene Akzeptanzpunkte |
| `bindgen` scheitert unter MSVC und blockiert den Build vollständig | Als Landmine in den Constraints notiert, mit `WHISPER_DONT_GENERATE_BINDINGS` als dokumentiertem Ausweg. LLVM/libclang steht als Human prerequisite |
| Die gepinnte `whisper-rs`-Version verhält sich anders als die geprüfte 0.14.3 | Die zwei tragenden Befunde sind gegen die gepinnte Version erneut zu belegen, bevor die Ladder gebaut wird — als Constraint festgeschrieben |
| Phase 1 ist noch nicht implementiert, also können sich die hier angenommenen APIs beim Bauen noch verschieben | Cross-Milestone-Kanten auf die Phase-1-Issues, `Depends on milestone: #1`. Verschiebt sich eine API, eskaliert das betroffene Issue mit `needs:planning` statt eine Krücke zu bauen |

## Decision log

- 2026-07-30: `whisper-rs` v0.14.3 in einem lesenden, wegwerfbaren Klon außerhalb des
  Repos geprüft. Belegt: die GPU-Backends sind **Compile-Time-Features** (`cuda`,
  `vulkan`, `metal`, `hipblas`, `intel-sycl`, `coreml`), es gibt keinen
  Streaming-Beispielpfad in der Bindung, `log_backend` existiert, und `use_gpu` ist
  zur Laufzeit umschaltbar mit `cfg!(feature = "_gpu")` als Default. Genauigkeit
  nachgetragen: `coreml` setzt `_gpu` **nicht**, ist für Windows aber folgenlos. Kein
  externer Code ins Repo übernommen.
- 2026-07-30: Lizenz von `whisper-rs`/`whisper-rs-sys` ist **Unlicense** und fehlt in
  der `deny.toml`-Allowlist — ein vorab gefundener Merge-Blocker. Eingetragen wird er
  als auf diese zwei Crates begrenzte `exceptions`, nicht als globale Erlaubnis.
- 2026-07-30: Recherche zu den Referenzsamples. Englisch gelöst (AMI, CC-BY-4.0,
  echte Besprechungen). Deutsch nicht: ein permissiv lizenziertes deutsches
  Meeting-Korpus existiert nicht; VoxPopuli DE (CC0, 282 h) ist spontane
  Mehrsprecher-Sprache, aber parlamentarisch — ein Ersatz, kein Treffer.
- 2026-07-30: Review des Spec-Entwurfs durch einen frischen Agenten. Der tragende
  Fund war eine **falsche Latenzrechnung**: die erste Fassung übersah den
  Redundanzfaktor `Kontext ÷ Hop` (fünffaches Encodieren jeder Sekunde) und die feste
  30-s-Eingabe des whisper-Encoders, wegen der ein kürzerer Kontext ohne gesetztes
  `audio_ctx` **keine** Rechenzeit spart. Beides ist jetzt entschieden und beziffert.
  Weiter aufgedeckt und behoben: das deutsche WER-Kriterium war still von „≤ 15 %" zu
  „gemessen und dokumentiert" abgeschwächt; der Bezug der Referenzsamples lag im
  ausgelieferten `provisioning` statt im Entwickler-Werkzeug und hätte die
  Modell-Allowlist mit Audio-URLs gefüllt; der `Local`-Strom war erwähnt statt
  verdrahtet, obwohl Phase 3 ihn braucht und er die Last verdoppelt; das Transkript
  hatte keinen autoritativen Speicher, lag also allein auf einem verlustbehafteten
  Bus; und zwei CI-Tests hätten ein Modell geladen — genau das, womit die Spec die
  Auslagerung der WER-Messung begründet.
- 2026-07-30: Eine Feststellung des Reviews **nicht** übernommen: `intel-sycl` sei
  erst ab 0.16.0 vorhanden. Die geprüfte 0.14.3-`Cargo.toml` führt
  `intel-sycl = ["whisper-rs-sys/intel-sycl", "_gpu"]`. Der Rest des Befundes
  (`coreml` ohne `_gpu`; gevendorte C-Quellen sind für `cargo deny` unsichtbar) ist
  übernommen.
