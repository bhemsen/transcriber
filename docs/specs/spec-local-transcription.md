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
      dem Gesprochenen: `Hop 3 s + Inferenz ≤ 1,3 s + Overhead ≤ 0,3 s = 4,6 s`.
      **Bewusste Eingrenzung eines normativen Kriteriums:** `docs/vision.md` nennt
      „Live-Rohtext ≤ 5 s" ohne Strom-Angabe; diese Spec bindet es an `Remote` und
      gibt `Local` ein eigenes, größeres Budget.
- [ ] Die **Kapazitätsungleichung** hält: `I_Remote + I_Local ≤ Hop` in jedem Hop, in
      dem beide laufen — und `Hop ≤ Kontext` für jeden Strom. Ohne die erste fällt die
      Erkennung unbegrenzt zurück statt nur die Latenz zu reißen; ohne die zweite
      überspringen die Fenster Audio, statt es zu überlappen.
- [ ] Auf einer GPU mit 8 GB VRAM greift Stufe 1 der Ladder; ohne nutzbare GPU greift
      eine CPU-Stufe und erzeugt korrekten Text (die Latenz dort: siehe die offene
      Entscheidung unten).
- [ ] Jedes Segment verweist über eine `ProvenanceId` auf die Ladder-Stufe, die es
      erzeugt hat — ein Stufenwechsel mitten in der Sitzung ist am Segment sichtbar.
- [ ] Die WER liegt auf dem definierten **englischen** Referenzsample **≤ 15 %**.
- [ ] Die WER liegt auf dem definierten **deutschen** Referenzsample **≤ 15 %**.
- [ ] Das Modell wird nicht aus dem Repository und nicht aus dem Installer geladen,
      sondern über eine versionierte Allowlist mit SHA-256-Prüfung in ein
      Cache-Verzeichnis.
- [ ] `session` hält das Transkript autoritativ im Arbeitsspeicher; ein
      zurückgefallener Event-Abonnent verliert **keinen** Protokolltext.
- [ ] `asr` hat weiterhin keinen Schreib- und keinen Netzpfad; der Audit-Test bewacht
      es ab dieser Phase mit und belegt, dass ein HTTP-Client in keinem
      **ausgelieferten** Crate außer `provisioning` vorkommt. `xtask` ist ausdrücklich
      ausgenommen — es wird nie ausgeliefert und holt die Referenzsamples.
- [ ] `cargo xtask verify` bleibt grün, lokal und in CI, **ohne** ein Modell zu laden
      — mit gemessener und in `docs/workflow.md` korrigierter Laufzeit.
- [ ] Kein Foundation-Dokument widerspricht mehr dem Code (Liste in In scope).

## Scope

### In scope

- Crate `asr`: `SpeechToText`-Trait, `whisper-rs`-Implementierung, Fenster-Scheduling,
  Ladder, Sprachwahl, Deduplizierung des Überlappungsbereichs.
- Crate `provisioning` (neu, minimal): versionierte **Modell**-Allowlist,
  SHA-256-Prüfung, Cache-Verzeichnis. Der einzige Netzzugriff im ausgelieferten Stand.
