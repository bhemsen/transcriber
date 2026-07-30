# Vision

> Normativ. Nur was und warum — keine Implementierungsdetails. Rund eine Seite;
> diese Datei ist über CLAUDE.md dauerhaft im Kontext.

## Problem

Protokolle von Online-Calls entstehen heute auf zwei Wegen, die beide etwas
Wichtiges kosten. Handschriftlich: sie binden genau die Aufmerksamkeit, die im
Gespräch gebraucht wird, und bleiben lückenhaft. Per Cloud-Notetaker: ein
sichtbarer Bot sitzt im Call, Audio und Video gehen zu einem Dritt-Anbieter, und
am Ende liegen dauerhafte A/V-Artefakte irgendwo. In DSGVO-sensiblen Kontexten ist
der zweite Weg nicht vertretbar; wer ihn ablehnt, fällt auf den ersten zurück.
Auch die lokalen Open-Source-Alternativen speichern Aufnahmen oder dauerhafte
Stimmprofile und greifen das komplette System-Audio ab statt nur das Gespräch.

## Why now

Drei Voraussetzungen sind erst seit kurzem gleichzeitig erfüllt: Windows liefert
prozessbezogenes Loopback-Audio, also die Tonspur genau einer Anwendung ohne
virtuellen Audiotreiber — und macOS (Core Audio Process Taps) sowie Linux
(PipeWire) haben inzwischen äquivalente Wege. Lokale Spracherkennung läuft auf
Consumer-Hardware schneller als Echtzeit. Und Diarization ist als reines
ONNX-Modell verfügbar, also ohne Python-Laufzeit im Auslieferungspaket. Damit ist
"alles bleibt auf dem Rechner" kein Funktionsverzicht mehr.

## Target users

Primär: datenschutzbewusste Wissensarbeiter, die Cloud-Notetaker aktiv ablehnen —
Beratung, Recht, Medizin, öffentlicher Dienst, Betriebsräte, Freiberufler. Sie
erwarten eine installierbare Anwendung mit Oberfläche, kein Entwicklerwerkzeug.
Sekundär: Entwickler, die die Audio-Capture- und Diarization-Bausteine
weiterverwenden.

## Goal

Eine lokal laufende Desktop-Anwendung, mit der man eine laufende Anwendung als
Tonquelle auswählt, den Consent bestätigt und am Ende ein sprecherzugeordnetes
Gesprächsprotokoll erhält — während zu keinem Zeitpunkt eine Bild- oder
Tonaufnahme entsteht und kein Stimmprofil die Sitzung überlebt.

## USP / differentiation

Der einzige Meeting-Transkriptor, der ausschließlich die Tonspur einer gewählten
Anwendung abgreift, weder Audio noch Sprecher-Embeddings über die Sitzung hinaus
persistiert und die Aufnahme erst nach dokumentierter Consent-Bestätigung startet.
Datenminimierung geschieht beim Abgriff, nicht beim Aufräumen: was nicht erfasst
wird, muss nicht gelöscht werden. Die bewussten Gegenleistungen: kein
LLM-Summary, keine Kalender-Integration, kein Cloud-Pfad, keine
Sprecher-Wiedererkennung über Sitzungen hinweg. Die Belege je Alternative — was
übernommen und was bewusst vermieden wird — stehen in `docs/prior-art.md`.

## Success criteria

- A/V-Persistenz: 0 Bytes. Ein automatisierter Test über eine 10-minütige Sitzung
  findet kein Audio-Artefakt; kein Code-Pfad schreibt PCM auf einen Datenträger.
- Kein Stimmprofil überlebt den Prozess: nach Sitzungsende ist kein Embedding mehr
  auffindbar, weder im Dateisystem noch in einer Datenbank.
- Transkriptqualität: WER ≤ 15 % auf je einem definierten deutschen und englischen
  Meeting-Referenzsample.
- Sprecherzuordnung: "ich" gegen "Gegenseite" 100 % korrekt (strukturell über
  getrennte Ströme); DER ≤ 15 % bei mindestens drei Gegenseiten-Sprechern.
- Quellen-Isolation: Audio einer nicht ausgewählten Anwendung erscheint nachweisbar
  nicht im Protokoll (Testfall: Musik parallel abspielen).
- Time-to-first-protocol: vom Download bis zum ersten fertigen Protokoll
  ≤ 15 Minuten, ohne Installation virtueller Audiotreiber.
- Latenz: Live-Rohtext ≤ 5 s hinter dem Gesprochenen; fertiges Protokoll mit
  Sprecherlabeln ≤ 60 s nach Sitzungsende.
- Nicht-Störung: kein Audio-Ausfall im laufenden Call; lauffähig auf 8 GB VRAM mit
  funktionierendem CPU-Fallback.
- Consent-Gate: die Aufnahme startet nachweisbar nie ohne bestätigte Attestation;
  die Attestation steht mit Zeitstempel im Protokollkopf.

## Scope

### In

- Windows: eine laufende Anwendung (Prozessbaum) als Tonquelle auswählen.
- Das eigene Mikrofon parallel als separaten, strukturell unterscheidbaren Strom.
- Lokale Transkription (Deutsch, Englisch) mit Live-Textansicht.
- Sprechertrennung der Gegenseite am Sitzungsende; manuelles, sitzungsgebundenes
  Umbenennen der Labels.
- Protokollausgabe (Markdown und JSON) mit Consent-Kopf, Aufbewahrungs- und
  Löschfunktion.
- Oberfläche, Installer, Modellbereitstellung beim ersten Start.
- Audio-Backend-Abstraktion, in die macOS und Linux später eingehängt werden.

### Out

- Bildschirm- oder Videoaufzeichnung, OCR von Bildschirminhalten.
- LLM-Zusammenfassungen (frühestens nach dem MVP, dann ausschließlich lokal).
- Kalender-, Meeting- und Bot-Integrationen.
- Cloud-Spracherkennung, auch nicht mit eigenem Schlüssel.
- Mobile Plattformen, Live-Übersetzung.

## Non-goals

- Verdeckte Aufnahme, Stealth- oder Anti-Detection-Funktionen — das ist genau der
  Tatbestand des § 201 StGB und wäre als Open-Source-Projekt nicht vertretbar.
- Speichern von Ton oder Bild in irgendeiner Form — das Kernversprechen; jede Datei
  wäre sein Bruch.
- Dauerhafte Stimmprofile zur Sprecher-Wiedererkennung — biometrische Daten nach
  Art. 9 DSGVO wären eine schwerere Last als das Protokoll, das sie beschriften.
- Ein optionaler Cloud-Pfad — ein optionaler Pfad ist ein vorhandener Pfad.
- Umgehen oder Unterdrücken von Aufnahme-Indikatoren des Call-Clients.
- Ein allgemeines Diktier- oder Transkriptionswerkzeug zu sein — besetztes Feld,
  anderes Problem.
