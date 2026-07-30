# Spec: Phase 3 — Sprechertrennung

> Created: 2026-07-30

Diese Spec liefert die Sprecherzuordnung: overlap-aware Segmentierung des
`Remote`-Stroms über pyannote-segmentation-3.0, inkrementelle Embedding-Extraktion
während der Sitzung, **eigenes** Clustering am Sitzungsende, und die Zuordnung der
Labels auf die Transkript-Segmente über Zeitüberlappung. Sie trägt das zweite
Kernversprechen des Projekts: **kein Stimmprofil überlebt die Sitzung**.

Alle Modelle laufen direkt über `ort` (Rust-ONNX-Runtime). `sherpa-onnx` kommt in
dieser Phase **nicht** zum Einsatz — die Begründung steht in den Entscheidungen und
zieht eine Änderung an `docs/constitution.md` nach sich.

Prosa auf Deutsch, Identifier und Überschriften auf Englisch —
`docs/constitution.md`, Conventions.

## Outcome

- [ ] Am Sitzungsende tragen die Segmente des `Remote`-Stroms Sprecherlabels
      (`Speaker A`, `Speaker B`, …), zugeordnet über Zeitüberlappung.
- [ ] Der `Local`-Strom trägt strukturell **„Ich"** — mit den zwei dokumentierten
      Grenzen unten (geteiltes Raummikrofon; fehlende Echokompensation).
- [ ] **DER ≤ 15 %** auf einem Referenzsample mit mindestens drei Sprechern, gemessen
      auf einer **Fern-/Mischmikrofon**-Spur (SDM/MDM), mit **0,25 s Collar** und
      **eingeschlossener** Überlappung.
- [ ] **Kein Embedding überlebt den Prozess:** nach dem Übergang nach `Ended` ist in
      keinem von der Anwendung gehaltenen Puffer ein Embedding-Wert ungleich Null
      auffindbar, und weder im Dateisystem noch in einer Datenbank existiert eines.
      Die vier bekannten Grenzen stehen im Risikoregister.
- [ ] `Clustering + Zuordnung` sind **≤ 20 s nach Abschluss des ASR-Flush** fertig —
      ein Teilbudget des 60-s-Kriteriums, gemessen ab dem Punkt, an dem die Zuordnung
      überhaupt beginnen kann.
- [ ] Der Ringpuffer-**Fan-out** aus Phase 1 trägt erstmals zwei **reale** Konsumenten;
      der `diarize`-Cursor meldet über einen ganzen Lauf **keinen** Verlust.
- [ ] `diarize` hat keinen Schreib- und keinen Netzpfad, und **kein** Embedding-Typ
      implementiert `Serialize` oder ein wertausgebendes `Debug`.
- [ ] Beide Modelle liegen mit Lizenz und SHA-256 in der `provisioning`-Allowlist, und
      die Lizenz wird **maschinell** gegen eine erlaubte Menge geprüft.
- [ ] Der Build lädt **kein** unverifiziertes Binärarchiv — weder lokal noch in CI.
- [ ] Kein Foundation-Dokument widerspricht mehr dem Code (Liste in In scope).

## Scope

### In scope

- Crate `diarize` (neu), vollständig über `ort`:
  - **Segmentierung** mit pyannote-segmentation-3.0: 10-s-Fenster, Powerset-Decoding,
    daraus lokale Sprecheraktivität **einschließlich** Überlappung. Dieses Modell
    liefert zugleich die Sprach-/Nicht-Sprach-Entscheidung — ein separates VAD-Modell
    entfällt.
  - **Embedding-Extraktion** je lokalem Sprecherbereich, inkrementell während der
    Sitzung.
  - **Eigenes agglomeratives Clustering** in sicherem Rust (Kosinus-Distanz,
    Schwellenwert) am Sitzungsende.
- Crate `core`: `SpeakerLabel`, `SpeakerId`, `SpeakerAssignment`, und die Erweiterung
  von `TranscriptSegment` um das Label. Embedding-Typen liegen **nicht** in `core`.
- Crate `session`: `diarize` als **zweiter** Leser am Ringpuffer-Cursor; Embeddings in
  `Zeroizing`-Puffern; explizites Nullen beim Übergang nach `Ended`; Anstoßen des
  Clusterings **und** die Label-Zuordnung — die Zusammenführung liegt hier.
- Crate `provisioning`: beide Modelle in die Allowlist (Version, URL, SHA-256,
  Lizenz) plus ein Test, der die Lizenz gegen eine erlaubte Menge prüft.
- Crate `cli`: Sprecherlabels in der Ausgabe.
- `xtask`: die **opt-in** DER-Messung (`cargo xtask der`) samt **RTTM-Parser** und
  **DER-Scorer**; Erweiterung des Quell- und Manifest-Audits auf `diarize`.
