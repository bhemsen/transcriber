# Spec: Phase 3 — Sprechertrennung

> Created: 2026-07-30

Diese Spec liefert die Sprecherzuordnung: VAD und overlap-aware Segmentierung auf dem
`Remote`-Strom, inkrementelle Embedding-Extraktion während der Sitzung, globales
Clustering **einmal** am Sitzungsende, und die Zuordnung der entstehenden Labels auf
die Transkript-Segmente über Zeitüberlappung. Sie trägt das zweite Kernversprechen
des Projekts: **kein Stimmprofil überlebt die Sitzung**.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] Am Sitzungsende tragen die Segmente des `Remote`-Stroms Sprecherlabels
      (`Speaker A`, `Speaker B`, …), zugeordnet über Zeitüberlappung.
- [ ] Der `Local`-Strom trägt strukturell **„Ich"** — ohne Clustering, ohne
      Embedding-Vergleich, allein aus der Strom-Identität aus Phase 1.
- [ ] **DER ≤ 15 %** auf dem definierten Referenzsample mit mindestens drei Sprechern.
- [ ] **Kein Embedding überlebt den Prozess:** nach `stop()` ist kein Embedding-Wert
      ungleich Null mehr in den Puffern der Anwendung auffindbar, und weder im
      Dateisystem noch in einer Datenbank existiert eines — maschinell belegt, mit
      der unten offengelegten Grenze bezüglich der Inferenz-Laufzeit.
- [ ] Sprecherlabels stehen **≤ 60 s nach Sitzungsende** fest — gemessen als
      `Clustering + Zuordnung`, das Budget für das Schreiben bleibt Phase 4.
- [ ] Der Ringpuffer-**Fan-out** aus Phase 1 trägt erstmals zwei Konsumenten: `asr`
      und `diarize` lesen denselben Puffer über getrennte Cursor, ohne einander zu
      kennen.
- [ ] `diarize` hat keinen Schreib- und keinen Netzpfad, und **kein**
      Embedding-Typ implementiert `Serialize` — der Audit-Test bewacht das Crate mit.
- [ ] Alle drei Modelle (Segmentierung, Embedding, VAD) liegen mit Lizenz und
      SHA-256 in der `provisioning`-Allowlist.
- [ ] Kein Foundation-Dokument widerspricht mehr dem Code (Liste in In scope).

## Scope

### In scope

- Crate `diarize` (neu): `VoiceActivityDetector`, overlap-aware Segmentierung,
  inkrementelle `SpeakerEmbeddingExtractor`-Nutzung, Clustering am Sitzungsende,
  Zuordnung der Labels über Zeitüberlappung. Kein Schreib-, kein Netzpfad.
- Crate `core`: `SpeakerLabel`, `SpeakerId`, und die Erweiterung von
  `TranscriptSegment` um das Label. Embedding-Typen liegen **nicht** in `core` —
  siehe die Entscheidungen.
- Crate `session`: `diarize` als **zweiter** Leser am Ringpuffer-Cursor; Halten der
  Embeddings in `Zeroizing`-Puffern; explizites Nullen am Sitzungsende; Anstoßen des
  Clusterings und der Label-Zuordnung beim Übergang nach `Stopping`.
- Crate `provisioning`: die drei Modelle in die Allowlist, je mit Version, URL,
  SHA-256 **und Lizenz**.
- Crate `cli`: Sprecherlabels in der Ausgabe; sitzungsgebundenes Umbenennen bleibt
  Phase 5 (Oberfläche), die Datenstruktur dafür entsteht hier.
- `xtask`: die **opt-in** DER-Messung (`cargo xtask der`), aufbauend auf dem
  Referenzsample-Bezug aus Phase 2; und die Erweiterung des Quell- und
  Manifest-Audits auf `diarize`.