- Crate `core`: `TranscriptSegment { id, stream, t0, t1, text, state, provenance }`
  — wobei `provenance: ProvenanceId` auf einen sitzungsweiten Eintrag verweist, statt
  Modell- und Backend-Namen je Segment zu duplizieren. Dazu
  `TranscriptProvenance`, `ProvenanceId`, `SessionOffset`, `Language`.
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
  6. `docs/architecture.md`, `session`-Zeile und Flow 2: dort ist das Transkript
     ausschließlich „Event an die Oberfläche". Mit dem autoritativen
     Transkript-Speicher unten ist das unvollständig — die Zeile bekommt den
     Speicher, der Bus die Rolle der Benachrichtigung.
  7. **`docs/constitution.md`, Don'ts, und `CLAUDE.md`, Regel 1** — die Ausnahme für
     öffentliche Benchmark-Samples. Entwurf, am Gate zu bestätigen: *„Ausgenommen ist
     ein öffentliches, permissiv lizenziertes Sprach-Benchmark-Sample, das
     ausschließlich ein Entwickler-Werkzeug (`cargo xtask wer`) außerhalb des
     Repositories ablegt, um die WER-Kriterien zu messen. Die ausgelieferte Anwendung
     schreibt weiterhin unter keinen Umständen Audio, und Audio aus einer Sitzung
     wird nie gespeichert."* Beide Dokumente werden geändert, nicht nur eines: die
     Constitution deckt das Speichern, `CLAUDE.md` Regel 1 deckt mit „kein ‚nur für
     diesen Test'" den Zweck. Eine Spec-lokale Ausnahme genügt nicht — Specs werden
     nach `docs/specs/archive/` verschoben, das Verbot bliebe absolut zurück.
  8. Abhängig von der Antwort auf OPEN 1: `docs/constitution.md`, Tech stack, nennt
     eine „CUDA-/Vulkan-/CPU-Ladder". Wird genau ein GPU-Backend ausgeliefert, wird
     die Zeile unwahr und ist mitzuziehen.
  9. Abhängig von der Antwort auf OPEN 3: darf der CPU-Pfad die 5 s verfehlen, ist
     **`docs/vision.md`, Success criteria**, mitzuziehen — die Zeile „Latenz:
     Live-Rohtext ≤ 5 s hinter dem Gesprochenen" gilt dann für den GPU-Pfad, und der
     CPU-Fallback bekommt eine eigene, benannte Erwartung. Ein normatives Kriterium
     wird nicht durch eine Spec eingeschränkt, sondern im Vision-Dokument.

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
  | `ureq` | HTTP für den Modell-Download in `provisioning` **und** für den Referenzsample-Bezug in `xtask` | MIT/Apache-2.0 | gedeckt |
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
- **CI baut ohne GPU-Feature** (reiner CPU-Build), damit auf dem Runner kein
  Vulkan-SDK oder CUDA-Toolkit installiert werden muss. Offengelegte Folge: dass der
  GPU-Feature-Build **kompiliert**, prüft CI nicht — das belegt der lokale
  `cargo xtask build` am QA-Gate, und er ist dort ein eigener Akzeptanzpunkt.
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
| `trait SpeechToText { fn transcribe(&mut self, pcm_16k_mono: &[f32], language: Language) -> Result<Vec<RawSegment>, AsrError>; fn provenance(&self) -> TranscriptProvenance; }`. `RawSegment` (in `asr`) trägt **fensterrelative** Zeiten und Text; `SessionOffset` und `ProvenanceId` liegen in `core` | `&mut self`, weil die whisper-Implementierung einen `WhisperState` hält. Die Eingabe ist bereits 16 kHz mono, also genau das, was der Resampler aus Phase 1 liefert. **Kein** `window_start`-Argument: die Implementierung darf die absolute Achse gar nicht kennen, sonst wäre der Parameter ein ungenutztes Versprechen — der Scheduler addiert den Offset | 2026-07-30 |
| Die **Ladder liegt über dem Trait**, nicht darin: sie wählt Backend, Modell und Fensterparameter und konstruiert daraus eine `SpeechToText`-Implementierung | Sonst müsste jede künftige Engine die Ladder-Logik nachbauen, und die Zusage „Parakeet kommt ohne Änderung an der Pipeline dazu" wäre unbelegt | 2026-07-30 |
| Beide Ströme werden transkribiert, über **einen** `WhisperContext` mit **zwei** `WhisperState`s, seriell je Hop — nicht parallel | Phase 3 und 4 brauchen den `Local`-Text für das strukturelle „Ich" (`docs/architecture.md`, Flow 3); nur `Remote` zu transkribieren würde diese Phase formal grün abnehmbar machen und Phase 3 einen halben Datenpfad hinterlassen. Ein Kontext spart den doppelten Modellspeicher; seriell, weil zwei gleichzeitige GPU-Läufe die Latenz beider verschlechtern statt einen zu beschleunigen | 2026-07-30 |
| Reicht die Kapazität nicht für beide Ströme, behält **`Remote` Priorität**: `Local` stuft zuerst ab, indem sein Hop über den Nennwert von 9 s hinaus vergrößert wird | Der Nutzer weiß, was er selbst gesagt hat; die Gegenseite ist der Grund, warum das Werkzeug existiert. Die Abstufung wird gemeldet, nicht verschwiegen. Der konkrete Nennwert und die Ungleichung, aus der er folgt, stehen unten — ohne sie wäre „stuft zuerst ab" eine Absicht ohne Zahl | 2026-07-30 |
| Sprachwahl je Sitzung: `de`, `en` oder `auto`. `auto` erkennt **einmal** auf den ersten Sekunden des `Remote`-Stroms und wird dann festgeschrieben | Pro Fenster neu zu erkennen erzeugt Sprachwechsel mitten im Gespräch, die schlechter sind als eine falsche, aber stabile Wahl | 2026-07-30 |