- **Foundation-Doc-Korrekturen** — die verbindliche Aufzählung, als **ein** Schritt:
  1. `docs/constitution.md`, Tech stack: die Zeile „VAD, Segmentierung, Embeddings,
     Clustering | `sherpa-onnx` (Apache-2.0) + pyannote-segmentation-3.0-ONNX" wird
     zu „`ort` (MIT/Apache-2.0) + pyannote-segmentation-3.0-ONNX + ein
     Sprecher-Embedding-Modell; Clustering in eigenem sicherem Rust". **Normative
     Änderung**, am Gate entschieden — Begründung unten.
  2. `docs/architecture.md`, Component map: die `diarize`-Zeile nennt „(sherpa-onnx)"
     und wird entsprechend korrigiert.
  3. `docs/architecture.md`, Boundaries: die Kante lautet
     `session` → `core` + `audio` + `asr` + `diarize`.
  4. `docs/architecture.md`, Flow 2: dort steht „**Jede Quelle** … → Fan-out an zwei
     Konsumenten". Nach dieser Phase hängt `diarize` nur am `Remote`-Strom; die Zeile
     wird darauf korrigiert.
  5. `docs/architecture.md`, Flow 3: neben „der lokale Strom bekommt strukturell
     «Ich»" gehören **beide** Grenzen — geteiltes Raummikrofon, **und** der Fall, in
     dem Phase 1 ohne Echokompensation weiterläuft und Ton der Gegenseite im
     `Local`-Strom fälschlich „Ich" trägt.
  6. `docs/architecture.md`, Flow 4 und `core`-Zeile: Umbenennen ist eine
     `protocol`-Sache, `SpeakerLabel` ist **unveränderlich**; und die Ankündigung,
     `core` bekomme in Phase 3 `zeroize`, trifft nicht zu — die Embedding-Typen liegen
     in `diarize`.
  7. `docs/workflow.md`, Environment prerequisites: dort wird verlangt, dass
     `whisper.cpp` **und `sherpa-onnx`** aus den Quellen gebaut werden. `sherpa-onnx`
     entfällt; `ort` bezieht seine Laufzeit anders.
  8. `docs/workflow.md`, Commands: die erwartete Verify-Dauer nennt ebenfalls
     `sherpa-onnx`; sie wird nachgemessen und korrigiert.
  9. `docs/constitution.md` **und** `CLAUDE.md`, Regel 1: die Benchmark-Ausnahme aus
     Phase 2 nennt namentlich `cargo xtask wer`. `cargo xtask der` fällt nicht
     darunter; die Formulierung wird auf „Entwickler-Werkzeuge des Repositories zur
     Messung der Qualitätskriterien" verallgemeinert. **Cross-Milestone-Kante** auf
     das Phase-2-Issue, das die Ausnahme schreibt.
  10. `docs/workflow.md`, Issue conventions: das Token ist als
      `Depends on milestone: #<n>` definiert; diese Phase braucht zwei Kanten, also
      wird die mehrwertige Form dort zugelassen statt stillschweigend gedehnt.

### Out of scope

- Protokoll-Ausgabe, Protokollkopf, Sprecherzahl im Kopf — Phase 4.
- Umbenennen der Labels — Phase 5, und wegen Korrektur 6 eine reine
  `protocol`-Angelegenheit ohne Vorbereitung hier.
- **Jede** Wiedererkennung über Sitzungen hinweg — `docs/vision.md`, Non-goals.
- Diarisierung des `Local`-Stroms.
- Streaming-Sprecherlabels während des Gesprächs.
- Ein separates VAD-Modell. Die Segmentierung liefert die Sprach-Entscheidung mit.

## Constraints

- `#![forbid(unsafe_code)]` in `diarize`. `ort` kapselt das FFI.
- `diarize` hat keinen Schreibpfad und keinen HTTP-Client; Modelldateien lesen ist
  erlaubt. Nur `provisioning` greift aufs Netz.
- Abhängigkeitsrichtung: `diarize` → `core` + `audio`; `session` → `core` + `audio` +
  `asr` + `diarize`. **`asr` und `diarize` kennen einander nicht.**
