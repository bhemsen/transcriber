# transcriber — Roadmap

> Lebendes Dokument: die sequenzierte Warteschlange der Phasen. Das Hand-off an
> `/loopkit:plan`, das die nächste Phase zieht, Spec und Issues erzeugt und beides
> hierher zurückverlinkt. Keine Status-Marker — der Fortschritt lebt in den
> GitHub-Issues und -Milestones, auf die jede Phase verlinkt.

## Phase overview

| Phase | Name | Spec | Milestone |
|---|---|---|---|
| 1 | Capture-Fundament Windows | [spec](specs/spec-capture-foundation.md) | [#1](https://github.com/bhemsen/transcriber/milestone/1) |
| 2 | Lokale Transkription | [spec](specs/spec-local-transcription.md) | [#2](https://github.com/bhemsen/transcriber/milestone/2) |
| 3 | Sprechertrennung | — | — |
| 4 | Protokoll und Aufbewahrung | — | — |
| 5 | Oberfläche | — | — |
| 6 | Auslieferung | — | — |
| 7 | Automatisches Namens-Mapping | — | — |
| 8 | macOS-Backend | — | — |
| 9 | Linux-Backend | — | — |

Eine Phase bekommt einen Spec-Link, sobald `/loopkit:plan` sie entwirft, und einen
Milestone-Link, sobald die Spec gemergt ist.

## Phaseninhalte

- **Phase 1 — Capture-Fundament Windows.** Per-Prozess-Loopback und Mikrofon als
  zwei getrennte Ströme, fest dimensionierter Ringpuffer, Session-Zustandsmaschine
  mit typseitigem Consent-Gate, CLI-Harness. Dazu die betriebssystemseitige
  Echokompensation auf dem Mikrofon-Strom — ohne sie bricht die strukturelle
  Sprechertrennung, sobald der Nutzer Lautsprecher statt Kopfhörer verwendet.
  Liefert die Quellen-Isolation und das Consent-Kriterium.
- **Phase 2 — Lokale Transkription.** whisper-rs mit GPU-/CPU-Ladder,
  Fenster-Scheduling auf dem Live-Strom, definiertes deutsches und englisches
  Referenzsample samt WER-Messung. Liefert die Kriterien Transkriptqualität und
  Live-Latenz.
- **Phase 3 — Sprechertrennung.** VAD, overlap-aware Segmentierung, inkrementelle
  Embedding-Extraktion, globales Clustering am Sitzungsende, DER-Messung. Liefert
  das Sprecherkriterium und die Zusicherung, dass kein Stimmprofil den Prozess
  überlebt.
- **Phase 4 — Protokoll und Aufbewahrung.** Markdown- und JSON-Ausgabe, Kopf mit
  Attestation und Zeitstempel, Löschfunktion, FS-Audit-Test als Dauergate. Liefert
  das Null-Byte-Kriterium nachweisbar.
- **Phase 5 — Oberfläche.** Tauri und React: Quellenauswahl, Attestation-Dialog,
  Live-View, Protokollliste, Sprecher umbenennen, Einstellungen, Deutsch und
  Englisch.
- **Phase 6 — Auslieferung.** Installer, Modellbereitstellung beim Erststart mit
  SHA-256-Prüfung, Onboarding, README mit Rechtshinweis. Liefert das
  Time-to-first-protocol-Kriterium.
- **Phase 7 — Automatisches Namens-Mapping.** Opt-in und explorativ: Sprechernamen
  aus dem Call-Client ableiten, mit sauberer Degradierung auf manuelles Labeln.
  Verzichtbar, ohne das Versprechen zu brechen.
- **Phase 8 — macOS-Backend.** Core Audio Process Taps hinter `AudioSource`, ohne
  Screen-Recording-Berechtigung.
- **Phase 9 — Linux-Backend.** PipeWire-Node-Auswahl hinter `AudioSource`, gegen die
  PipeWire-API neu geschrieben statt aus GPL-Quellen übernommen.

Die Sequenz ist ein Walking Skeleton: nach Phase 4 existiert ein vollständig
nutzbares Kommandozeilenwerkzeug, das alle Kernkriterien der Vision erfüllt. Phase
5 und 6 machen es für die Zielgruppe zugänglich, 7 bis 9 sind Erweiterungen.

## North star

Ein Protokoll entsteht, ohne dass je eine Aufnahme existiert.