### Fenster-Scheduling und die Latenzrechnung

| Decision | Rationale | Date |
|---|---|---|
| Der **Redundanzfaktor** ist explizit: `Kontext ÷ Hop`. Bei 15 s Kontext und 3 s Hop wird jede Sekunde Audio **fünfmal** encodiert. Über zwei Ströme verdoppelt sich das | Das ist die Größe, die die Machbarkeit bestimmt, und sie fehlte in der ersten Fassung dieser Spec. Ohne sie wirkt „3 s Hop lässt 2 s für die Inferenz" wie eine Rechnung, ist aber keine | 2026-07-30 |
| `audio_ctx` wird **gesetzt** (`set_audio_ctx`), passend zur Kontextlänge — Richtwert 1500 für 30 s, also rund 768 für 15 s | Whispers Encoder hat eine **feste 30-s-Eingabe** und padded kürzeres Audio: ein 15-s-Fenster kostet ohne gesetztes `audio_ctx` genauso viel Encoder-Zeit wie ein 30-s-Fenster. Die Kontextverkürzung kauft also **nichts**, solange dieser Wert nicht gesetzt ist. Er ist qualitätswirksam (Halluzinationsrisiko bei zu kleinen Werten), deshalb: setzen, und den Effekt in der WER-Messung mitmessen | 2026-07-30 |
| **Drei Ungleichungen**, die nicht miteinander verrechnet werden dürfen. (1) **Latenz** `Remote`: `Hop + I + Overhead ≤ 5 s`, mit **Overhead ≤ 0,3 s** (Puffer-Lesen, Resampling, Dedup, Event). (2) **Kapazität**: `I_Remote + I_Local ≤ Hop` in jedem Hop, in dem beide laufen. (3) **`Hop ≤ Kontext`** für jeden Strom | „Schneller als Echtzeit" (1×) und „Inferenz kürzer als der Hop" (< 3 s) sind beide erfüllbar, während das 5-s-Kriterium reißt. Die Latenzschranke ist der Rest nach einem benannten Overhead, nicht das ganze Budget: 3 s Hop + 2,0 s Inferenz sind bereits 5,0 s und lassen für den Overhead null. Die Kapazitätsungleichung ist die eigentlich harte: hält sie nicht, fällt die Stufe **unbegrenzt** zurück statt bloß die Latenz zu reißen. Und `Hop ≤ Kontext` ist keine Feinheit — bei größerem Hop überlappen die Fenster nicht mehr, sie **überspringen**, und Audio wird lautlos nie transkribiert | 2026-07-30 |
| Innerhalb eines Hops läuft **`Remote` zuerst**, `Local` danach in der verbleibenden Zeit desselben Hops; passt es nicht, wird `Local`s Fenster auf den nächsten Hop verschoben | Das ist der Grund, warum `Local` die Latenz von `Remote` **nie** verschlechtert. Liefen beide an der Hop-Grenze um die Reihenfolge, würde in jedem dritten Hop `I_Local` vor `Remote`s nächstem Fenster liegen, und dessen Text käme `I_Local` später — bei einem Budget ohne Reserve reicht das, um die 5 s zu reißen | 2026-07-30 |
| `Local` hat den **größeren Hop**: **9 s** gegen 3 s bei `Remote` (Stufe 1), immer ≤ Kontext | Reduziert die Durchschnittslast auf ein Drittel, ohne die Kapazitätsschranke zu berühren — die gilt für den Hop, in dem beide laufen, unabhängig davon, wie selten das ist. Der Preis ist rund 11 s Latenz auf `Local` (`9 + I + Overhead`), was die Remote-Priorität ausdrücklich in Kauf nimmt: der Nutzer weiß, was er selbst gesagt hat | 2026-07-30 |
| Absolute Zeitachse: der Scheduler addiert den Fensteranfang auf der **Sitzungs-Zeitachse aus Phase 1** zu den fensterrelativen Zeiten des `RawSegment` | Genau die Nahtstelle, über die `docs/architecture.md`, Flow 3, die Sprecherlabels „über Zeitüberlappung" legt. Fensterrelative Zeiten wären dort unbrauchbar | 2026-07-30 |
| **Dedup-Regel:** für den Überlappungsbereich gewinnt immer das **jüngere** Fenster. `Final` wird ein Segment, sobald es **vollständig vor** dem Commit-Horizont `jetzt − (Kontext − Hop)` liegt; der Schnitt fällt auf die letzte whisper-**Segmentgrenze** vor dem Horizont, nie mitten in ein Segment | Whisper tokenisiert dasselbe Audio in zwei Fenstern unterschiedlich, also ist ein Token-Vergleich (LCS) fragil. Die von whisper selbst gelieferten Segmentgrenzen sind die einzigen stabilen Schnittpunkte. „Jüngeres Fenster gewinnt" ist richtig, weil es mehr rechten Kontext hatte | 2026-07-30 |
| Segmente haben eine **`id`** (monoton je Strom). `Provisional` → `Final` ist ein Ersetzen über diese `id`, kein Anhängen | Ohne Identität kann ein Konsument provisorischen Text nicht ersetzen, sondern nur doppelt anzeigen | 2026-07-30 |
| Bei **Sitzungsende** werden alle noch im Kontextfenster liegenden Segmente in einem letzten Lauf finalisiert (Flush), bevor die Sitzung `Ended` erreicht | Ohne Flush hätte der letzte Satz jeder Sitzung keinen Übergang nach `Final` — und das Vision-Budget „fertiges Protokoll ≤ 60 s nach Sitzungsende" muss diesen Lauf enthalten | 2026-07-30 |
| Fällt die Inferenz dauerhaft hinter den Hop zurück, wird der Hop **nicht** stillschweigend verlängert: die Sitzung meldet den Rückstand und die Ladder stuft ab | Ein still wachsender Hop würde das Latenzkriterium unbemerkt verletzen. Abstufen ist die ehrliche Reaktion: schlechterer Text, gehaltene Latenz | 2026-07-30 |
| Der Scheduler bekommt eine **injizierbare Zeitquelle** | Sonst braucht ein Test für zwei Fenstergrenzen ≥ 18 s Wanduhr-Zeit — in genau dem Verify, dessen Dauer diese Phase neu messen will | 2026-07-30 |

