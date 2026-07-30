# Architecture

> Strukturell, lebendes Dokument — das volatilste Artefakt. Wird aktualisiert,
> sobald sich Komponenten, Grenzen oder Flüsse ändern. Greenfield: das hier ist
> ein Seed, kein fertiger Entwurf.

## Component map

Cargo-Workspace, Crates unter `crates/`, Namensschema `transcriber-<concern>`.

| Component | Responsibility |
| --------- | -------------- |
| `core` | Domänentypen: `SessionState`, `SessionId`, `ConsentAttestation`, `TranscriptSegment`, `SpeakerLabel`, `StreamIdentity` (`Local` / `Remote`), Zeitachse, Fehlertypen. Kein I/O, keine Plattform-Abhängigkeiten, `thiserror` erlaubt (ab Phase 3 zusätzlich `zeroize`) |
| `audio` | `AudioSource`- und `SourceFactory`-Trait, PCM-Frame-Typen, fest dimensionierter Ringpuffer, Resampling auf 16 kHz mono, synthetische Testton-Quelle (`TestToneSource`/`TestToneSources`) |
| `audio-win` | WASAPI Process Loopback und Mikrofon-Capture, Prozessbaum-Enumeration. Das einzige Crate, das die `unsafe`-Ausnahme ziehen darf, wenn ein konkreter Fall sie erzwingt |
| `asr` | `SpeechToText`-Trait und whisper-rs-Implementierung, Fenster-Scheduling für den Live-Strom, GPU-/CPU-Ladder |
| `diarize` | VAD, overlap-aware Segmentierung, inkrementelle Embedding-Extraktion, globales Clustering am Sitzungsende (sherpa-onnx) |
| `session` | Zustandsmaschine und Orchestrator (`Session`): Consent-Gate, Pipeline-Verdrahtung, Event-Bus, Sitzungs-Lebenszyklus. Hält die Zero-Persistence-Invariante |
| `protocol` | Rendert und schreibt Markdown und JSON, Protokollkopf, Aufbewahrung und Löschung. Der einzige Schreiber |
| `provisioning` | Modell-Allowlist, Download mit SHA-256-Prüfung, Cache-Verzeichnis. Der einzige Netzzugriff |
| `cli` | Harness-Binary für die Phasen 1–4, bevor eine Oberfläche existiert |
| `app` (`src-tauri`) | Tauri-Commands und -Events als Brücke von `session` zur Oberfläche. Enthält keine Fachlogik |
| `frontend/` | React 19 + Vite: Quellenauswahl, Attestation-Dialog, Live-View, Protokollliste, Einstellungen |

## Boundaries

- Abhängigkeitsrichtung: `core` ← `audio` ← `audio-win`; `asr` und `diarize` →
  `core` + `audio`; `session` → `core` + `audio`; `protocol` → nur `core`;
  `provisioning` → `core`.
- `protocol` hängt bewusst nicht von `audio` ab. Der einzige Weg, Audio auf Platte
  zu schreiben, wäre eine neue Kante im Abhängigkeitsgraph — und die fällt im
  Review auf.
- `asr` und `diarize` kennen einander nicht. Beide konsumieren Frames aus dem
  Fan-out der Session; zusammengeführt wird erst am Sitzungsende über
  Zeitüberlappung.
- `app` und `cli` sind austauschbare Frontends derselben `session`-API. Was nur in
  einem von beiden funktioniert, ist ein Architekturfehler.
- Threading: WASAPI-Capture läuft event-getrieben auf einem eigenen Thread je
  Quelle, ohne async. ASR und Embedding-Extraktion laufen als blockierende Worker.
  Die Orchestrierung nutzt tokio; Audio-Callbacks betreten die Runtime nie.

## Key flows

1. **Start.** Prozessliste aus `audio-win` → Nutzer wählt eine Anwendung →
   Attestation-Dialog → `ConsentAttestation` entsteht als Wert → ein
   `CapturePlan` trägt das gewählte `CaptureSubject` und die geöffneten
   `AudioSource`s (Mikrofon und Prozessbaum-Loopback) →
   `Session::start(consent, plan)` aktiviert die Sitzung. Ohne die Attestation
   existiert kein Startpfad.
2. **Live.** Jede Quelle schreibt Frames in ihren festen Ringpuffer → Resampling →
   Fan-out an zwei Konsumenten: (a) ASR-Fenster → whisper →
   `TranscriptSegment{stream, t0, t1, text}` → Event an die Oberfläche; (b) VAD
   schließt ein Sprachsegment → Embedding extrahiert → `Zeroizing`-Vektor mit
   Zeitstempel im RAM. Frames werden danach überschrieben, nie kopiert.
3. **Sitzungsende.** Die Embeddings des Remote-Stroms werden global geclustert →
   `Speaker A/B/C`; die Labels werden über Zeitüberlappung auf die Segmente gelegt;
   der lokale Strom bekommt strukturell "Ich". Embeddings werden genullt. Das
   Protokoll wird zusammengesetzt — Kopf mit Attestation und Zeitstempel, gewählter
   Anwendung, Modellversionen und Sprecherzahl — und von `protocol` als `.md` und
   `.json` geschrieben. Nichts anderes bleibt zurück.
4. **Umbenennen.** "Speaker A" → "Anna" ändert ausschließlich Text im geschriebenen
   Protokoll. Kein Embedding wird dafür gespeichert, keine Wiedererkennung in der
   nächsten Sitzung.
5. **Erststart.** `provisioning` lädt die Modelle aus der Allowlist, prüft SHA-256,
   legt sie im Cache-Verzeichnis ab und übergibt Pfade an `asr` und `diarize`.

## Where new code goes

- Neue Plattform → neues Crate `audio-<os>`, das `AudioSource` implementiert. Sonst
  ändert sich nichts.
- Neue STT-Engine → weitere Implementierung von `SpeechToText` in `asr`, ohne
  Änderung an `session`.
- Neues Ausgabeformat → Renderer in `protocol`.
- Neue Oberfläche → `frontend/` plus ein Tauri-Command in `app`; die Fachlogik
  wandert nach `session`.
- Alles, was PCM berührt → `audio`. Alles, was auf Platte schreibt → `protocol`
  oder `provisioning`, nirgends sonst.
- Alles, was ins Netz greift → `provisioning`, mit Eintrag in der Allowlist.
