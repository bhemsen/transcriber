# Spec: Phase 3 — Sprechertrennung

> Created: 2026-07-30

Diese Spec liefert die Sprecherzuordnung: VAD und overlap-aware Segmentierung auf dem
`Remote`-Strom, inkrementelle Embedding-Extraktion während der Sitzung, **eigenes**
Clustering am Sitzungsende, und die Zuordnung der Labels auf die Transkript-Segmente
über Zeitüberlappung. Sie trägt das zweite Kernversprechen des Projekts: **kein
Stimmprofil überlebt die Sitzung**.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] Am Sitzungsende tragen die Segmente des `Remote`-Stroms Sprecherlabels
      (`Speaker A`, `Speaker B`, …), zugeordnet über Zeitüberlappung.
- [ ] Der `Local`-Strom trägt strukturell **„Ich"** — mit den zwei dokumentierten
      Grenzen unten (geteiltes Raummikrofon; fehlende Echokompensation).
- [ ] **DER ≤ 15 %** auf einem Referenzsample mit mindestens drei Sprechern, gemessen
      auf einer **Fern-/Mischmikrofon**-Spur (SDM/MDM), nicht auf Einzel-Headsets.
- [ ] **Kein Embedding überlebt den Prozess:** nach dem Übergang nach `Ended` ist in
      keinem von der Anwendung gehaltenen Puffer ein Embedding-Wert ungleich Null
      auffindbar, und weder im Dateisystem noch in einer Datenbank existiert eines.
      Die vier bekannten Grenzen dieser Zusage stehen unten und im Risikoregister.
- [ ] `Clustering + Zuordnung` sind **≤ 20 s** nach `stop()` fertig — ein Teilbudget
      des 60-s-Kriteriums, das dem ASR-Flush aus Phase 2 und dem Schreiben in Phase 4
      Raum lässt.
- [ ] Der Ringpuffer-**Fan-out** aus Phase 1 trägt erstmals zwei **reale** Konsumenten:
      `asr` und `diarize` lesen denselben Puffer über getrennte Cursor, ohne einander
      zu kennen, und der `diarize`-Cursor meldet über einen ganzen Lauf **keinen**
      Verlust.
- [ ] `diarize` hat keinen Schreib- und keinen Netzpfad, und **kein** Embedding-Typ
      implementiert `Serialize` oder ein wertausgebendes `Debug`.
- [ ] Alle drei Modelle liegen mit Lizenz und SHA-256 in der
      `provisioning`-Allowlist, und die Lizenz wird **maschinell** gegen eine erlaubte
      Menge geprüft.
- [ ] Der Build lädt **kein** unverifiziertes Binärarchiv — weder lokal noch in CI.
- [ ] Kein Foundation-Dokument widerspricht mehr dem Code (Liste in In scope).

## Scope

### In scope

- Crate `diarize` (neu): `VoiceActivityDetector`, overlap-aware Segmentierung,
  inkrementelle `SpeakerEmbeddingExtractor`-Nutzung, **eigenes agglomeratives
  Clustering in sicherem Rust**, und die Cluster→Label-Abbildung. Kein Schreib-,
  kein Netzpfad.
- Crate `core`: `SpeakerLabel`, `SpeakerId`, `SpeakerAssignment` (inklusive der
  Mehrdeutigkeits-Angabe), und die Erweiterung von `TranscriptSegment` um das Label.
  Embedding-Typen liegen **nicht** in `core`.
- Crate `session`: `diarize` als **zweiter** Leser am Ringpuffer-Cursor; Halten der
  Embeddings in `Zeroizing`-Puffern; explizites Nullen beim Übergang nach `Ended`;
  Anstoßen des Clusterings **und** die Label-Zuordnung auf die `TranscriptSegment`s
  — die Zusammenführung liegt hier, nicht in `diarize`.
- Crate `provisioning`: die drei Modelle in die Allowlist, je mit Version, URL,
  SHA-256 **und Lizenz**, plus ein Test, der die Lizenz gegen eine erlaubte Menge
  prüft.
- Crate `cli`: Sprecherlabels in der Ausgabe.
- `xtask`: die **opt-in** DER-Messung (`cargo xtask der`) samt **RTTM-Parser** und
  **DER-Scorer**; die Erweiterung des Quell- und Manifest-Audits auf `diarize`; und
  das SHA-256-Gate für das native Archiv in `bootstrap`.