- **Foundation-Doc-Korrekturen** — die verbindliche Aufzählung, als **ein** Schritt:
  1. `docs/architecture.md`, Boundaries: die Kante lautet nach dieser Phase
     `session` → `core` + `audio` + `asr` + `diarize`.
  2. `docs/architecture.md`, Flow 2: der Fan-out an **zwei** Konsumenten ist ab hier
     real und nicht mehr angekündigt; die Zeile wird auf den gebauten Zustand
     gebracht.
  3. `docs/architecture.md`, Flow 3: dort wird der lokale Strom „strukturell «Ich»"
     zugeordnet — die dokumentierte Grenze (ein geteiltes Raummikrofon liefert genau
     ein Label) gehört daneben, sonst liest sich die Zeile als Zusicherung.
  4. `docs/constitution.md`, Architecture principles: die Zeile über
     `Zeroizing`-Puffer bekommt die unten offengelegte Grenze — was **wir** nullen
     können, endet an den Arenen der Inferenz-Laufzeit.

### Out of scope

- Protokoll-Ausgabe, Protokollkopf und die Sprecherzahl im Kopf — Phase 4. Diese
  Phase erzeugt die Labels, schreibt sie nicht.
- Sitzungsgebundenes Umbenennen der Labels in der Oberfläche — Phase 5. Die
  Datenstruktur (`SpeakerLabel` mit einem veränderbaren Anzeigenamen) entsteht hier,
  die Bedienung dort.
- **Jede** Form der Wiedererkennung über Sitzungen hinweg — `docs/vision.md`,
  Non-goals. Kein persistenter Speicher, kein Vergleich gegen frühere Sitzungen,
  keine Export-Funktion für Embeddings.
- Diarisierung des `Local`-Stroms. Er ist strukturell „Ich"; siehe die dokumentierte
  Grenze.
- Streaming-Sprecherlabels während des Gesprächs. `docs/prior-art.md` hält fest, dass
  sherpa-onnx' Diarisierung offline/batch ist, und unser Entwurf akzeptiert das:
  Labels sind ein Produkt des Sitzungsendes.

## Constraints

- `#![forbid(unsafe_code)]` in `diarize`. Die Bindung kapselt das FFI.
- `diarize` hat keinen Schreibpfad und keinen HTTP-Client; das **Lesen** von
  Modelldateien über Pfade ist erlaubt. Nur `provisioning` greift aufs Netz.
- Abhängigkeitsrichtung: `diarize` → `core` + `audio`; `session` → `core` + `audio` +
  `asr` + `diarize`. **`asr` und `diarize` kennen einander nicht**
  (`docs/architecture.md`) — zusammengeführt wird erst in `session`, über
  Zeitüberlappung.
- Sprecher-Embeddings liegen in `Zeroizing`-Puffern und werden bei Sitzungsende
  explizit genullt (`docs/constitution.md`). **Kein Embedding-Typ implementiert
  `Serialize` oder ein `Debug`, das Werte ausgibt** — Persistenz ist ein
  Compile-Fehler.
- Maximal 50 Zeilen je Funktion, maximal 400 Zeilen je Modul; kein `unwrap()`/
  `expect()` außer in Tests und `main`.
- **Abhängigkeits- und Lizenz-Inventar** dieser Phase:

  | Gegenstand | Zweck | Lizenz | Status |
  | --- | --- | --- | --- |
  | `sherpa-onnx` 1.13.x (crate) | VAD, Segmentierung, Embedding, Clustering | **Apache-2.0** | in `deny.toml` gedeckt |
  | pyannote-segmentation-3.0 (ONNX) | overlap-aware Segmentierung | **MIT**, über den **un-gated** ONNX-Mirror | Allowlist-Eintrag nötig |
  | Silero VAD (ONNX) | Sprachaktivität | **MIT** | Allowlist-Eintrag nötig |
  | Sprecher-Embedding-Modell | Embeddings | **offen** — siehe die offene Entscheidung | Allowlist-Eintrag nötig |
  | `zeroize` | Nullen der Embedding-Puffer | MIT/Apache-2.0 | gedeckt |

- **Landmine, am 2026-07-30 geprüft:** die `sherpa-onnx`-Crate linkt zwar statisch,
  **lädt aber beim Bauen vorgefertigte Archive aus dem Netz**, sofern nicht
  `SHERPA_ONNX_LIB_DIR` gesetzt ist. Das ist Build-Zeit, nicht Laufzeit, und
  `cargo deny` sieht es nicht. Die Entscheidung unten regelt es ausdrücklich.