Schematisch für `jetzt = 30 s` Sitzungszeit. **Maßgeblich sind die Klammerwerte, nicht
die Balkenbreiten** — die Zeichnung ist keine Skala.

```
Ringpuffer    ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓    30 s, fest

Remote — Kontext 15 s, Hop 3 s
  Fenster n−1  ├──── Kontext ────┤                (  9 … 24 )
  Fenster n       ├──── Kontext ────┤             ( 12 … 27 )
  Fenster n+1        ├──── Kontext ────┤          ( 15 … 30 )
                  ├Hop┤                           (   3 s   )

Commit-Horizont = jetzt − (Kontext − Hop) = 30 − 12       ( 18 s )
  Segmente     ─── Final ───┤├─── Provisional ───┤
                Schnitt auf   jüngeres Fenster gewinnt;
                whisper-       wird bei jedem Hop ersetzt
                Segmentgrenze
                vor 18 s

Local — Kontext 15 s, Hop 9 s: ein Fenster je drei Remote-Hops,
        und stets NACH Remote innerhalb desselben Hops
  Fenster m          ├──── Kontext ────┤          ( 15 … 30 )
```

Ein Segment wird also rund 12–15 s nach dem Gesprochenen `Final`, während
provisorischer Text nach `Hop + Inferenz` erscheint. Das Vision-Kriterium ≤ 5 s
bezieht sich auf **Live-Rohtext**, also auf `Provisional`.

### Ladder-Stufen

