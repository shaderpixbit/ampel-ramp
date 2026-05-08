# Ramp Dashboard — Benutzerhandbuch

Dieses Handbuch beschreibt, wie Sie die Ramp-Dashboard-Anwendung im täglichen Hofbetrieb nutzen. Es richtet sich an Mitarbeiter:innen am Verladehof, im Lager und im Büro.

## Inhalt

1. [Auf einen Blick](#auf-einen-blick)
2. [Die Ampel-Farben](#die-ampel-farben)
3. [Status einer Rampe ändern](#status-einer-rampe-ändern)
4. [Aufbau einer Rampen-Kachel](#aufbau-einer-rampen-kachel)
5. [Kennzeichen und Reservierung pflegen](#kennzeichen-und-reservierung-pflegen)
6. [2-Sekunden-Sperre](#2-sekunden-sperre)
7. [Sync-Anzeige](#sync-anzeige)
8. [Team-Chat](#team-chat)
9. [Dunkler / Heller Modus](#dunkler--heller-modus)
10. [Ihr Benutzername und Ihre Rolle](#ihr-benutzername-und-ihre-rolle)
11. [Protokoll & Statistik (Büro / Admin)](#protokoll--statistik-büro--admin)
12. [Benutzerverwaltung (Admin)](#benutzerverwaltung-admin)

---

## Auf einen Blick

Beim Öffnen des Programms sehen Sie ein Fenster mit drei Hauptbereichen:

- **Kopfzeile** oben — Werksname, Sync-Anzeige (`LIVE · 1.5s` / `OFFLINE`), Status-Zähler, Protokoll- und Benutzer-Buttons, Modus-Umschalter, Ihr Benutzername und Ihre Rolle.
- **Rampen-Übersicht** in der Mitte — **28 Rampen** in zwei Sektoren:
  - **Sektor A · Tor 30–42** (13 Rampen)
  - **Sektor B · Tor 43–57** (15 Rampen)
- **Team-Chat** rechts — Kurznachrichten an Ihre Kollegen.

![Gesamtansicht des Dashboards](images/01-overview.png)

---

## Die Ampel-Farben

Jede Rampe hat einen Status, der durch eine Farbe und ein Symbol dargestellt wird:

| Farbe | Bedeutung | Symbol |
|-------|-----------|--------|
| **Grün** (Frei) | Rampe ist frei und einsatzbereit. | Häkchen |
| **Gelb** (Wartend) | Ein LKW fährt zur Rampe oder ist reserviert. | Uhr (pulsierend) |
| **Rot** (Belegt) | Ein LKW steht an der Rampe. Be- oder Entladung läuft. | Schloss |

Oben in der Kopfzeile sehen Sie eine Zusammenfassung, wie viele Rampen aktuell in welchem Status sind.

![Status-Zusammenfassung](images/02-counts.png)

---

## Status einer Rampe ändern

Ein **Klick** auf eine Rampe schaltet sie zum nächsten Status weiter. Die Reihenfolge hängt von Ihrer Rolle ab:

| Rolle | Klick-Verhalten |
|-------|-----------------|
| **Admin** | `Frei` ↔ `Belegt` (Direktes Umschalten, überspringt `Wartend`) |
| **Büro** | `Frei` → `Wartend` → `Belegt` → `Frei` (vollständiger 3-Wege-Zyklus für Reservierungen) |
| **Lager** | `Frei` ↔ `Belegt` (Direktes Umschalten, überspringt `Wartend`) |

Die Änderung ist sofort für alle anderen Mitarbeiter:innen am Standort sichtbar (innerhalb von ~1,5 Sekunden).

![Statuswechsel einer Rampe](images/03-status-cycle.png)

> **Tipp:** Beim Wechsel auf `Frei` werden Kennzeichen und Reservierung automatisch gelöscht — die Rampe ist wieder leer.

---

## Aufbau einer Rampen-Kachel

Jede Rampe ist eine kompakte Kachel mit drei Zeilen:

| Zeile | Inhalt |
|-------|--------|
| **Oben** | Tor-Nummer (z. B. `42`) und Status-Badge (`FREI` / `WARTEND` / `BELEGT`) |
| **Mitte** | **LKW** — das Kennzeichen des aktuellen LKW (z. B. `M-TR 2418`) |
| **Unten** | **Res.** — die Reservierung („Reserviert für …") — nur für Büro/Admin sichtbar oder wenn ein Wert gesetzt ist |

Unter den drei Zeilen erscheint außerdem eine kleine Info-Leiste mit:

- **Benutzer und Zeit** der letzten Statusänderung
- **Verweildauer** im aktuellen Status (live mitgezählt, z. B. `02:14` für 2 Min 14 Sek)

![Aufbau einer Rampen-Kachel](images/04-ramp-tile.png)

---

## Kennzeichen und Reservierung pflegen

**Lager-Mitarbeiter:innen** sehen und pflegen nur das Kennzeichen.
**Büro/Admin** sehen und pflegen zusätzlich die Reservierung.

So bearbeiten Sie ein Feld:

1. Klicken Sie auf das Kennzeichen-Feld (`LKW`) oder das Reservierungs-Feld (`Res.`) auf der Kachel.
2. Tippen Sie den Text ein — z. B. `M-TR 2418` oder `Lieferung Müller GmbH 14:30`.
3. Drücken Sie **Enter** zum Speichern oder **Esc** zum Abbrechen.

> **Wichtig:** Inline-Bearbeitungen verändern **nicht** den Zeitstempel der letzten Statusänderung — die Verweildauer läuft also weiter, auch wenn Sie nur das Kennzeichen nachtragen.

---

## 2-Sekunden-Sperre

Sobald jemand den Status einer Rampe ändert, wird diese Rampe für **2 Sekunden** für alle Clients gesperrt. Während dieser Sperre:

- Sehen Sie ein dunkles Overlay mit einem rotierenden Lade-Symbol auf der Rampe.
- Klicks werden ignoriert.
- Andere Mitarbeiter:innen sehen die gleiche Sperre.

Diese Sperre verhindert, dass zwei Personen gleichzeitig denselben Status ändern und es zu Konflikten kommt.

![Gesperrte Rampe](images/05-locked.png)

---

## Sync-Anzeige

In der Kopfzeile sehen Sie eine kleine Anzeige:

- **Grüner Punkt + `LIVE · 1.5s`** — Die Verbindung zum Netzlaufwerk steht. Alle 1,5 Sekunden wird der Stand abgeglichen.
- **Punkt + `OFFLINE`** — Die Verbindung ist unterbrochen. Sie sehen ggf. veraltete Daten, und Klicks auf Rampen werden nicht gespeichert.

![Sync- und Offline-Anzeige](images/06-sync-indicator.png)

> **Bei `OFFLINE`:** Prüfen Sie zuerst Ihre Netzwerkverbindung. Bleibt das Problem länger bestehen, wenden Sie sich an Ihre IT-Abteilung. Ein Neustart der App ist meist nicht nötig — die Anzeige wechselt automatisch zurück auf `LIVE`, sobald die Verbindung wieder steht.

---

## Team-Chat

Rechts im Fenster befindet sich ein einfacher Chat-Bereich für kurze Absprachen mit Kollegen am Hof.

- Tippen Sie Ihre Nachricht in das Feld unten.
- Drücken Sie **Enter** oder klicken Sie auf den Senden-Pfeil.
- Eigene Nachrichten erscheinen rechts (dunkel), Nachrichten von Kollegen links (hell).
- Bei jedem neuen Tag wird ein Datums-Trenner eingefügt (`Heute`, `Gestern` oder Datum).
- Ungelesene Nachrichten werden mit einem kleinen Zähler-Badge auf dem Chat-Button angezeigt.

![Team-Chat](images/07-chat.png)

> **Hinweis:** Es werden die letzten 50 Nachrichten angezeigt; gespeichert werden bis zu 100. Ältere Nachrichten verschwinden automatisch.

---

## Dunkler / Heller Modus

Oben rechts neben Ihrem Benutzernamen finden Sie einen Schalter mit Sonne / Mond.

- **Klick auf die Sonne** → Wechsel in den dunklen Modus.
- **Klick auf den Mond** → Wechsel in den hellen Modus.

Die Wahl wird auf Ihrem Rechner gespeichert.

![Modus-Umschalter](images/08-theme-toggle.png)

---

## Ihr Benutzername und Ihre Rolle

Das Programm erkennt Ihren Windows-Benutzernamen automatisch und zeigt ihn oben rechts zusammen mit Ihrer Rolle an:

- **Admin** — voller Zugriff inkl. Benutzerverwaltung.
- **Büro** — Zugriff auf Protokoll, Kennzeichen und Reservierung; voller 3-Wege-Status-Zyklus.
- **Lager** — Standardrolle für neue Benutzer; nur Kennzeichen, kein Protokoll.

Der Name wird gespeichert, wann immer Sie:

- den Status einer Rampe ändern (sichtbar im Info-Streifen unter der Rampe), oder
- eine Chat-Nachricht senden, oder
- ein Kennzeichen / eine Reservierung pflegen.

Sie müssen sich nicht extra anmelden.

![Benutzer-Anzeige](images/09-user-badge.png)

> **Erstanmeldung:** Der allererste Benutzer, der die App öffnet, wird automatisch zum Admin. Alle weiteren neuen Benutzer erhalten standardmäßig die Rolle **Lager**. Ein Admin kann die Rolle nachträglich ändern (siehe [Benutzerverwaltung](#benutzerverwaltung-admin)).

---

## Protokoll & Statistik (Büro / Admin)

Über den Button **Protokoll** in der Kopfzeile öffnen Sie ein Fenster mit zwei Teilen:

- **Tagesprotokoll** — chronologische Liste aller Statuswechsel des aktuellen Tages mit Uhrzeit, Rampe, Vorher → Nachher, Dauer im vorigen Status, Mitarbeiter:in und Kennzeichen.
- **Statistik** — Auswertung über einen Zeitraum (Tag, Woche, Monat) mit:
  - Anzahl Statuswechsel
  - Durchschnittliche Verweildauer
  - Aktivste Rampen
  - Aktivste Mitarbeiter:innen

Über den **Drucken**-Button können Sie das Protokoll als PDF speichern oder ausdrucken.

![Protokoll & Statistik](images/10-protocol.png)

> **Hinweis:** Lager-Mitarbeiter:innen haben keinen Zugriff auf diesen Bereich.

---

## Benutzerverwaltung (Admin)

Über den Button **Benutzer** (nur für Admins sichtbar) öffnen Sie eine Tabelle aller registrierten Benutzer mit ihrer aktuellen Rolle.

Pro Benutzer können Sie die Rolle umstellen:

- **Lager** (Standard) — Nur Status-Toggle und Kennzeichen.
- **Büro** — Plus Reservierung und Protokoll.
- **Admin** — Voller Zugriff inkl. Benutzerverwaltung.

Änderungen sind sofort wirksam — die betroffenen Benutzer sehen die neue Rolle beim nächsten Statusabgleich (innerhalb von ~1,5 Sekunden), ohne die App neu zu starten.

![Benutzerverwaltung](images/11-user-mgmt.png)

> **Sicherheitshinweis:** Lassen Sie immer mindestens einen Admin im System. Wenn Sie sich selbst zum Lager-Benutzer machen, verlieren Sie sofort den Zugriff auf die Benutzerverwaltung.

---