- Die GitHub-`windows-latest`-Runner haben kein Audiogerät und keine GPU. **CI lädt
  kein Modell**: alle Tests dort laufen gegen Attrappen und synthetische Embeddings.

## Prior art

- [Speaker diarization without a Python runtime (Phase 3)](../prior-art.md#speaker-diarization-without-a-python-runtime-phase-3)
  — trägt die ganze Phase: der Split Segmentierung → Embedding → Clustering, das
  ADOPT „inkrementell während der Sitzung, Clustering **einmal** am Ende", statisches
  Linken, und das AVOID „die Diarisierung als Streaming behandeln". Auch das CHECK
  zu den Modell-Lizenzen, das die offene Entscheidung unten auslöst.
- [diart / Streaming Sortformer](../prior-art.md#diart--streaming-sortformer)
  — reference-only wegen der Python-Laufzeit, aber die Feststellung „overlap-aware
  Systeme senken die DER um 3–7 Punkte gegenüber reinem Clustering" ist der Grund,
  warum das Segmentierungsmodell overlap-aware sein **muss** und nicht eine
  Komfortwahl ist.
- [Speaker identity and naming (Phase 7)](../prior-art.md#speaker-identity-and-naming-phase-7)
  — das AVOID „Embeddings über Sitzungen hinweg persistieren" (biometrische Daten
  nach Art. 9 DSGVO) ist die Grenze, die diese Phase einhält, und das ADOPT
  „manuelles, sitzungsgebundenes Labeln als **primärer** Mechanismus".

## Human prerequisites

- [ ] Entscheidung zum Sprecher-Embedding-Modell am Gate (siehe unten) — sie bestimmt
      die Lizenzlage des ausgelieferten Pakets.
- [ ] Ein Referenzsample mit **Sprecher-Annotationen** und mindestens drei Sprechern.
      Der AMI-Auszug aus Phase 2 erfüllt beides und wird wiederverwendet; nichts
      Neues zu liefern, sofern Phase 2 ihn eingerichtet hat.
- [ ] Für den QA-Smoke-Test: ein Call mit **mindestens drei** Personen auf der
      Gegenseite. Ohne das ist das Sprecherkriterium nicht am echten System prüfbar.
- [ ] Keine Secrets, keine Accounts. Alle Modellquellen sind öffentlich und
      unauthentifiziert; die Segmentierung kommt ausdrücklich über den **un-gated**
      Mirror, damit kein Hugging-Face-Login nötig ist.

## Prior decisions

### Die Bindung und der Zuschnitt

| Decision | Rationale | Date |
|---|---|---|
| Bindung ist die Crate **`sherpa-onnx` 1.13.x** (Apache-2.0), **nicht** `sherpa-rs` | Am 2026-07-30 geprüft: `sherpa-rs` ist seit Juni 2026 **archiviert** und read-only, und sein Maintainer verweist selbst auf die offiziellen Bindungen. Apache-2.0 ist in `deny.toml` gedeckt, statisches Linken ist der Default | 2026-07-30 |
| Verwendet werden die **Komponenten**: `VoiceActivityDetector`, `SpeakerEmbeddingExtractor` über `OnlineStream`, und `FastClusteringConfig` als eigener Schritt. **Nicht** `OfflineSpeakerDiarization` | Am 2026-07-30 an der API geprüft: alle drei existieren getrennt, und genau das macht unseren Entwurf möglich. `OfflineSpeakerDiarization` verlangt das vollständige Audio am Stück — wir haben es nie, weil der Ringpuffer nach 30 s überschreibt. Der Batch-Weg wäre nicht nur langsamer, er wäre mit der Null-Persistenz-Zusage **unvereinbar** | 2026-07-30 |
| Der Build darf **keine unverifizierten Binärarchive** ziehen: entweder die Crate wird so konfiguriert, dass sie Prüfsummen verifiziert, oder es wird über `SHERPA_ONNX_LIB_DIR` aus den Quellen gebaut | Die Crate lädt sonst beim Bauen vorgefertigte Archive, und `cargo deny` sieht davon nichts. Für ein Projekt, dessen Kern eine Datenschutz- und Integritätszusage ist, wäre eine ungeprüfte Binärquelle im Build der unsauberste denkbare Punkt. Kostet Bauzeit; das ist der Preis | 2026-07-30 |
| Segmentierungsmodell: **pyannote-segmentation-3.0** über den **un-gated** ONNX-Mirror (MIT) | Am 2026-07-30 geprüft: MIT, und der un-gated Mirror vermeidet die Zugangsbeschränkung des Originals — sonst bräuchte jeder Nutzer einen Hugging-Face-Account, was dem Kriterium „≤ 15 Minuten bis zum ersten Protokoll" widerspräche. Overlap-aware ist nach `docs/prior-art.md` der größte einzelne DER-Hebel | 2026-07-30 |
| VAD: **Silero VAD** (MIT), nicht TEN VAD | Am 2026-07-30 geprüft: TEN VAD ist messbar besser (schnellere Sprach-/Pausenübergänge, weniger Speicher), steht aber unter einer **modifizierten** Apache-2.0-Fassung. „Modifiziert" ist nicht „Apache-2.0" und würde das Lizenz-Gate aufweichen. Silero ist MIT und gedeckt. Erweist sich die VAD-Latenz als Problem, ist TEN VAD eine begründete Wiedervorlage — mit Lizenzprüfung, nicht nebenbei | 2026-07-30 |

### Der Datenpfad und das Nullen

| Decision | Rationale | Date |
|---|---|---|
| `diarize` liest über einen **eigenen** Ringpuffer-Cursor aus Phase 1 — der erste echte Fan-out | Phase 1 hat die Cursor-API gebaut und den Fan-out ausdrücklich auf Phase 2/3 vertagt; hier wird das Versprechen eingelöst. `asr` und `diarize` bleiben voneinander unabhängig, wie `docs/architecture.md` es verlangt | 2026-07-30 |
| Embedding-Typen liegen in **`diarize`**, nicht in `core` | `protocol` darf laut Boundaries nur `core` kennen. Läge ein Embedding-Typ in `core`, wäre er für den Schreiber erreichbar — genau die Kante, die der Abhängigkeitsgraph verhindern soll. Nach `core` geht nur das **Label**, nie der Vektor | 2026-07-30 |
| Embeddings liegen in `Zeroizing`-Puffern, werden beim Übergang nach `Ended` explizit genullt, implementieren **kein** `Serialize` und **kein** wertausgebendes `Debug` | `docs/constitution.md`. Der Audit-Test aus Phase 1 wird auf `diarize` erweitert und prüft `Serialize`/`serde` mit — eine negative Trait-Zusicherung lässt sich nicht ausdrücken, ein Quell- und Manifest-Scan schon | 2026-07-30 |
| **Offengelegte Grenze der Null-Zusage:** genullt werden **unsere** Puffer. Was die ONNX-Laufzeit intern in ihren Arenen hält, können wir weder erreichen noch nullen. Die Zusage lautet deshalb präzise: kein Embedding in einem von der Anwendung gehaltenen Puffer, keines im Dateisystem, keines in einer Datenbank | Ohne diese Grenze wäre das Kriterium „nach Sitzungsende ist kein Embedding mehr auffindbar" eine Zusage über fremden Speicher, die wir nicht halten können. `docs/constitution.md` wird entsprechend präzisiert (Korrektur 4). Ehrlich begrenzt ist mehr wert als absolut behauptet | 2026-07-30 |
| Die Embedding-Sammlung wächst mit der Gesprächsdauer und ist damit **nicht** konstant wie der Ringpuffer | Kein Widerspruch zu Phase 1, aber eine Präzisierung wert: rund 1 000 Sprachsegmente je Stunde × 192 Werte × 4 B ≈ 0,8 MB — neben 23 MB Ringpuffer ohne Gewicht. Eine feste Obergrenze wäre schlechter: sie würde Sprecher verlieren statt Speicher zu sparen | 2026-07-30 |

### Clustering und Zuordnung

| Decision | Rationale | Date |
|---|---|---|
| Geclustert wird **einmal**, beim Übergang `Stopping → Ended`, über alle Embeddings des `Remote`-Stroms | `docs/prior-art.md` ADOPT: inkrementell extrahieren, einmal clustern. Das liefert globalen Kontext — ein Sprecher, der erst nach 40 Minuten wieder spricht, landet im selben Cluster — ohne je das vollständige Audio zu puffern | 2026-07-30 |
| Die **Sprecherzahl wird nicht vorgegeben**, sondern über einen Schwellenwert bestimmt (`FastClusteringConfig` mit Cluster-Schwelle statt fixem `num_clusters`) | Niemand weiß vor dem Gespräch, wie viele Menschen auf der Gegenseite sitzen. Eine feste Zahl wäre eine Eingabe, die die Oberfläche erfragen müsste — und die das Kriterium „mindestens drei Gegenseiten-Sprecher" nur zufällig träfe. Der Schwellenwert wird über die DER-Messung kalibriert, nicht geraten | 2026-07-30 |
| Labels werden über **Zeitüberlappung** auf die `TranscriptSegment`s gelegt; bei mehreren überlappenden Sprechern gewinnt der mit der größten Überlappung, und die Mehrdeutigkeit wird am Segment vermerkt | `docs/architecture.md`, Flow 3. Overlap-aware Segmentierung erzeugt **absichtlich** überlappende Sprecherbereiche; ohne eine Regel wäre unklar, welches Label ein Segment bekommt. Der Vermerk ist die Grundlage dafür, dass Phase 4 Unsicherheit sichtbar machen kann statt sie zu glätten | 2026-07-30 |
| Beide Zeitachsen sind die **Sitzungs-Zeitachse aus Phase 1** | `asr` und `diarize` kennen einander nicht; die gemeinsame Achse ist das einzige, worüber sie zusammenfinden. Ohne sie wäre die Zuordnung nicht berechenbar — genau der Grund, warum Phase 1 die gemeinsame Sitzungs-Null festgeschrieben hat | 2026-07-30 |
| Der `Local`-Strom bekommt **strukturell** „Ich", ohne Clustering. **Dokumentierte Grenze:** ein geteiltes Raummikrofon liefert genau ein Label, auch wenn mehrere Menschen hineinsprechen | `docs/vision.md` begründet die 100-%-Zusage ausdrücklich „strukturell über getrennte Ströme" — sie gilt für die Trennung Ich/Gegenseite, nicht für die Trennung mehrerer Personen an einem Mikrofon. Das ist eine Grenze, keine Lücke, und sie gehört sichtbar in `docs/architecture.md` (Korrektur 3) statt in eine Fußnote | 2026-07-30 |

### Gates

| Decision | Rationale | Date |
|---|---|---|
| Die DER-Messung ist **opt-in** (`cargo xtask der`) und nicht Teil von Verify, aufbauend auf dem Referenzsample-Bezug aus Phase 2 (in `xtask`, nie in `provisioning`) | Dieselbe Begründung wie bei der WER-Messung: Verify ist das Gate je Iteration und muss schnell und netzfrei bleiben. Der AMI-Auszug trägt beide Messungen, weil er Sprecher-Annotationen **und** Transkripte hat — eine Quelle, zwei Kriterien | 2026-07-30 |
| DER wird mit ausgewiesener **Collar**- und Overlap-Behandlung gemessen und beides mit dem Ergebnis dokumentiert | Wie bei der WER-Normalisierung: ohne festgeschriebene Konvention ist eine DER-Zahl nicht vergleichbar, und die Behandlung überlappender Sprache verschiebt sie um mehrere Punkte — ausgerechnet dort, wo unser Segmentierungsmodell seinen Vorteil hat | 2026-07-30 |
| Alle Maschinen-Tests laufen gegen **synthetische Embeddings** und Attrappen, ohne Modell und ohne Audiogerät | CI hat weder GPU noch Audiogerät, und ein Test, der Modelle lädt, würde Verify netzabhängig machen — dieselbe Falle, die in Phase 2 zweimal aufgefallen ist. Clustering, Zuordnungsregel und Nullen sind alle ohne echte Sprache prüfbar | 2026-07-30 |
| Kein `/loopkit:design`-Zyklus | Keine UI-Fläche in dieser Phase; die Ausgabe bleibt die CLI. Der Sprecher-Chip und das Umbenennen sind in `docs/design.md` bereits als Komponenten festgelegt und werden in Phase 5 entworfen | 2026-07-30 |
| OPEN — **Sprecher-Embedding-Modell.** sherpa-onnx unterstützt 3D-Speaker-, WeSpeaker- und NeMo-Modelle und weist die Lizenzen ausdrücklich dem Nutzer zu. Die stärksten Modelle sind auf VoxCeleb trainiert, dessen Daten-Terms auf Forschung beschränkt sind — für ein Apache-2.0-Produkt ein echter Konflikt zwischen Messwert und Lizenzlage | resolved at the spec-acceptance gate | — |

## Tracking

- Milestone: Phase 3 — Sprechertrennung (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist
- `Depends on milestone: #1, #2`. Diese Phase braucht den Ringpuffer-Cursor und die
  Sitzungs-Zeitachse aus Phase 1, die `TranscriptSegment`-Typen und `provisioning`
  aus Phase 2. Die Issues tragen die entsprechenden Cross-Milestone-Kanten.

Jedes Issue verweist im Body auf diesen Spec-Pfad.

## Verification

Maschinell, in Verify und in CI — **ohne Modell, ohne Netz, ohne Audiogerät**:

- [ ] `cargo xtask verify` grün; `cargo xtask build` grün.
- [ ] `cargo deny check` grün; alle drei Modelle stehen mit Version, SHA-256 **und
      Lizenz** in der `provisioning`-Allowlist.
- [ ] Der Quell- und Manifest-Audit bewacht `diarize`: kein Schreib-, kein Netz-,
      kein `serde`-Pfad, und **kein `Serialize` auf einem Embedding-Typ**. Eine
      absichtliche Verletzung lässt ihn einmal fehlschlagen.
- [ ] **Null-Test:** nach dem Übergang nach `Ended` ist in den Embedding-Puffern kein
      Wert ungleich Null mehr auffindbar.
- [ ] **FS-Audit (aus Phase 1, erweitert):** eine vollständige Sitzung mit
      angehängtem `diarize`-Konsumenten hinterlässt 0 neue Dateien.
- [ ] **Clustering-Test** gegen synthetische Embeddings mit bekannter Gruppierung:
      drei klar getrennte Sprecher werden als drei Cluster erkannt, ohne dass die
      Sprecherzahl vorgegeben wird.
- [ ] **Zuordnungs-Test:** bei überlappenden Sprecherbereichen bekommt ein Segment
      das Label mit der größten Zeitüberlappung, und die Mehrdeutigkeit wird
      vermerkt.
- [ ] **Fan-out-Test:** `asr`- und `diarize`-Attrappen lesen denselben Ringpuffer über
      getrennte Cursor; der langsamere meldet Verlust, ohne den schnelleren zu
      beeinflussen — die Einlösung der Phase-1-Zusage.
- [ ] **`Local`-Test:** der lokale Strom erhält „Ich" ohne jeden
      Embedding-Vergleich.

Manuell am Milestone-QA-Gate (Smoke-Test nach `docs/workflow.md`):

- [ ] **Sprecherzuordnung:** Call mit mindestens drei Personen auf der Gegenseite →
      am Sitzungsende tragen die Segmente unterscheidbare Labels, und der eigene
      Beitrag ist durchgängig „Ich".
- [ ] **DER:** `cargo xtask der` auf dem AMI-Auszug ergibt ≤ 15 %, mit
      dokumentierter Collar- und Overlap-Konvention.
- [ ] **60-s-Budget:** vom `stop()` bis zu feststehenden Labels vergehen ≤ 60 s,
      gemessen und protokolliert.
- [ ] **Nicht-Störung:** die zusätzliche Embedding-Extraktion während der Sitzung
      lässt weder den Call noch die Live-Transkription einbrechen; der
      `asr`-Rückstand aus Phase 2 wächst dadurch nicht.
- [ ] **Null Bytes bleibt gültig:** nach der Sitzung findet eine prozessbezogene
      Beobachtung keinen Schreibzugriff außerhalb der Konsole.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Die DER verfehlt 15 %, weil der Cluster-Schwellenwert nicht kalibriert ist | Der Schwellenwert ist ausdrücklich ein **kalibrierter**, kein geratener Wert: die DER-Messung ist das Kalibrierwerkzeug, und ihr Ergebnis wird mit dem verwendeten Wert dokumentiert |
| Das gewählte Embedding-Modell ist lizenzrechtlich enger als das Projekt | Genau der Gegenstand der offenen Entscheidung. Die Allowlist führt die Lizenz je Modell mit, damit die Lage später belegbar bleibt statt rekonstruiert werden zu müssen |
| Die Embedding-Extraktion konkurriert mit der ASR-Inferenz um dieselbe Rechenzeit und reißt Phase 2s Kapazitätsungleichung | Das QA-Gate prüft ausdrücklich, dass der `asr`-Rückstand nicht wächst. Reicht es nicht, ist der Hebel die Extraktionsrate (nicht jedes VAD-Segment muss ein Embedding erzeugen), bevor an der Latenz gedreht wird |
| Die Zusage „kein Embedding überlebt" wird als Aussage über den gesamten Prozessspeicher gelesen | Die Grenze ist offengelegt und wird in `docs/constitution.md` geschrieben, nicht nur hier: wir nullen unsere Puffer, nicht die Arenen der Inferenz-Laufzeit |
| Der Build zieht ungeprüfte Binärarchive | Als Landmine benannt und entschieden: verifizieren oder aus den Quellen bauen. Der Preis ist Bauzeit, und der ist bewusst akzeptiert |
| Ein Raummikrofon mit mehreren Sprechern erzeugt den Eindruck, die 100-%-Zusage sei gebrochen | Die Grenze steht in `docs/architecture.md` (Korrektur 3) und gehört in Phase 6 in die README. Sie ist eine Eigenschaft des strukturellen Ansatzes, kein Fehler |

## Decision log

- 2026-07-30: Die Rust-Bindung an sherpa-onnx geprüft. `sherpa-rs` ist seit Juni 2026
  **archiviert**; sein Maintainer verweist auf die offiziellen Bindungen. Die Crate
  `sherpa-onnx` (1.13.4, Apache-2.0, statisches Linken als Default) stellt genau die
  Komponenten getrennt bereit, die unser Entwurf braucht: `VoiceActivityDetector`,
  `SpeakerEmbeddingExtractor` über `OnlineStream` und `FastClusteringConfig` als
  eigenen Schritt — daneben die Batch-API `OfflineSpeakerDiarization`, die wir
  bewusst nicht verwenden. Damit ist der inkrementelle Entwurf aus `docs/prior-art.md`
  nicht nur wünschenswert, sondern durch die API gedeckt.
- 2026-07-30: Modell-Lizenzen geprüft. pyannote-segmentation-3.0 ist **MIT** und über
  einen **un-gated** ONNX-Mirror verfügbar — damit entfällt der Hugging-Face-Login,
  den das Original verlangt. Silero VAD ist **MIT**. TEN VAD ist technisch besser,
  steht aber unter einer **modifizierten** Apache-2.0-Fassung und ist deshalb
  abgelehnt. Für die Sprecher-Embedding-Modelle weist sherpa-onnx die Lizenzprüfung
  ausdrücklich dem Nutzer zu, und die stärksten Kandidaten sind auf VoxCeleb
  trainiert, dessen Terms auf Forschung beschränken — daraus die offene Entscheidung.
- 2026-07-30: Landmine, die `cargo deny` nicht sieht: die Crate lädt beim Bauen
  vorgefertigte Binärarchive, sofern `SHERPA_ONNX_LIB_DIR` nicht gesetzt ist.
  Entschieden, dass der Build keine unverifizierten Binärquellen verwenden darf.