| Stufe | Backend | Modell | Kontext | Hop `Remote` | Hop `Local` | Redundanz `Remote` | Inferenz-Schranke `I` | Latenz `Remote` |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | GPU | `large-v3-turbo` q5_0 | 15 s | 3 s | 9 s | 5× | **≤ 1,3 s** | `3 + 1,3 + 0,3 = 4,6 s` ✓ |
| 2 | CPU | `small` | 10 s | 5 s | 8 s | 2× | **≤ 2,5 s** | > 5 s — siehe OPEN 3 |
| 3 | CPU | `base` | 10 s | 5 s | 8 s | 2× | **≤ 2,5 s** | > 5 s — siehe OPEN 3 |

So entstehen die Schranken, damit sie nachrechenbar sind und niemand sie aus der
falschen Ungleichung ableitet:

- **Stufe 1** ist von der **Kapazität** gebunden, nicht von der Latenz: beide Ströme
  nutzen dasselbe 15-s-Fenster, kosten also gleich viel, und `I + I ≤ 3 s` ergibt
  `I ≤ 1,5 s`. Abgerundet auf **1,3 s** für Reserve. Die Latenz ist damit automatisch
  erfüllt (`4,6 ≤ 5 s`) — die Latenzformel allein hätte `I ≤ 1,7 s` erlaubt und die
  Kapazität gerissen.
- **Stufe 2 und 3** sind ausschließlich von der Kapazität gebunden: `I + I ≤ 5 s`
  ergibt `I ≤ 2,5 s`. Die Latenzungleichung ist dort **unerfüllbar**, weil schon der
  Hop 5 s beträgt — genau das ist der Gegenstand von OPEN 3.
- `Hop ≤ Kontext` hält in allen Stufen (9 ≤ 15; 8 ≤ 10). Auch die
  Abstufungs-Reaktion „`Local`-Hop vergrößern" endet an dieser Grenze, nicht bei
  einem beliebigen Wert.

Stufe 3 ist die letzte: hält **sie** die Kapazitätsungleichung nicht, gibt es nichts
darunter, und die Sitzung fällt unbegrenzt zurück. Das ist der härtere Teil von
OPEN 3 — nicht bloß „die Latenz ist schlechter".

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
| OPEN — darf der CPU-Pfad das 5-s-Latenzkriterium verfehlen? `docs/vision.md` führt „Latenz ≤ 5 s" und „funktionierender CPU-Fallback" als **getrennte** Kriterien; die Rechnung oben zeigt, dass eine CPU-Stufe die 5 s nicht hält, weil schon der Hop 5 s beträgt. **Ein „ja" ändert `docs/vision.md`, Success criteria** — Korrektur 9 zieht die Zeile dann mit, genau wie Korrektur 7 die Audio-Ausnahme. Es ist keine Spec-Detailfrage. Mitzuentscheiden ist der härtere Teil: hält Stufe 3 auch die **Kapazitäts**-Ungleichung nicht, fällt die Sitzung unbegrenzt zurück, und darunter liegt nichts mehr | resolved at the spec-acceptance gate | — |

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
      oder `serde`-Pfad, und belegt, dass ein HTTP-Client in keinem **ausgelieferten**
      Crate außer `provisioning` vorkommt — `xtask` ist als nie ausgeliefertes
      Entwickler-Werkzeug ausgenommen, und der Audit hält diese Ausnahme explizit,
      nicht implizit.
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
- [ ] **Inferenz-Schranke:** ≤ 1,3 s je 15-s-Fenster auf der 8-GB-GPU, mit gesetztem
      `audio_ctx` — und der gemessene Overhead ≤ 0,3 s.
- [ ] **Kapazität:** in jedem Hop, in dem beide Ströme laufen, gilt
      `I_Remote + I_Local ≤ Hop`. Über eine zehnminütige Sitzung wächst der Rückstand
      **nicht monoton** — das ist die Prüfung, die eine dauerhaft zurückfallende Stufe
      von einer bloß langsamen unterscheidet.