- Embeddings in `Zeroizing`, kein `Serialize`, kein wertausgebendes `Debug`.
- **Abhängigkeits- und Lizenz-Inventar:**

  | Gegenstand | Zweck | Lizenz | Status |
  | --- | --- | --- | --- |
  | `ort` 2.0.0-rc.x, exakt gepinnt | ONNX-Inferenz für beide Modelle | MIT/Apache-2.0 | gedeckt; **Release Candidate** — Risikozeile |
  | `ort-sys` | FFI | MIT/Apache-2.0 | gedeckt |
  | ONNX Runtime (Laufzeit) | Inferenz | MIT | Bezug siehe Entscheidung |
  | pyannote-segmentation-3.0 (ONNX) | Segmentierung **und** VAD | **MIT** (LICENSE-Datei des un-gated Mirrors, Copyright 2022 CNRS; die HF-Metadaten deklarieren keine) | Allowlist |
  | Sprecher-Embedding-Modell (VoxCeleb-trainiert) | Embeddings | **CC BY 4.0** | Allowlist, NOTICE-Eintrag |
  | `zeroize` | Nullen der Puffer | MIT/Apache-2.0 | gedeckt |
  | DER-Scorer (Ungarische Methode) | nur `xtask` | zu prüfen | vor Nutzung prüfen |

- **Kein unverifizierter Binär-Download beim Bauen.** `ort` bringt `sha2` mit und kann
  Prüfsummen verifizieren; genutzt wird der verifizierende Pfad, und die erwartete
  Summe wird gepinnt. Ist sie nicht prüfbar, wird die Laufzeit über `load-dynamic`
  aus einer von `cargo xtask bootstrap` beschafften und geprüften Datei geladen. In
  **keinem** Fall lädt der Build ungeprüft.
- Die `windows-latest`-Runner haben kein Audiogerät und keine GPU. **CI lädt kein
  Modell**; alle Tests laufen gegen Attrappen und synthetische Embeddings.

## Prior art

