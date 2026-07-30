---
kind: "ui"
color:
  background: "#101418"
  foreground: "#E6E9EC"
  primary: "#4C8DFF"
  secondary: "#2A313A"
  accent: "#3DDC97"
  muted: "#8A929B"
  border: "#262D35"
  destructive: "#FF5C5C"
type:
  font-sans: "Inter, 'Segoe UI Variable', 'Segoe UI', system-ui, sans-serif"
  font-mono: "'JetBrains Mono', 'Cascadia Code', ui-monospace, monospace"
  scale: "12 / 14 / 16 / 20 / 24 / 32 px"
  weights: "400 / 500 / 600"
  line-height: "1.5 body / 1.25 headings"
spacing:
  unit: "4px"
  scale: "4 / 8 / 12 / 16 / 24 / 32 / 48 px"
radii:
  sm: "4px"
  md: "8px"
  lg: "14px"
  full: "9999px"
shadow:
  sm: "0 1px 2px rgba(0,0,0,.35)"
  md: "0 8px 24px rgba(0,0,0,.45)"
---

# Design contract

> Design-Vertrag für die loopkit-Skills (`/loopkit:design`, `/loopkit:plan`,
> `/loopkit:implement`) — die einzige Quelle für Medium, Regeln und Handoff des
> Designs. Geschwister-Dokument zu `docs/workflow.md`.

## Overview

Eine ruhige, dichte Werkzeug-Oberfläche, die während eines Calls neben dem
Call-Client läuft und nicht um Aufmerksamkeit kämpft. Dark-first, weil sie neben
Videobildern und dunklen Meeting-Clients steht; das Light-Set wird in Phase 5 als
zweiter Token-Satz ergänzt, nicht als Umfärbung dieses hier.

Die Farbwahl folgt einer Zustandslogik, nicht einer Marke: `accent` (Grün) sagt
"läuft, und nichts davon wird gespeichert" — der beruhigende Normalzustand dieses
Werkzeugs. `destructive` (Rot) ist ausschließlich Löschaktionen und dem
Aufnahme-Indikator vorbehalten, damit Rot im Produkt nie beliebig wird. `primary`
trägt genau eine Aktion pro Ansicht. `muted` und `border` halten die Dichte
lesbar, ohne Linien zu betonen.

Zielmarke Barrierefreiheit: WCAG 2.1 AA auf jedem interaktiven Element, inklusive
sichtbarem Fokus-State mit Offset — ein Werkzeug, das man während eines Gesprächs
bedient, muss vollständig per Tastatur bedienbar sein, ohne den Blick vom Call zu
nehmen.

## Design tool

- Tool / MCP: **Claude (Artifact-Vorschau)** als primärer Editor. Entwürfe entstehen
  als Artifact-Seite, werden dort iteriert und als Screenshot ins Repo exportiert.
  Kein zweites Werkzeug, kein externer Design-Account.
- Das Werkzeug ist der **Editor, nie die Quelle der Wahrheit**. Durable state sind
  die Tokens im Front-Matter dieser Datei und die committeten Screenshots.
- Auth: ausschließlich in-session über die bestehende Anmeldung — kein Headless-Lauf,
  kein API-Key, kein Scheduler.

## Where designs live

- Source / working designs: Artifact-Seite pro Design-Zyklus, verlinkt aus der
  jeweiligen Spec. Editierfläche, kein durable state.
- Committed tokens: das Front-Matter dieser Datei ist die verbindliche Quelle. Ab
  Phase 5 wird daraus `frontend/src/styles/tokens.css` generiert; abweichende
  Werte dort sind ein Merge-Blocker.
- Committed assets: `docs/design/assets/` — exportierte Screenshots, benannt nach
  der Fläche (`source-picker.png`, `consent-dialog.png`, `live-view.png`).

## Durable form

Die verbindliche Form eines Designs ist **eine im Repo committete Datei** — die
Tokens oben oder ein Screenshot unter `docs/design/assets/` — referenziert aus der
Spec oder dem Issue. Ein Link auf eine Artifact-Seite ist **keine** gültige
verbindliche Form. Wenn `/loopkit:design` fertig ist, existiert das Design als
committete Datei, nicht als Link.

## Review path

- Reviewer: ein in-session Agent-Reviewer mit Fokus auf die Do's/Don'ts unten und
  die WCAG-2.1-AA-Marke (Kontrast, Fokus-States, Tastaturbedienbarkeit,
  Zustandsmarkierung nicht nur über Farbe).
- Das Design wird **am Spec-Acceptance-Gate** als Teil des Spec-Pakets geprüft,
  nicht an einem eigenen Stop.

## Components

- **Button** — Varianten `primary` (genau eine pro Ansicht), `secondary`,
  `destructive`, `ghost`. Höhe 32px kompakt / 40px normal, Radius `md`, Padding
  12/16. Zustände default / hover / active / disabled / focus; Fokus als 2px-Ring in
  `primary` mit 2px Offset, nie `outline: none`.
- **Select (Quellenauswahl)** — listet laufende Anwendungen mit Prozessnamen und
  Icon, gefiltert auf Prozesse mit aktiver Audio-Ausgabe. Leerzustand mit Hinweis,
  nicht mit leerer Liste. Radius `md`, Border `border`, Hintergrund `secondary`.
- **Consent-Dialog** — modal, per Escape **nicht** schließbar, Bestätigen erst nach
  aktivem Häkchen aktiv. Fokusfalle mit Rückgabe des Fokus an den Auslöser. Trägt
  den Volltext der Attestation, keine gekürzte Fassung.
- **Transcript-Line** — Zeitstempel in `mono`/`muted`, Sprecher-Chip in `secondary`
  mit Radius `full`, Text in `foreground`. Der eigene Beitrag wird über den Chip
  unterschieden, nicht über eine abweichende Textfarbe.
- **Status-Bar** — Aufnahme-Indikator in `destructive` **plus** Text ("Erfassung
  läuft") plus Icon; daneben die Dauer und die gewählte Anwendung. Nie nur farbig.
- **Protokoll-Listeneintrag** — Titel, Datum, Sprecherzahl, Dauer; Löschaktion als
  `destructive` `ghost`-Button mit Bestätigung.
- **Toggle** — für Einstellungen; beschriftet, Zustand zusätzlich als Text.

## Do's and Don'ts

**Do**

- Nur die benannten Tokens verwenden — kein Roh-Hex außerhalb dieser Datei.
- Schriften **lokal bündeln**. Ein Werkzeug, das Netz-Nullversprechen macht, darf
  beim Start keinen Webfont abrufen; ein CDN-Font wäre ein Verstoß gegen die
  Constitution, nicht nur eine Stilfrage.
- Jeden Zustand zusätzlich zur Farbe durch Text oder Icon markieren.
- Die Spacing-Skala für jedes Margin und Padding benutzen.
- WCAG 2.1 AA auf jedem interaktiven Element einhalten, Fokus sichtbar mit Offset.

**Don't**

- Farbe, Schrift oder Radius einführen, die nicht im Front-Matter stehen.
- Einen Artifact-Link für das Design halten — die committete Datei ist es.
- Ein drittes Human-Gate erfinden; Design wird am Spec-Acceptance-Gate mitgeprüft.
- Den Aufnahme-Indikator dezent gestalten. Er ist die eine Stelle, an der die
  Oberfläche laut sein darf.