- [ ] **Reihenfolge:** `Remote` läuft in jedem Hop zuerst; ein `Local`-Fenster
      verschlechtert die `Remote`-Latenz messbar **nicht** (Vergleich einer Sitzung mit
      und ohne `Local`-Strom).
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
| Die 1,3-s-Schranke wird trotz gesetztem `audio_ctx` verfehlt, weil zwei Ströme × 5× Redundanz die GPU überfordern | Latenz- und Kapazitätsschranke sind getrennte, früh messbare Akzeptanzpunkte — die eine zu halten belegt die andere nicht. Reißt eine, sind die Hebel in dieser Reihenfolge: `audio_ctx` senken, `Local`-Hop vergrößern (Grenze: `Hop ≤ Kontext`, also 15 s), Kontext auf 10 s verkürzen, `Local` ganz aussetzen. Alle vier sind Konfiguration, keine Umbauten |
| Der autoritative Transkript-Speicher wächst mit der Gesprächsdauer, während Phase 1 „Speicherverbrauch unabhängig von der Laufzeit konstant" zusagt | Kein Widerspruch, aber eine Präzisierung wert: die Zusage aus Phase 1 gilt dem **Audio**-Ringpuffer. Text wächst, und zwar in einer Größenordnung (einige zehn KB je Stunde), die neben 23 MB Ringpuffer nicht ins Gewicht fällt |
| Ein zu kleines `audio_ctx` verschlechtert die Qualität oder erzeugt Halluzinationen | Der gewählte Wert wird mit jeder WER-Zahl dokumentiert, damit Qualität und Latenz gegeneinander sichtbar sind statt einzeln optimiert |
| q5_0 kostet gegenüber f16 genug WER, um das englische 15-%-Ziel zu reißen | f16 ist der benannte Ausweg; dann ist das Download-Kriterium für Phase 6 neu zu bewerten. Die Messung entscheidet, nicht die Annahme |
| Die WER auf dem deutschen Sample verfehlt 15 %, weil das Sample kein Meeting ist | Die Sample-Wahl ist eine Gate-Entscheidung. Das Ergebnis wird mit der Sample-Herkunft dokumentiert; verfehlt es das Ziel, ist die Frage „falsches Sample oder falsches Modell" — und das Kriterium bleibt ≤ 15 %, statt auf „gemessen" abgesenkt zu werden |
| Der whisper.cpp-Build macht Verify und CI deutlich langsamer | Build-Cache in CI, Neumessung der Verify-Dauer und Korrektur in `docs/workflow.md` sind eigene Akzeptanzpunkte |
| `bindgen` scheitert unter MSVC und blockiert den Build vollständig | Als Landmine in den Constraints notiert, mit `WHISPER_DONT_GENERATE_BINDINGS` als dokumentiertem Ausweg. LLVM/libclang steht als Human prerequisite |
| Die gepinnte `whisper-rs`-Version verhält sich anders als die geprüfte 0.14.3 | Die zwei tragenden Befunde sind gegen die gepinnte Version erneut zu belegen, bevor die Ladder gebaut wird — als Constraint festgeschrieben |
| Phase 1 ist noch nicht implementiert, also können sich die hier angenommenen APIs beim Bauen noch verschieben | Cross-Milestone-Kanten auf die Phase-1-Issues, `Depends on milestone: #1`. Verschiebt sich eine API, eskaliert das betroffene Issue mit `needs:planning` statt eine Krücke zu bauen |

## Decision log

- 2026-07-30: `whisper-rs` in einem lesenden, wegwerfbaren Klon außerhalb des Repos
  geprüft — **des Default-Branches**, der sich in `Cargo.toml` als 0.14.3 ausgibt,
  aber der **veröffentlichten** 0.14.3 voraus ist. Der Stand ist also „master", nicht
  0.14.3; die frühere Etikettierung war falsch. Belegt (und gegen die zu pinnende
  Version erneut zu belegen, siehe Constraints): die GPU-Backends sind
  **Compile-Time-Features** (`cuda`, `vulkan`, `metal`, `hipblas`, `coreml`, auf
  master zusätzlich `intel-sycl`), es gibt keinen Streaming-Beispielpfad in der
  Bindung, `log_backend` existiert, und `use_gpu` ist zur Laufzeit umschaltbar mit
  `cfg!(feature = "_gpu")` als Default. `coreml` setzt `_gpu` **nicht**, für Windows
  folgenlos. Gevendorte C-Quellen sieht `cargo deny` nicht — deren MIT-Status ist für
  die NOTICE relevant, nicht für das Gate. Kein externer Code ins Repo übernommen.
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
- 2026-07-30: Zweite Review-Runde. Der Reviewer hatte mit `intel-sycl` recht: es fehlt
  in der **veröffentlichten** 0.14.3 und kam erst mit 0.16.0. Der Widerspruch dagegen
  stützte sich auf einen `--depth 1`-Klon des Default-Branches, dessen `Cargo.toml`
  die Versionsnummer 0.14.3 trägt, aber neuer ist. Daraus folgt mehr als eine
  Korrektur: jede Aussage jenes Eintrags ist an einen Branch statt an eine Version
  gebunden, weshalb der Eintrag oben umbenannt ist und die Constraints eine erneute
  Prüfung gegen die gepinnte Version verlangen.
