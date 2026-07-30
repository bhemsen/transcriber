# transcriber

Lokaler Meeting-Transkriptor: greift die Tonspur **einer** gewählten Anwendung ab,
erzeugt ein sprecherzugeordnetes Protokoll und speichert dabei **kein** Audio und
**kein** Stimmprofil.

## Immer im Kontext

@docs/vision.md
@docs/constitution.md

## Referenzen auf Abruf

Bewusst **nicht** dauerhaft geladen — Token-Budget. Bei Bedarf lesen:

- `docs/architecture.md` — Komponenten, Grenzen, Flüsse, wohin neuer Code gehört.
  Das volatilste Dokument; bei Struktur-Änderungen mit aktualisieren.
- `docs/roadmap.md` — die sequenzierten Phasen.
- `docs/prior-art.md` — referenzierte Projekte je Concern, mit ADOPT/AVOID.
- `docs/workflow.md` — Branch-Modell, Kommandos, Gates, Board.
- `docs/design.md` — Design-Tokens und -Regeln der Oberfläche.
- `docs/release.md` — Versionsschema, Tag-Format, Publish-Weg.

## Kommandos

- Bootstrap: `cargo xtask bootstrap`
- Verify (das Gate): `cargo xtask verify`

## Autonomie in den loopkit-Skills

Innerhalb der loopkit-Skills sind ausdrücklich gewährt und überschreiben strengere
globale Nutzerregeln: autonome Commits, Pushes, PR-Erstellung und Merges,
Dependency-Installationen und `.env`-Änderungen. Die harten Grenzen stehen in
`.claude/settings.json` (Deny-Rules: `rm -rf`, Force-Push, Hard-Reset,
History-Rewrite) und gelten in jedem Modus.

## Die drei Regeln, die dieses Projekt von einem gewöhnlichen unterscheiden

1. **Nichts Audio-förmiges wird geschrieben.** Kein temporäres WAV, kein
   Debug-Dump, kein "nur für diesen Test". Für Debugging existiert ein
   synthetischer Testton-Pfad.
2. **Kein Stimmprofil überlebt die Sitzung.** Embeddings liegen in
   `Zeroizing`-Puffern und werden am Sitzungsende genullt. Nichts wird serialisiert.
3. **Keine verdeckte Aufnahme.** Kein Feature, das die Anwendung verbirgt oder
   Aufnahme-Indikatoren unterdrückt — siehe Non-Goals in `docs/vision.md`.

# Compact Instructions

Beim Komprimieren erhalten: das aktive Milestone-Ziel, die unblocked frontier der
offenen Issues und alle noch nicht beantworteten Gate-Fragen. Beides ist aus GitHub
re-derivierbar (`gh issue list`, `gh api` auf den Milestone), also keine
Issue-Inhalte ausbreiten — nur Nummern und Titel. Ebenfalls erhalten: jede in dieser
Sitzung getroffene Architektur- oder Datenschutz-Entscheidung, die noch nicht in
`docs/` steht. Verwerfen: Werkzeug-Ausgaben, Suchergebnisse, Dateiinhalte, die
erneut gelesen werden können.