- **Foundation-Doc-Korrekturen** — die verbindliche Aufzählung, als **ein** Schritt:
  1. `docs/architecture.md`, Boundaries: die Kante lautet nach dieser Phase
     `session` → `core` + `audio` + `asr` + `diarize`.
  2. `docs/architecture.md`, Flow 2: dort steht „**Jede Quelle** … → Fan-out an zwei
     Konsumenten". Nach dieser Phase hängt `diarize` nur am `Remote`-Strom; die Zeile
     wird auf „der `Remote`-Strom wird an zwei Konsumenten verteilt, der `Local`-Strom
     nur an die ASR" korrigiert.
  3. `docs/architecture.md`, Flow 3 („der lokale Strom bekommt strukturell «Ich»"):
     daneben gehören **beide** Grenzen — ein geteiltes Raummikrofon liefert genau ein
     Label, **und** bei fehlender Echokompensation (Phase 1 warnt und läuft weiter)
     kann Ton der Gegenseite im `Local`-Strom landen und dort fälschlich „Ich"
     tragen. Ohne den zweiten Satz liest sich die Zeile als Zusicherung.
  4. `docs/architecture.md`, Flow 4 („Umbenennen ändert ausschließlich Text im
     geschriebenen Protokoll"): das Umbenennen ist damit eine `protocol`-Sache, nicht
     ein veränderbares Feld an einer beendeten Sitzung. `SpeakerLabel` ist
     **unveränderlich**; die Zeile wird entsprechend geschärft.
  5. `docs/constitution.md` **und** `CLAUDE.md`, Regel 1: die Benchmark-Ausnahme aus
     Phase 2 nennt namentlich `cargo xtask wer` und den Zweck „um die WER-Kriterien zu
     messen". `cargo xtask der` fällt nicht darunter. Die Formulierung wird auf
     „Entwickler-Werkzeuge des Repositories zur Messung der Qualitätskriterien"
     verallgemeinert. **Cross-Milestone-Kante** auf das Phase-2-Issue, das die
     Ausnahme schreibt — sie kann nicht zweimal unabhängig entstehen.
  6. `docs/workflow.md`, Commands: die Zeile erwartet, dass die Verify-Dauer „mit
     `whisper-rs` und **`sherpa-onnx`** deutlich steigt … nach Phase 2 neu messen".
     `sherpa-onnx` kommt erst hier; die Messung wird nachgezogen.

### Out of scope

- Protokoll-Ausgabe, Protokollkopf, Sprecherzahl im Kopf — Phase 4.
- Umbenennen der Labels in der Oberfläche — Phase 5. Weil `SpeakerLabel`
  unveränderlich ist (Korrektur 4), ist das eine reine `protocol`-Angelegenheit und
  braucht hier keine Vorbereitung.
- **Jede** Wiedererkennung über Sitzungen hinweg — `docs/vision.md`, Non-goals.
- Diarisierung des `Local`-Stroms.
- Streaming-Sprecherlabels während des Gesprächs.

## Constraints

- `#![forbid(unsafe_code)]` in `diarize`. Das schließt den Weg über
  `sherpa-onnx-sys` auf die C-API ausdrücklich aus — die Constitution erlaubt die
  `unsafe`-Ausnahme nur den Plattform-Backends.
- `diarize` hat keinen Schreibpfad und keinen HTTP-Client; Modelldateien lesen ist
  erlaubt. Nur `provisioning` greift aufs Netz.
- Abhängigkeitsrichtung: `diarize` → `core` + `audio`; `session` → `core` + `audio` +
  `asr` + `diarize`. **`asr` und `diarize` kennen einander nicht.**
- Embeddings in `Zeroizing`, kein `Serialize`, kein wertausgebendes `Debug`.
- **Abhängigkeits- und Lizenz-Inventar** (Maßstab: Phase 2s „vollständiges Inventar"):

  | Gegenstand | Zweck | Lizenz | Status |
  | --- | --- | --- | --- |
  | `sherpa-onnx` **1.13.4**, exakt gepinnt | VAD, Segmentierung, Embedding | Apache-2.0 | gedeckt |
  | `sherpa-onnx-sys` 1.13.4 | FFI | Apache-2.0 | gedeckt |
  | dessen **Build-Deps** `ureq`, `tar`, `bzip2` | Archiv-Download beim Bauen | MIT/Apache-2.0 | **kollidiert mit dem Audit** — siehe unten |
  | gevendortes `onnxruntime` (statisch, 13 Libs) | Inferenz | MIT | `cargo deny` sieht es nicht; NOTICE-relevant |
  | pyannote-segmentation-3.0 (ONNX) | overlap-aware Segmentierung | **MIT** (LICENSE-Datei des Mirrors, Copyright 2022 CNRS — die HF-Metadaten deklarieren keine) | Allowlist |
  | Silero VAD (ONNX) | Sprachaktivität | **MIT** | Allowlist |
  | Sprecher-Embedding-Modell | Embeddings | **offen** — siehe unten | Allowlist |
  | `zeroize` | Nullen der Puffer | MIT/Apache-2.0 | gedeckt |
  | DER-Scorer (Ungarische Methode) | DER-Rechnung, nur `xtask` | zu prüfen | vor Nutzung prüfen |

- **Der Phase-1-Audit geht an `diarize` rot, und zwar berechtigt.**
  `xtask/tests/audit/mod.rs` blockt per Präfix (`serde`, `reqwest`, `ureq`) über den
  **vollständigen transitiven** Graphen aus `cargo tree -e normal,build`, und
  `sherpa-onnx-sys` führt `ureq` als Build-Dependency. Die Ausnahme wird **benannt
  und begründet** eingetragen (wie Phase 2 es für `xtask` getan hat), nicht implizit
  umgangen: erlaubt ist `ureq` ausschließlich als Build-Dependency von
  `sherpa-onnx-sys`, nie als Laufzeit-Abhängigkeit eines Crates.
- **Der Audit bekommt zwei neue Symbole:** `sherpa_onnx::write` und `Wave::write`.
  Am 2026-07-30 geprüft: `write` ist eine **öffentliche freie Funktion** der Crate und
  schreibt über die C-Bibliothek eine WAV-Datei. Die Blockliste der Constitution
  (`fs::write`, `File::create`, `OpenOptions::write`, `reqwest`, `ureq`) trifft davon
  nichts — ein einzeiliges `sherpa_onnx::write(...)` in `diarize` würde kompilieren,
  den Audit bestehen und Roh-PCM auf die Platte schreiben. Das ist der direkteste
  denkbare Bruch von Regel 1 in `CLAUDE.md`.
- **Kein unverifizierter Binär-Download beim Bauen.** `SHERPA_ONNX_ARCHIVE_DIR` zeigt
  auf ein lokal vorliegendes Archiv; `cargo xtask bootstrap` beschafft es einmal und
  prüft SHA-256 gegen eine gepinnte Summe. Ist die Variable nicht gesetzt, ist das ein
  **Build-Fehler**, kein stiller Download. CI setzt sie und cacht das Archiv.
- Die `windows-latest`-Runner haben kein Audiogerät und keine GPU. **CI lädt kein
  Modell**; alle Tests laufen gegen Attrappen und synthetische Embeddings.

## Prior art

- [Speaker diarization without a Python runtime (Phase 3)](../prior-art.md#speaker-diarization-without-a-python-runtime-phase-3)
  — der Split Segmentierung → Embedding → Clustering, das ADOPT „inkrementell während
  der Sitzung, Clustering **einmal** am Ende", statisches Linken, das AVOID „die
  Diarisierung als Streaming behandeln", und das CHECK zu den Modell-Lizenzen.
- [diart / Streaming Sortformer](../prior-art.md#diart--streaming-sortformer)
  — „overlap-aware Systeme senken die DER um 3–7 Punkte" ist der Grund, warum das
  Segmentierungsmodell overlap-aware sein **muss**.
- [Speaker identity and naming (Phase 7)](../prior-art.md#speaker-identity-and-naming-phase-7)
  — das AVOID „Embeddings über Sitzungen hinweg persistieren" (Art. 9 DSGVO) ist die
  Grenze, die diese Phase einhält.

## Human prerequisites

- [ ] Entscheidung zum Sprecher-Embedding-Modell am Gate (siehe unten).
- [ ] Ein Referenzsample mit **zeitgestempelten Sprecher-Annotationen** (RTTM-förmig)
      und mindestens drei Sprechern, auf einer **SDM/MDM**-Spur. Das ist **nicht**
      dasselbe Artefakt wie Phase 2s WER-Auszug: dort genügt ein Referenz*transkript*,
      und die naheliegende WER-Wahl ist die saubere Einzel-Headset-Spur (IHM), auf der
      Diarisierung trivial und die Zahl wertlos wäre.
- [ ] Für den QA-Smoke-Test: ein Call mit **mindestens drei** Personen auf der
      Gegenseite.
- [ ] Keine Secrets, keine Accounts; die Segmentierung kommt über den **un-gated**
      Mirror, damit kein Hugging-Face-Login nötig ist.

## Prior decisions

### Bindung, Clustering und der Build

| Decision | Rationale | Date |
|---|---|---|
| Bindung ist **`sherpa-onnx` 1.13.4** (Apache-2.0, exakt gepinnt), **nicht** `sherpa-rs` | Am 2026-07-30 geprüft: `sherpa-rs` ist seit 2026-06-06 archiviert, sein Maintainer verweist auf die offiziellen Bindungen. Exakte Pinnung statt `1.13.x`, weil die Versionsnummer der C-Bibliothek folgt und nicht der API-Stabilität (27 Versionen, Sprung von 0.1.13 auf 1.12.30) | 2026-07-30 |
| Verwendet werden **`VoiceActivityDetector`** und **`SpeakerEmbeddingExtractor`** über `OnlineStream`. **Das Clustering schreiben wir selbst**, in sicherem Rust in `diarize`: Kosinus-Distanz plus agglomeratives Clustering mit Schwellenwert | **Korrektur einer falschen Annahme der ersten Fassung.** Gegen die veröffentlichte 1.13.4 geprüft: `FastClusteringConfig` hat genau zwei öffentliche Felder (`num_clusters`, `threshold`) und **keine** Methoden; es existiert **keine** exportierte Funktion, die Embeddings entgegennimmt und Labels liefert. Einziger Einstieg ist `OfflineSpeakerDiarization::process(&[f32])` — der nimmt **rohes Audio**, nicht Embeddings, und macht alles intern. Der Batch-Weg ist mit der Null-Persistenz-Zusage unvereinbar (wir haben das vollständige Audio nie, der Ringpuffer überschreibt nach 30 s), und `sherpa-onnx-sys` direkt zu rufen verbietet `#![forbid(unsafe_code)]`. Eigenes Clustering ist damit nicht die bequemste, sondern die **einzige** verbleibende Option — und es hat drei echte Vorteile: der Schwellenwert wird wirklich unser Parameter, es entfällt eine FFI-Kopie jedes Embeddings, und der Code ist ohne Modell testbar | 2026-07-30 |
| Segmentierungsmodell **pyannote-segmentation-3.0** über den un-gated ONNX-Mirror (MIT laut LICENSE-Datei des Mirrors) | Am 2026-07-30 geprüft: das Original ist gated („You need to agree to share your contact information"), der Mirror nicht und trägt MIT weiter. Ohne ihn bräuchte jeder Nutzer einen Hugging-Face-Account — unvereinbar mit „≤ 15 Minuten bis zum ersten Protokoll". Der Allowlist-Eintrag verweist auf die **LICENSE-Datei**, nicht auf die Model-Card, die keine Lizenz deklariert | 2026-07-30 |
| VAD: **Silero VAD** (MIT), nicht TEN VAD | Am 2026-07-30 geprüft: TEN VADs Lizenz ist eine modifizierte Apache-2.0-Fassung **mit Wettbewerbsklausel** („may not Deploy … in a way that competes with Agora's offerings") und Copyleft-artiger Bindung von Derivaten. Ein `Apache-2.0`-Eintrag in der Allowlist würde sie stillschweigend durchwinken — das Gate würde nicht bloß aufgeweicht, es würde blind. `src/vad.rs` exportiert `TenVadModelConfig` neben `SileroVadModelConfig`, die Verwechslung ist also einen Tippfehler entfernt | 2026-07-30 |
| Der Build nutzt `SHERPA_ONNX_ARCHIVE_DIR` plus ein eigenes SHA-256-Gate in `cargo xtask bootstrap`; fehlt die Variable, bricht der Build ab | **Korrektur der ersten Fassung**, die beide Zweige falsch beschrieb: eine Prüfsummen-Konfiguration existiert in der Crate **nicht** (ein Scan von `build.rs` findet nur `HashSet`), und `SHERPA_ONNX_LIB_DIR` baut nichts aus Quellen, sondern zeigt auf **fertige** Bibliotheken. `SHERPA_ONNX_ARCHIVE_DIR` ist der real existierende Hebel. Ohne diese Regel lädt jeder CI-Lauf ein ungeprüftes Archiv — genau das, was die Entscheidung verbietet | 2026-07-30 |

### Nahtstellen und Threading

| Decision | Rationale | Date |
|---|---|---|
| Öffentliche API von `diarize`: `Diarizer::push(&mut self, pcm_16k_mono: &[f32], t0: SessionOffset) -> Result<(), DiarizeError>` während der Sitzung, und `Diarizer::finish(self) -> Result<Vec<SpeakerSpan>, DiarizeError>` am Ende. `SpeakerSpan { speaker: SpeakerId, t0, t1 }` | `session` **schiebt** Frames hinein und behält damit die Kontrolle über Reihenfolge und Zeitachse; `diarize` zieht nirgends. `finish` verbraucht `self`, damit die Embeddings nach dem Clustering nicht weiterleben können — das Typsystem erzwingt das Ende ihrer Lebensdauer | 2026-07-30 |
| Die **Label-Zuordnung liegt in `session`**, nicht in `diarize` | Die erste Fassung widersprach sich hier dreifach. `diarize` liefert `SpeakerSpan`s auf der Sitzungs-Zeitachse; `session` legt sie über die `TranscriptSegment`s. Läge die Zuordnung in `diarize`, müsste `diarize` `TranscriptSegment` kennen — genau die Kante, die „`asr` und `diarize` kennen einander nicht" verhindert | 2026-07-30 |
| `diarize` läuft auf einem **eigenen** Thread, CPU-only, mit auf 2 begrenzter onnxruntime-Threadzahl. Der Ringpuffer wird über einen `Mutex` geteilt; Leser halten ihn nur für die Dauer eines `read` | Die Cursor-API aus Phase 1 (`add_reader(&mut self)`, `read(&mut self, …)`) verlangt exklusiven Zugriff, und in `crates/audio` gibt es heute **kein** `Arc`/`Mutex` — der erste echte Fan-out ist damit auch die erste Synchronisationsentscheidung. CPU-only und begrenzte Threads, damit Phase 2s Kapazitätsungleichung auf der GPU unberührt bleibt | 2026-07-30 |
| Phase 2s Ungleichung bekommt einen **dritten Term**: `I_Remote + I_Local ≤ Hop` gilt weiter für die GPU; `diarize` läuft daneben auf CPU-Threads und darf den ASR-Rückstand nicht wachsen lassen — maschinell geprüft über `loss_count` des `diarize`-Cursors, das über einen ganzen Lauf **0** bleiben muss | Ein Rückstand von `diarize` ist teurer als einer von `asr`: der Ringpuffer überschreibt nach 30 s, verlorene Sprache heißt verlorene Embeddings, und ein ganzer Sprecher kann verschwinden. Eine Beobachtung („der Rückstand wächst nicht") ist dafür zu schwach; es braucht eine Schranke | 2026-07-30 |

### Segmentierung, Embedding, Clustering

| Decision | Rationale | Date |
|---|---|---|
| Ein Embedding entsteht nur aus **nicht-überlappten** Sprachanteilen eines Sprecherbereichs, und nur wenn davon **≥ 1,0 s** zusammenkommen | Overlap-aware Segmentierung erzeugt überlappende Bereiche **absichtlich**; ein Embedding aus überlappter Sprache mischt zwei Stimmen und verschiebt den Cluster-Schwerpunkt. Genau der Vorteil des Modells würde sich sonst in einen Nachteil verkehren. Die 1,0-s-Grenze, weil kürzere Ausschnitte unzuverlässige Embeddings liefern | 2026-07-30 |
| Cluster-Schwellenwert: Startwert **0,55** Kosinus-Distanz, als Konstante in `diarize`, kalibriert über `cargo xtask der` | Ein „wird kalibriert"-Versprechen ohne Startwert wäre eine Lücke: die DER-Messung ist opt-in, und ohne sie muss trotzdem etwas gelten. Der Startwert ist ein Ausgangspunkt, kein Ergebnis, und wird mit jeder DER-Zahl zusammen dokumentiert | 2026-07-30 |
| Entartete Fälle sind Teil des Vertrags: **ein** Cluster (das 1:1-Gespräch — der häufigste Zuschnitt überhaupt) ergibt genau `Speaker A`; **null** Embeddings (die Gegenseite schwieg) ergeben eine leere Zuordnung und keinen Fehler | Ein Clustering-Test mit drei sauber getrennten Sprechern beweist den einfachen Fall. Die entarteten Fälle sind die häufigen, und ohne Festlegung erfindet sie der Implementierer | 2026-07-30 |
| Bei mehreren überlappenden Sprecherbereichen bekommt ein `TranscriptSegment` das Label mit der größten Zeitüberlappung; die Mehrdeutigkeit wird als `SpeakerAssignment { primary: SpeakerId, overlap_ratio: f32, runner_up: Option<SpeakerId> }` geführt | „Mehrdeutigkeit wird vermerkt" ohne Typ wäre für Phase 4 unbrauchbar. `overlap_ratio` erlaubt es dort, Unsicherheit sichtbar zu machen, statt sie zu glätten | 2026-07-30 |
| Fehlt eines der drei Modelle und schlägt der Download fehl, startet die Sitzung **nicht** — der Fehler erscheint vor dem Consent-Schritt | Konsistent mit Phase 2 (fehlendes ASR-Modell) und unterschieden vom fehlenden Mikrofon in Phase 1: eine Sitzung ohne Sprecherzuordnung wäre kein degradiertes, sondern ein anderes Produkt, und niemand soll dafür eine Attestation bestätigen | 2026-07-30 |
| `SpeakerLabel` ist **unveränderlich** | `docs/architecture.md`, Flow 4: Umbenennen ändert ausschließlich Text im geschriebenen Protokoll. Ein veränderbares Feld an einer nach Phase 1 **terminalen** `Ended`-Sitzung wäre für Phase 5 ohnehin nicht erreichbar | 2026-07-30 |

### Das Nullen — vier Grenzen, nicht eine

| Decision | Rationale | Date |
|---|---|---|
| Der Embedding-Speicher wird **einmal** mit fester Kapazität allokiert und in Blöcken erweitert, wobei der alte Block vor der Freigabe explizit genullt wird | `Zeroizing` nullt beim `Drop` die **aktuelle** Allokation. Wächst ein `Vec`, wird der alte Block kopiert und **ungenullt** freigegeben. Phase 1 hat genau diesen Fund schon gemacht und behoben (feste Erst-Kapazität, `RingBuffer::storage` als einmal voll allokiertes `Zeroizing<Vec<f32>>`); die erste Fassung dieser Spec hat das Muster wieder eingeführt, das Phase 1s Review entfernt hatte | 2026-07-30 |
| Das Ergebnis von `SpeakerEmbeddingExtractor::compute()` wird **unmittelbar** in `Zeroizing` überführt, die Zwischenallokation genullt, und der C-seitige Extraktor nach `finish` explizit freigegeben | `compute()` liefert einen **nackten `Vec<f32>`** — jedes Embedding existiert also zuerst in einer nicht-zeroisierten Allokation. Das ist keine „Arena der Inferenz-Laufzeit", sondern die Nahtstelle, die wir selbst aufrufen, und sie ist erreichbar | 2026-07-30 |
| Genullt wird beim Übergang nach **`Ended`**, nicht bei `stop()` | Zwischen `request_stop` und `end` liegen nach Phase 1s Zustandsmaschine zwei Kanten und laut Budget bis zu 20 s, in denen die Embeddings für das Clustering notwendigerweise **leben**. Die erste Fassung schrieb „nach `stop()`" und war damit schlicht falsch | 2026-07-30 |
| Der Audit prüft auch auf **`Debug`** an Embedding-Typen, nicht nur auf `Serialize` | `Zeroizing<T>` implementiert `Debug`, wenn `T` es tut — ein `#[derive(Debug)]` gäbe die Werte aus, und die Constraints fordern das Gegenteil, ohne dass ein Gate es bisher sah | 2026-07-30 |
| **Keine** Änderung an `docs/constitution.md` für die Null-Grenzen | Die erste Fassung wollte die Zeile abschwächen. Falsch, aus drei Gründen: die Constitution-Zeile ist eine **Anweisung** („Embeddings liegen in `Zeroizing`-Puffern und werden … genullt"), keine Abwesenheitsbehauptung, überclaimt also nichts; das Vision-Kriterium ist bereits auf „weder im Dateisystem noch in einer Datenbank" begrenzt; und Phase 1 stand vor demselben Problem (`rubato`s FFT-Zustand hält PCM, von außen nicht zeroisierbar) und hat es als **Risiko plus Issue #34** geführt, ohne die Constitution anzufassen. Dieselbe Antwort hier. Die vollständige Grenze — Realloc, die nackte `compute()`-Allokation, Swap/Pagefile und Crash-Dumps, die ONNX-Arenen — steht im Risikoregister und gehört in Phase 6 in die README, nicht in eine abgeschwächte normative Regel | 2026-07-30 |

### Gates

| Decision | Rationale | Date |
|---|---|---|
| `Clustering + Zuordnung` bekommen ein **Teilbudget von 20 s** der 60 s aus `docs/vision.md` | Die erste Fassung beanspruchte das ganze Budget und ließ Phase 4 nichts — obwohl Phase 2 bereits einen Teil reserviert hat („das Vision-Budget muss diesen Lauf enthalten", über den ASR-Flush). Beide laufen im selben Zustandsübergang. Reihenfolge: das Clustering darf **parallel** zum ASR-Flush laufen, die Zuordnung nicht — sie braucht beide Ergebnisse. Am QA-Gate werden die drei Anteile **getrennt** gemessen | 2026-07-30 |
| Die DER-Messung ist **opt-in** (`cargo xtask der`), mit ausgewiesener Collar- und Overlap-Konvention | Wie bei der WER-Messung: Verify bleibt schnell und netzfrei. Ohne festgeschriebene Konvention ist eine DER-Zahl nicht vergleichbar, und die Overlap-Behandlung verschiebt sie ausgerechnet dort, wo unser Modell seinen Vorteil hat | 2026-07-30 |
| Der DER-Scorer (Ungarische Methode für die Cluster↔Referenz-Zuordnung, plus Missed/False-Alarm/Confusion) und der **RTTM-Parser** sind eingeplante Arbeit dieser Phase | DER ist keine Editierdistanz wie WER. Das stillschweigend anzunehmen wäre eine versteckte Aufgabe — Phase 2 hat denselben Punkt für den Transkript-Parser ausdrücklich eingeplant | 2026-07-30 |
| Alle Maschinen-Tests laufen gegen **synthetische Embeddings** und Attrappen | CI hat weder GPU noch Audiogerät; ein Test, der Modelle lädt, würde Verify netzabhängig machen — die Falle, die in Phase 2 zweimal auffiel. Clustering, Zuordnungsregel und Nullen sind ohne echte Sprache prüfbar | 2026-07-30 |
| Kein `/loopkit:design`-Zyklus | Keine UI-Fläche; die Ausgabe bleibt die CLI. `docs/design.md` führt den Sprecher-Chip als Teil der `Transcript-Line`; ein Umbenennen kommt dort als Komponente nicht vor und wird in Phase 5 entworfen | 2026-07-30 |
| OPEN — **Sprecher-Embedding-Modell.** sherpa-onnx weist die Lizenzprüfung ausdrücklich dem Nutzer zu („Each model has its own license"). Die starken Kandidaten sind VoxCeleb-trainiert: **CC BY 4.0**, kommerzielle Nutzung ausdrücklich **erlaubt**, mit Namensnennungspflicht — die Prosa der Datensatz-Startseite sagt abweichend „for research purposes" und widerspricht damit der Lizenz, die sie selbst nennt; das Urheberrecht an den zugrunde liegenden Videos bleibt bei den Rechteinhabern. Zu entscheiden ist zwischen Namensnennung plus Restunsicherheit und Alternativen (CNCeleb, VoxBlink2) | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 3 — Sprechertrennung (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist
- `Depends on milestone: #1, #2`. Das `docs/workflow.md`-Token ist als
  `Depends on milestone: #<n>` definiert; die mehrwertige Form wird dort mitgezogen
  (Teil von Korrektur 6), statt sie stillschweigend zu dehnen.
- **Kontext:** `crates/` enthält heute nur `core` und `audio`. `session`, `asr`,
  `provisioning`, `cli` und `audio-win` existieren **nicht**, und `xtask` kennt nur
  `bootstrap`/`verify`/`build` — also auch kein `wer`, auf dem `der` aufbauen könnte.
  Jede hier zitierte Geschwister-API ist ein Versprechen; die Cross-Milestone-Kanten
  sind bindend.

## Verification

Maschinell, in Verify und CI — **ohne Modell, ohne Netz, ohne Audiogerät**:

- [ ] `cargo xtask verify` und `cargo xtask build` grün; `cargo deny check` grün.
- [ ] `cargo xtask bootstrap` verifiziert das native Archiv gegen die gepinnte
      SHA-256; ohne `SHERPA_ONNX_ARCHIVE_DIR` bricht der Build ab, statt zu laden.
- [ ] Der Audit bewacht `diarize`: kein Schreib-, Netz- oder `serde`-Pfad, **kein
      `Serialize` und kein `Debug` an einem Embedding-Typ**, und die neuen Symbole
      `sherpa_onnx::write` / `Wave::write` sind blockiert — je einmal durch eine
      absichtliche Verletzung belegt.
- [ ] Die `ureq`-Ausnahme für `sherpa-onnx-sys` ist **benannt** und greift nur für
      Build-Dependencies; eine Laufzeit-`ureq`-Kante in einem bewachten Crate lässt
      den Audit weiterhin fehlschlagen.
- [ ] Der `provisioning`-Test prüft die Modell-Lizenz gegen eine erlaubte Menge.
- [ ] **Null-Test:** nach `Ended` ist in keinem Embedding-Puffer ein Wert ungleich
      Null; zusätzlich belegt ein Test, dass ein Wachstum der Sammlung den alten Block
      **vor** der Freigabe nullt.
- [ ] **FS-Audit (Phase 1, erweitert):** eine Sitzung mit `diarize`-Konsument
      hinterlässt 0 neue Dateien.
- [ ] **Clustering-Tests** gegen synthetische Embeddings: drei getrennte Sprecher →
      drei Cluster ohne vorgegebene Zahl; **ein** Sprecher → genau `Speaker A`;
      **null** Embeddings → leere Zuordnung, kein Fehler.
- [ ] **Overlap-Test:** aus einem Bereich mit überlappter Sprache entsteht kein
      Embedding; aus < 1,0 s nicht-überlappter Sprache ebenfalls nicht.
- [ ] **Zuordnungs-Test:** das Label mit der größten Überlappung gewinnt, und
      `overlap_ratio`/`runner_up` sind gesetzt.
- [ ] **Fan-out-Verdrahtung:** `asr` und `diarize` laufen als **zwei reale
      Konsumenten** am selben `Mutex`-geteilten Ringpuffer; über einen vollen Lauf
      bleibt `loss_count` des `diarize`-Cursors bei **0**. (Der reine
      Cursor-Mechanismus ist bereits durch Phase 1s `two_cursor_…`-Test belegt; hier
      geht es um die Verdrahtung.)
- [ ] **`Local`-Test:** der lokale Strom erhält „Ich" ohne Embedding-Vergleich.

Manuell am Milestone-QA-Gate:

- [ ] **Sprecherzuordnung:** Call mit ≥ 3 Personen auf der Gegenseite → unterscheidbare
      Labels, eigener Beitrag durchgängig „Ich".
- [ ] **AEC-Degradierung:** ist `is_aec_supported() == false` (Phase 1 warnt und läuft
      weiter), wird geprüft und protokolliert, wie sich die „Ich"-Zuordnung bei
      Lautsprecher-Nutzung verhält — das ist der Fall, in dem sie **falsch** sein kann.
- [ ] **DER:** `cargo xtask der` auf der SDM/MDM-Spur ergibt ≤ 15 %, mit
      dokumentierter Collar-, Overlap- und Schwellenwert-Angabe.
- [ ] **20-s-Teilbudget:** `stop()` → feststehende Labels ≤ 20 s; ASR-Flush,
      Clustering und Zuordnung werden **getrennt** gemessen und summiert gegen die 60 s
      der Vision geprüft.
- [ ] **Nicht-Störung:** weder Call noch Live-Transkription brechen ein; der
      ASR-Rückstand wächst nicht.
- [ ] **Prozessbezogene Beobachtung:** kein Schreibzugriff außerhalb der Konsole —
      die einzige Prüfung, die Schreibzugriffe der **nativen** Bibliothek überhaupt
      erfassen kann.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Das eigene Clustering ist schlechter als eine erprobte Implementierung und reißt die DER | Agglomeratives Clustering über Kosinus-Distanz ist der Standardansatz und überschaubar; die DER-Messung ist das Kalibrier- und Beweismittel. Reißt sie, ist der nächste Hebel das Segmentierungs-Postprocessing, nicht ein FFI-Umweg, den die Constitution verbietet |
| **Die Null-Zusage hat vier bekannte Grenzen** | (1) Realloc — behoben über feste Kapazität plus explizites Nullen alter Blöcke. (2) Die nackte `compute()`-Allokation — behoben über sofortige Überführung und Nullen. (3) Swap/Pagefile und Crash-Dumps — vom Prozess **nicht** kontrollierbar; gehört als Aussage in die README (Phase 6). (4) Die internen Arenen der ONNX-Laufzeit — nicht erreichbar. (1) und (2) sind Arbeit dieser Phase, (3) und (4) sind offengelegte Grenzen. Sie stehen hier und in der README, **nicht** als Abschwächung in der Constitution — dem `rubato`-Präzedenzfall aus Phase 1 folgend |
| Der Phase-1-Audit blockiert `diarize` dauerhaft wegen `ureq` im Build-Graphen | Als benannte, begründete Ausnahme gelöst und maschinell auf Build-Dependencies begrenzt — nicht durch Aufweichen der Blockliste |
| `sherpa_onnx::write` wird versehentlich benutzt | In der Blockliste, mit absichtlicher Verletzung belegt. Es ist der einzige Pfad, auf dem dieses Crate Roh-PCM schreiben könnte |
| Die Embedding-Extraktion konkurriert mit der ASR-Inferenz | `diarize` läuft CPU-only mit begrenzter Threadzahl, die GPU bleibt der ASR. Der `loss_count`-Test ist die Schranke, nicht eine Beobachtung |
| Der AMI-Auszug aus Phase 2 ist für DER unbrauchbar, weil er die IHM-Spur nutzt | Als Human prerequisite ausgewiesen: die DER-Messung braucht SDM/MDM **und** eine RTTM-Referenz. „Eine Quelle, zwei Kriterien" gilt für die Audioquelle, nicht für das Artefakt |
| Die Geschwister-Milestones sind noch nicht implementiert; die hier angenommenen APIs können sich verschieben | `Depends on milestone: #1, #2` plus Cross-Milestone-Kanten je Issue. Verschiebt sich eine API, eskaliert das Issue mit `needs:planning`, statt eine Krücke zu bauen |

## Decision log

- 2026-07-30: **Die tragende Entscheidung der ersten Fassung war falsch.** Sie stützte
  sich auf eine Zusammenfassung der API-Dokumentation und las die **Existenz eines
  Namens** (`FastClusteringConfig`) als Existenz einer **nutzbaren Nahtstelle**. Gegen
  die veröffentlichte 1.13.4 geprüft: zwei öffentliche Felder, keine Methoden, einziger
  Konsument ist `OfflineSpeakerDiarizationConfig`; es gibt keine exportierte Funktion
  von Embeddings zu Labels, und `OfflineSpeakerDiarization::process` nimmt rohes Audio.
  Der Entwurf der Phase war auf der gewählten Bindung nicht implementierbar. Konsequenz:
  das Clustering schreiben wir selbst in sicherem Rust. Es ist derselbe Fehlermodus, der
  in Phase 2 beim Versionsstand auftrat — die Lehre ist, das **veröffentlichte
  Artefakt** zu lesen, nicht eine Beschreibung davon.
- 2026-07-30: **Lizenz-Korrektur.** Die erste Fassung behauptete, VoxCeleb sei „auf
  Forschung beschränkt". Das ist falsch: die Lizenz ist **CC BY 4.0** und erlaubt
  kommerzielle Nutzung ausdrücklich; WeSpeaker führt seine VoxCeleb-Modelle wörtlich
  darunter. Der Eindruck entsteht allein aus der Prosa der Datensatz-Startseite, die
  „for research purposes … under a CC BY 4.0 License" schreibt und damit der Lizenz
  widerspricht, die sie selbst nennt. Eine erfundene Lizenzsperre hätte am Gate zur
  Wahl eines schwächeren Modells geführt, um einem Konflikt auszuweichen, den es nicht
  gibt.
- 2026-07-30: Zwei Wege, auf denen diese Phase die Gates der Vorphasen gebrochen hätte,
  beide vom Review gefunden: `sherpa-onnx-sys` bringt `ureq` als Build-Dependency und
  hätte den Phase-1-Audit an `diarize` rot gemacht; und `sherpa_onnx::write` ist eine
  öffentliche freie Funktion, die über die C-Bibliothek eine WAV-Datei schreibt und die
  Symbol-Blockliste der Constitution vollständig unterläuft — ein Einzeiler hätte
  kompiliert, den Audit bestanden und Roh-PCM auf die Platte geschrieben.
- 2026-07-30: Die Build-Entscheidung der ersten Fassung beschrieb **beide** Zweige
  falsch — eine Prüfsummen-Konfiguration existiert nicht, und `SHERPA_ONNX_LIB_DIR`
  baut nicht aus Quellen. Ersetzt durch `SHERPA_ONNX_ARCHIVE_DIR` plus eigenes
  SHA-256-Gate in `bootstrap`, mit Build-Fehler statt stillem Download.
- 2026-07-30: Weiter behoben: das 60-s-Budget war vollständig beansprucht, obwohl
  Phase 2 den ASR-Flush darin schon reserviert hatte (jetzt 20 s Teilbudget, drei
  Anteile getrennt gemessen); die Kapazitätsfrage hatte keinen Term und der
  Ringpuffer keine Synchronisationsentscheidung; die Label-Zuordnung war an drei
  Stellen widersprüchlich verortet (jetzt eindeutig in `session`); und die Grenze der
  „Ich"-Zusage nannte das Raummikrofon, aber nicht den Fall, in dem Phase 1 ohne
  Echokompensation weiterläuft und die Zuordnung dadurch nicht gröber, sondern
  **falsch** wird.
