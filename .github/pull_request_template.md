## Worum geht es?

<!-- Was ändert sich und warum? Verweise auf zugehörige Issues, z. B. "Closes #123". -->

## Prüfung

- [ ] `npm run check`
- [ ] `npm run check:native` (bei Änderungen in `apps/desktop/src-tauri`)
- [ ] `npm run check:xml` / `npm run check:pdf` (bei Änderungen an XRechnung, ZUGFeRD oder PDF/A)
- [ ] Windows-Tests bzw. `npm run release:gate` (bei Änderungen am Drucker oder Installer)

## Checkliste

- [ ] Tests für das geänderte Verhalten ergänzt
- [ ] Dokumentation und `CHANGELOG.md` aktualisiert
- [ ] Keine echten Rechnungen oder personenbezogenen Daten im Diff