- [Speaker diarization without a Python runtime (Phase 3)](../prior-art.md#speaker-diarization-without-a-python-runtime-phase-3)
  — der Split Segmentierung → Embedding → Clustering und das ADOPT „inkrementell
  während der Sitzung, Clustering **einmal** am Ende" tragen diese Phase weiterhin
  vollständig. Was **nicht** trägt, ist die Annahme, `sherpa-onnx` stelle diesen Split
  als API bereit; der Eintrag wird mit Korrektur 1 nachgezogen.
- [diart / Streaming Sortformer](../prior-art.md#diart--streaming-sortformer)
  — „overlap-aware Systeme senken die DER um 3–7 Punkte" ist der Grund, warum das
  Powerset-Decoding trotz seines Aufwands gebaut wird.
- [Speaker identity and naming (Phase 7)](../prior-art.md#speaker-identity-and-naming-phase-7)
  — das AVOID „Embeddings über Sitzungen hinweg persistieren" (Art. 9 DSGVO).

## Human prerequisites

- [ ] Ein Referenzsample mit **zeitgestempelten Sprecher-Annotationen** (RTTM-förmig),
      mindestens drei Sprechern, auf einer **SDM/MDM**-Spur. **Nicht** dasselbe
      Artefakt wie Phase 2s WER-Auszug: dort genügt ein Referenz*transkript*, und die
      naheliegende WER-Wahl ist die saubere Einzel-Headset-Spur (IHM), auf der
      Diarisierung trivial und die Zahl wertlos wäre.
- [ ] Für den QA-Smoke-Test: ein Call mit **mindestens drei** Personen auf der
      Gegenseite.
- [ ] Keine Secrets, keine Accounts; die Segmentierung kommt über den **un-gated**
      Mirror, damit kein Hugging-Face-Login nötig ist.

## Prior decisions

### Der Stack — und warum `sherpa-onnx` hier nicht vorkommt

| Decision | Rationale | Date |
|---|---|---|
| **Alle Modelle laufen über `ort`; `sherpa-onnx` wird in `diarize` nicht verwendet.** Am Gate entschieden; zieht Korrektur 1 an `docs/constitution.md` nach | Zwei Prüfungen gegen die veröffentlichte `sherpa-onnx` 1.13.4 haben ergeben, dass die Bindung **weder Clustering noch Segmentierung** einzeln ausführbar macht: `FastClusteringConfig`, `OfflineSpeakerSegmentationModelConfig` und `OfflineSpeakerSegmentationPyannoteModelConfig` sind reine Config-Structs ohne Methoden, es gibt keinen Typ `OfflineSpeakerSegmentation`, und der einzige Einstieg ist `OfflineSpeakerDiarization::process(&[f32])` auf **rohem, vollständigem Audio** — das wir nie haben, weil der Ringpuffer nach 30 s überschreibt. Blieben VAD und Embedding-Extraktor: zwei dünne Hüllen um ONNX-Modelle, erkauft mit einer zweiten, gevendorten ONNX-Laufzeit, einem ungeprüften Binär-Download beim Bauen, einer Ausnahme im Phase-1-Audit (`ureq` als Build-Dependency) und der öffentlichen Funktion `sherpa_onnx::write`, die über die C-Bibliothek eine WAV-Datei schreibt und die Symbol-Blockliste vollständig unterläuft. Ein schlechter Tausch. Über `ort` fallen alle vier Probleme weg, es bleibt **eine** Laufzeit, und der Split aus `docs/prior-art.md` wird zum ersten Mal wirklich umsetzbar | 2026-07-30 |
| `ort` wird **exakt** gepinnt und ist ein **Release Candidate** (2.0.0-rc.x) | MIT/Apache-2.0, in `deny.toml` gedeckt, und die mainstream-Rust-Bindung an ONNX Runtime. Der rc-Status ist ein echtes Risiko und steht als solches im Register — nicht als Fußnote | 2026-07-30 |
| **Kein separates VAD-Modell.** Die Sprach-/Nicht-Sprach-Entscheidung kommt aus der Segmentierung | Das Powerset-Klassenset enthält „non-speech" als eigene Klasse — die Segmentierung liefert VAD als Nebenprodukt. Ein zweites Modell wäre ein zusätzlicher Download, ein zusätzlicher Lizenz-Eintrag und eine zweite Wahrheit über dieselbe Frage. Silero VAD entfällt damit ersatzlos | 2026-07-30 |

### Segmentierung und Powerset-Decoding

| Decision | Rationale | Date |
|---|---|---|
| pyannote-segmentation-3.0 läuft auf **10-s-Fenstern** über 16-kHz-Mono; das letzte Fenster wird mit Nullen aufgefüllt und die über die Audiodauer hinausreichenden Frames werden nach der Inferenz verworfen | Die dokumentierte Betriebsart des Modells. Die 10 s passen in den 30-s-Ringpuffer, also ist die Segmentierung inkrementell während der Sitzung fahrbar — die Voraussetzung dafür, dass wir nie vollständiges Audio brauchen | 2026-07-30 |
| Die Ausgabe ist `[batch, frames, 7]`: Nicht-Sprache, drei Einzelsprecher, drei Sprecher**paare**. Das Decoding nimmt je Frame das Argmax und bildet die Klasse auf die Menge der aktiven Sprecher ab | Das ist die Powerset-Darstellung: Überlappung ist **explizit im Klassenset** und muss nicht geschätzt werden. Genau daher die 3–7 DER-Punkte aus `docs/prior-art.md`. Bis zu drei gleichzeitige Sprecher **je Fenster** — die globale Sprecherzahl entsteht erst durch das Clustering über alle Fenster | 2026-07-30 |
| Fenster überlappen sich, und die Frame-Aktivitäten werden über die Überlappung aggregiert, bevor binarisiert wird | Ohne Aggregation entstünden an jeder Fenstergrenze Sprecherwechsel-Artefakte. Referenzimplementierungen (`pyannote-onnx`, Transformers.js) zeigen die Form; übernommen wird das Verfahren, kein Code | 2026-07-30 |
| Ein Embedding entsteht nur aus **nicht-überlappten** Anteilen eines lokalen Sprecherbereichs, und nur wenn davon **≥ 1,0 s** zusammenkommen | Ein Embedding aus überlappter Sprache mischt zwei Stimmen und verschiebt den Cluster-Schwerpunkt — der Vorteil des Modells würde sich in einen Nachteil verkehren. Erst das Powerset-Decoding macht diese Regel überhaupt formulierbar: es sagt uns, **welche** Frames überlappt sind | 2026-07-30 |

### Clustering, Zuordnung, Nahtstellen

| Decision | Rationale | Date |
|---|---|---|
| Clustering: **agglomerativ über Kosinus-Distanz**, Schwellenwert statt vorgegebener Sprecherzahl, Startwert **0,55**, als Konstante in `diarize`, kalibriert über `cargo xtask der` | Der Standardansatz jeder x-vector-Pipeline, überschaubar in sicherem Rust, O(n²) über wenige tausend Vektoren und damit weit innerhalb des Budgets. Niemand weiß vor dem Gespräch, wie viele Menschen auf der Gegenseite sitzen; ein Startwert ist nötig, weil die DER-Messung opt-in ist | 2026-07-30 |
| Öffentliche API: `Diarizer::push(&mut self, pcm_16k_mono: &[f32], t0: SessionOffset) -> Result<(), DiarizeError>` und `Diarizer::finish(self) -> Result<Vec<SpeakerSpan>, DiarizeError>` | `session` schiebt Frames hinein und behält die Kontrolle über Zeitachse und Reihenfolge. `finish` verbraucht `self`, damit die Embeddings nach dem Clustering nicht weiterleben können — das Typsystem erzwingt das Ende ihrer Lebensdauer | 2026-07-30 |
| Die **Label-Zuordnung liegt in `session`** | `diarize` liefert `SpeakerSpan`s auf der Sitzungs-Zeitachse; `session` legt sie über die `TranscriptSegment`s. Läge sie in `diarize`, müsste `diarize` `TranscriptSegment` kennen — die Kante, die „`asr` und `diarize` kennen einander nicht" verhindert | 2026-07-30 |
| Bei mehreren überlappenden Sprecherbereichen gewinnt das Label mit der größten Zeitüberlappung; geführt als `SpeakerAssignment { primary, overlap_ratio, runner_up }` | „Mehrdeutigkeit wird vermerkt" ohne Typ wäre für Phase 4 unbrauchbar. `overlap_ratio` erlaubt es dort, Unsicherheit sichtbar zu machen statt sie zu glätten | 2026-07-30 |
| Entartete Fälle sind Teil des Vertrags: **ein** Cluster (das 1:1-Gespräch) ergibt `Speaker A`; **null** Embeddings ergeben eine leere Zuordnung und keinen Fehler | Die entarteten Fälle sind die häufigen. Ohne Festlegung erfindet sie der Implementierer | 2026-07-30 |
| `diarize` läuft auf einem **eigenen** CPU-Thread mit auf 2 begrenzter ONNX-Threadzahl; der Ringpuffer wird über einen `Mutex` geteilt, den Leser nur für die Dauer eines `read` halten | Phase 1s Cursor-API verlangt exklusiven Zugriff, und in `crates/audio` gibt es heute kein `Arc`/`Mutex` — der erste echte Fan-out ist auch die erste Synchronisationsentscheidung. CPU-only, damit Phase 2s Kapazitätsungleichung auf der GPU unberührt bleibt | 2026-07-30 |
| Phase 2s Ungleichung bekommt einen dritten Term: `diarize` darf den ASR-Rückstand nicht wachsen lassen, maschinell geprüft über `loss_count` des `diarize`-Cursors, das über einen ganzen Lauf **0** bleibt | Ein Rückstand von `diarize` ist teurer als einer von `asr`: der Ringpuffer überschreibt nach 30 s, verlorene Sprache heißt verlorene Embeddings, und ein ganzer Sprecher kann verschwinden. Eine Beobachtung wäre dafür zu schwach | 2026-07-30 |
| Fehlt eines der Modelle und schlägt der Download fehl, startet die Sitzung **nicht** — Fehler vor dem Consent-Schritt | Konsistent mit Phase 2 (fehlendes ASR-Modell) und unterschieden vom fehlenden Mikrofon in Phase 1: eine Sitzung ohne Sprecherzuordnung wäre ein anderes Produkt, und niemand soll dafür eine Attestation bestätigen | 2026-07-30 |
| `SpeakerLabel` ist **unveränderlich** | `docs/architecture.md`, Flow 4: Umbenennen ändert ausschließlich Text im geschriebenen Protokoll. Ein veränderbares Feld an einer terminalen `Ended`-Sitzung wäre für Phase 5 ohnehin nicht erreichbar | 2026-07-30 |

### Das Nullen — vier Grenzen, nicht eine

| Decision | Rationale | Date |
|---|---|---|
| Der Embedding-Speicher wird **einmal** mit fester Kapazität allokiert und blockweise erweitert, wobei der alte Block **vor** der Freigabe explizit genullt wird | `Zeroizing` nullt beim `Drop` die **aktuelle** Allokation; wächst ein `Vec`, wird der alte Block kopiert und ungenullt freigegeben. Phase 1 hat genau diesen Fund gemacht und behoben (feste Erst-Kapazität, `RingBuffer::storage` einmal voll allokiert); eine frühere Fassung dieser Spec hat das Muster wieder eingeführt | 2026-07-30 |
| Das Inferenz-Ergebnis wird **unmittelbar** in `Zeroizing` überführt und die Zwischenallokation genullt | Jedes Embedding existiert zuerst in einer nicht-zeroisierten Allokation. Das ist keine „Arena der Laufzeit", sondern die Nahtstelle, die wir selbst aufrufen, und sie ist erreichbar | 2026-07-30 |
| Genullt wird beim Übergang nach **`Ended`**, nicht bei `stop()` | Zwischen `request_stop` und `end` liegen zwei Kanten und laut Budget bis zu 20 s, in denen die Embeddings für das Clustering notwendigerweise **leben** | 2026-07-30 |
| Der Audit prüft auch auf **`Debug`** an Embedding-Typen, nicht nur auf `Serialize` | `Zeroizing<T>` implementiert `Debug`, wenn `T` es tut — ein `#[derive(Debug)]` gäbe die Werte aus | 2026-07-30 |
| **Keine** Abschwächung von `docs/constitution.md` für die Null-Grenzen | Die Zeile ist eine **Anweisung** („Embeddings liegen in `Zeroizing`-Puffern und werden … genullt"), keine Abwesenheitsbehauptung, überclaimt also nichts; das Vision-Kriterium ist bereits auf „weder im Dateisystem noch in einer Datenbank" begrenzt; und Phase 1 hat den identischen Fall (`rubato`s FFT-Zustand hält PCM, von außen nicht zeroisierbar) als Risiko plus Issue #34 geführt, ohne die Constitution anzufassen. Dieselbe Antwort hier; die vollständige Grenze steht im Risikoregister und gehört in Phase 6 in die README | 2026-07-30 |

### Gates

| Decision | Rationale | Date |
|---|---|---|
| `Clustering + Zuordnung` bekommen **20 s**, gemessen **ab Abschluss des ASR-Flush** | Eine frühere Fassung maß ab `stop()` und hätte damit den Flush aus Phase 2 stillschweigend in dieselben 20 s gezwungen, ohne dass Phase 2 das zugesagt hat. Die Zuordnung kann ohnehin erst nach dem Flush beginnen — sie braucht beide Ergebnisse. Das Clustering darf **parallel** zum Flush laufen. Am QA-Gate werden ASR-Flush, Clustering und Zuordnung **getrennt** gemessen und gegen die 60 s der Vision summiert | 2026-07-30 |
| DER wird mit **0,25 s Collar** und **eingeschlossener** Überlappung gemessen | Eine frühere Fassung verschob die Konvention auf die Messung — aber sie bewegt die Zahl auf AMI-SDM um rund den Faktor zwei und entscheidet damit über ein normatives Kriterium. Überlappung **einzuschließen** ist die ehrliche Wahl: unser ganzes Argument für das Powerset-Decoding ist die Überlappungsbehandlung, und sie auszuschließen würde uns schmeicheln | 2026-07-30 |
| Der DER-Scorer (Ungarische Methode plus Missed/False-Alarm/Confusion) und der **RTTM-Parser** sind eingeplante Arbeit dieser Phase | DER ist keine Editierdistanz wie WER. Das stillschweigend anzunehmen wäre eine versteckte Aufgabe — Phase 2 hat denselben Punkt für den Transkript-Parser ausdrücklich eingeplant | 2026-07-30 |
| Sprecher-Embedding-Modell: ein **VoxCeleb-trainiertes** Modell unter **CC BY 4.0**, mit Namensnennung in der NOTICE | Am Gate war das als offene Frage vorgesehen; die Prüfung hat sie **geschlossen**. Eine frühere Fassung behauptete, VoxCeleb sei auf Forschung beschränkt — falsch: die Lizenz ist CC BY 4.0 und erlaubt kommerzielle Nutzung ausdrücklich; der Eindruck stammt aus der Prosa eines Datensatz-Mirrors, die der Lizenz widerspricht, die sie selbst nennt. Die vermeintlichen Alternativen sind **restriktiver**: CNCeleb erlaubt keine kommerzielle Nutzung, VoxBlink2 ist CC BY-NC-SA. Damit bleibt keine Gabelung — CC BY 4.0 ist die einzige mit einem Apache-2.0-Produkt verträgliche Option. Offengelegt bleibt: die Metadaten des Datensatzes stehen unter CC BY-**SA** 4.0, und das Urheberrecht an den zugrunde liegenden Videos bleibt bei den Rechteinhabern | 2026-07-30 |
| Alle Maschinen-Tests laufen gegen **synthetische Embeddings** und Attrappen | CI hat weder GPU noch Audiogerät; ein Test, der Modelle lädt, würde Verify netzabhängig machen — die Falle, die in Phase 2 zweimal auffiel | 2026-07-30 |
| Kein `/loopkit:design`-Zyklus | Keine UI-Fläche; die Ausgabe bleibt die CLI | 2026-07-30 |

## Tracking

- Milestone: Phase 3 — Sprechertrennung (angelegt am Spec-Acceptance-Gate)
- Issues: entstehen aus dieser Spec, sobald sie gemergt ist
- `Depends on milestone: #1, #2`. Die mehrwertige Form wird durch **Korrektur 10** in
  `docs/workflow.md` zugelassen, statt sie stillschweigend zu dehnen.
- **Kontext:** `crates/` enthält heute nur `core` und `audio`. `session`, `asr`,
  `provisioning`, `cli` und `audio-win` existieren **nicht**, und `xtask` kennt kein
  `wer`, auf dem `der` aufbauen könnte. Jede zitierte Geschwister-API ist ein
  Versprechen; die Cross-Milestone-Kanten sind bindend.

## Verification

Maschinell, in Verify und CI — **ohne Modell, ohne Netz, ohne Audiogerät**:

- [ ] `cargo xtask verify`, `cargo xtask build`, `cargo deny check` grün.
- [ ] Der Build lädt nichts ungeprüft: die ONNX-Laufzeit wird gegen eine gepinnte
      SHA-256 verifiziert, oder der Build bricht ab.
- [ ] Der Audit bewacht `diarize`: kein Schreib-, Netz- oder `serde`-Pfad, **kein
      `Serialize` und kein `Debug`** an einem Embedding-Typ — je einmal durch eine
      absichtliche Verletzung belegt.
- [ ] Der `provisioning`-Test prüft die Modell-Lizenz gegen eine erlaubte Menge.
- [ ] **Powerset-Decoding-Test** gegen synthetische Logits: jede der sieben Klassen
      wird auf die richtige Sprechermenge abgebildet, und ein Paar-Klassenframe
      erzeugt **zwei** aktive Sprecher.
- [ ] **Fensteraggregations-Test:** an einer Fenstergrenze entsteht kein
      Sprecherwechsel-Artefakt.
- [ ] **Overlap-Test:** aus überlappter Sprache entsteht kein Embedding; aus < 1,0 s
      nicht-überlappter Sprache ebenfalls nicht.
- [ ] **Null-Test:** nach `Ended` kein Embedding-Wert ungleich Null; zusätzlich belegt
      ein Test, dass ein Wachstum der Sammlung den alten Block **vor** der Freigabe
      nullt.
- [ ] **FS-Audit (Phase 1, erweitert):** eine Sitzung mit `diarize`-Konsument
      hinterlässt 0 neue Dateien.
- [ ] **Clustering-Tests:** drei getrennte Sprecher → drei Cluster ohne vorgegebene
      Zahl; **ein** Sprecher → `Speaker A`; **null** Embeddings → leere Zuordnung.
- [ ] **Zuordnungs-Test:** größte Überlappung gewinnt, `overlap_ratio`/`runner_up`
      gesetzt.
- [ ] **Fan-out-Verdrahtung:** `asr` und `diarize` als zwei **reale** Konsumenten am
      `Mutex`-geteilten Ringpuffer; `loss_count` des `diarize`-Cursors bleibt **0**.
- [ ] **`Local`-Test:** der lokale Strom erhält „Ich" ohne Embedding-Vergleich.

Manuell am Milestone-QA-Gate:

- [ ] **Sprecherzuordnung:** Call mit ≥ 3 Personen → unterscheidbare Labels, eigener
      Beitrag durchgängig „Ich".
- [ ] **AEC-Degradierung:** bei `is_aec_supported() == false` wird geprüft und
      protokolliert, wie sich die „Ich"-Zuordnung bei Lautsprecher-Nutzung verhält —
      der Fall, in dem sie **falsch** sein kann.
- [ ] **DER:** `cargo xtask der` auf der SDM/MDM-Spur ergibt ≤ 15 %, bei 0,25 s Collar
      und eingeschlossener Überlappung, mit dokumentiertem Schwellenwert.
- [ ] **Budget:** ASR-Flush, Clustering und Zuordnung getrennt gemessen; Clustering +
      Zuordnung ≤ 20 s ab Flush-Ende, Summe ≤ 60 s.
- [ ] **Nicht-Störung:** weder Call noch Live-Transkription brechen ein; der
      ASR-Rückstand wächst nicht.
- [ ] **Prozessbezogene Beobachtung:** kein Schreibzugriff außerhalb der Konsole — die
      einzige Prüfung, die Schreibzugriffe der **nativen** Laufzeit erfassen kann.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| **`ort` ist ein Release Candidate.** Ein rc trägt eine tragende Komponente | Exakt gepinnt, und die API-Nutzung bleibt auf das Nötige beschränkt (Session laden, Tensor rein, Tensor raus). Bricht ein rc-Update etwas, ist der betroffene Code klein. Ein Wechsel auf die stabile 2.0 ist eingeplante Folgearbeit |
| Das selbst gebaute Powerset-Decoding weicht vom Referenzverhalten ab und kostet DER | Der Decoding-Test gegen synthetische Logits prüft die Klassenabbildung isoliert; die DER-Messung prüft die Kette. Referenzimplementierungen sind benannt und dienen als Verhaltensmaßstab — übernommen wird das Verfahren, kein Code |
| Das eigene Clustering ist schlechter als eine erprobte Implementierung | Agglomerativ über Kosinus-Distanz ist der Standard-Erststufenansatz; die DER-Messung ist Kalibrier- und Beweismittel. Reißt sie, ist der nächste Hebel das Segmentierungs-Postprocessing — das jetzt tatsächlich erreichbar ist |
| **Die Null-Zusage hat vier bekannte Grenzen** | (1) Realloc — behoben über feste Kapazität plus Nullen alter Blöcke. (2) Die nicht-zeroisierte Zwischenallokation der Inferenz — behoben über sofortige Überführung. (3) Swap/Pagefile und Crash-Dumps — vom Prozess **nicht** kontrollierbar, gehört als Aussage in die README (Phase 6). (4) Interne Arenen der ONNX-Laufzeit — nicht erreichbar. (1) und (2) sind Arbeit dieser Phase, (3) und (4) offengelegte Grenzen — **nicht** als Abschwächung in der Constitution, dem `rubato`-Präzedenzfall folgend |
| Die Embedding-Extraktion konkurriert mit der ASR-Inferenz | `diarize` CPU-only mit begrenzter Threadzahl, GPU bleibt der ASR. Der `loss_count`-Test ist die Schranke, nicht eine Beobachtung |
| Der AMI-Auszug aus Phase 2 ist für DER unbrauchbar (IHM statt SDM/MDM) | Als Human prerequisite ausgewiesen. „Eine Quelle, zwei Kriterien" gilt für die Audioquelle, nicht für das Artefakt |
| Die Geschwister-Milestones sind noch nicht implementiert | `Depends on milestone: #1, #2` plus Cross-Milestone-Kanten je Issue. Verschiebt sich eine API, eskaliert das Issue mit `needs:planning` |

## Decision log

- 2026-07-30: **Zweimal dieselbe Fehlerart, und sie hat die Phase umgeworfen.** Die
  erste Fassung las die Existenz eines Namens (`FastClusteringConfig`) als Existenz
  einer nutzbaren Nahtstelle; die zweite wiederholte das eine Komponente weiter bei der
  Segmentierung. Gegen die veröffentlichte `sherpa-onnx` 1.13.4 geprüft: beide
  Segmentierungs-Configs sind methodenlose Structs, ein Typ `OfflineSpeakerSegmentation`
  existiert nicht, und der einzige Einstieg nimmt rohes Audio. Die Lehre — dieselbe wie
  beim Versionsstand in Phase 2 — lautet: das **veröffentlichte Artefakt** lesen, nicht
  eine Beschreibung davon.
- 2026-07-30: Am Gate entschieden, `diarize` vollständig auf `ort` zu stellen. Das war
  keine Vorliebe, sondern die Auflösung: von `sherpa-onnx` blieben nur zwei dünne
  Modell-Hüllen übrig, erkauft mit einer zweiten gevendorten Laufzeit, einem ungeprüften
  Build-Download, einer Audit-Ausnahme für `ureq` und der öffentlichen Funktion
  `sherpa_onnx::write`, die die Symbol-Blockliste unterläuft. Alle vier Probleme
  entfallen ersatzlos. `docs/constitution.md` wird entsprechend geändert — eine
  normative Änderung, die der Mensch am Gate getroffen hat, nicht die Spec.
- 2026-07-30: Silero VAD entfällt: das Powerset-Klassenset enthält „non-speech", die
  Segmentierung liefert die VAD-Entscheidung also mit. Ein Modell, ein Download und ein
  Lizenz-Eintrag weniger.
- 2026-07-30: **Lizenz-Korrektur, zweifach.** Die Behauptung, VoxCeleb sei auf Forschung
  beschränkt, war falsch — CC BY 4.0 erlaubt kommerzielle Nutzung; die Prosa stammt von
  einem Mirror und widerspricht der Lizenz, die er selbst nennt. Und die als sicherer
  angebotenen Alternativen sind **restriktiver**: CNCeleb untersagt kommerzielle
  Nutzung, VoxBlink2 ist CC BY-NC-SA. Die vermeintliche Gabelung existierte nicht; die
  Frage ist entschieden statt ans Gate getragen.
- 2026-07-30: Weiter behoben: das 20-s-Budget wurde ab `stop()` gemessen und hätte den
  ASR-Flush aus Phase 2 stillschweigend eingeschlossen (jetzt ab Flush-Ende); die
  Collar- und Overlap-Konvention war auf die Messung verschoben, obwohl sie die DER um
  etwa den Faktor zwei bewegt und damit über ein normatives Kriterium entscheidet
  (jetzt 0,25 s, Überlappung eingeschlossen); und die Korrekturliste hat drei
  Dokumentstellen nachgetragen, die die neuen Entscheidungen unwahr gemacht hätten.