- 2026-07-30: Ebenfalls aus Runde zwei behoben — die Durchsatzfrage, die von der
  Latenzfrage getrennt ist: eine Inferenz-Schranke von 2,0 s bei 3 s Hop lässt für
  den Overhead null, und zwei Ströme mit gleichem Hop brauchen 3,4 s je 3-s-Hop, wären
  also in der Nennkonfiguration dauerhaft im Rückstand. Die daraus abgeleiteten
  Zahlen hat Runde drei noch einmal korrigiert (siehe unten); maßgeblich ist die
  Ladder-Tabelle. Dazu geschlossen: der Audit-Invariant, den das eigene `xtask`-Werkzeug
  gebrochen hätte; das fehlende CI-Feature-Set; die Heimat von `RawSegment`,
  `SessionOffset` und der Segment-Herkunft; und die Korrekturliste, die die
  zugesagte Constitution-Änderung nicht enthielt und sie damit aus der Definition von
  „fertig" herausfallen ließ.
- 2026-07-30: Dritte Review-Runde, zwei falsche Zahlen und ein stiller Datenverlust.
  Erstens war der Satz „die Inferenz-Schranke je Stufe ist `Hop − Overhead`" falsch —
  er hätte für Stufe 1 auf 2,7 s statt 1,3 s geführt und damit genau die Verwechslung
  von Hop-Budget und Latenz-Budget wiederholt, die Runde zwei beheben sollte. Die
  Schranken stehen jetzt mit ihrer Herleitung in der Ladder-Tabelle: Stufe 1 ist von
  der **Kapazität** gebunden (`I + I ≤ Hop`), nicht von der Latenz.
  Zweitens hatte `Local` in den CPU-Stufen einen Hop von 15 s bei 10 s Kontext. Ein
  Hop größer als der Kontext lässt die Fenster nicht überlappen, sondern
  **überspringen**: ein Drittel des eigenen Stroms wäre lautlos nie transkribiert
  worden, und der Commit-Horizont `jetzt − (Kontext − Hop)` hätte in der Zukunft
  gelegen. `Hop ≤ Kontext` ist jetzt eine benannte Ungleichung, die auch die
  Abstufungs-Reaktion begrenzt.
  Drittens war die Kapazitätsregel als Durchschnitt formuliert, obwohl sie im
  Kollisions-Hop gilt: in jedem dritten Hop hätten beide Ströme zusammen 3,4 s bei
  3 s Hop gebraucht und damit `Remote`s nächstes Fenster verzögert. Gelöst nicht
  durch eine Mittelung, sondern durch eine Reihenfolge: `Remote` läuft in jedem Hop
  zuerst, `Local` danach in der verbleibenden Zeit. Damit kann `Local` die
  `Remote`-Latenz nicht verschlechtern, und das ist prüfbar (Sitzung mit und ohne
  `Local`-Strom vergleichen).
  Außerdem: die Verification-Zeile des Audits war nicht mit dem Outcome mitgezogen und
  hätte die `xtask`-Ausnahme weiter gebrochen; OPEN 3 nennt jetzt `docs/vision.md` und
  hängt an Korrektur 9, statt seine Dokumentfolge nur zu erwähnen; und das Diagramm
  hat seine erfundene Skala verloren — die Klammerwerte sind maßgeblich, die Balken
  sind schematisch.
