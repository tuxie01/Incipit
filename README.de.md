# Incipit

*English version: [README.md](README.md)*

Incipit ist ein Plugin für Claude Code. Es richtet ein Softwareprojekt jedes
Mal auf dieselbe Weise ein und betreibt es danach mit einem kleinen Team von
Agenten, deren Rechte durchgesetzt und nicht nur beschrieben werden.

`/incipit:newproject` legt Konfiguration und Agenten an, befragt dich, bis das
Projekt definiert ist (die „Grill-Session“), baut die Werkzeuge, mit denen
sich prüfen lässt, ob das Ziel erreicht ist, und übergibt das Projekt an
einen Lead-Agenten. Ab dann läuft die Arbeit über Tickets. Ein Ticket-Server
entscheidet, wer was darf: Nur der Tester kann ein Ticket schließen, und nur
mit hinterlegter Evidenz.

Version 0.1.4.

## Inhalt

1. [Welches Problem es löst](#1-welches-problem-es-löst)
2. [Systeme und Voraussetzungen](#2-systeme-und-voraussetzungen)
3. [Installation](#3-installation)
4. [Ein Projekt einrichten](#4-ein-projekt-einrichten)
5. [Das Team](#5-das-team)
6. [Tickets](#6-tickets)
7. [Änderungen an der Spec](#7-änderungen-an-der-spec)
8. [Das Entscheidungslog](#8-das-entscheidungslog)
9. [Wie Agenten Arbeit übergeben](#9-wie-agenten-arbeit-übergeben)
10. [Wie die Regeln durchgesetzt werden](#10-wie-die-regeln-durchgesetzt-werden)
11. [Befehle für den Owner](#11-befehle-für-den-owner)
12. [Dateien in einem Projekt](#12-dateien-in-einem-projekt)
13. [Der Alltag mit dem Team](#13-der-alltag-mit-dem-team)
14. [Ein Projekt aktualisieren](#14-ein-projekt-aktualisieren)
15. [Grenzen und offene Punkte](#15-grenzen-und-offene-punkte)
16. [Incipit weiterentwickeln](#16-incipit-weiterentwickeln)
17. [Lizenz](#lizenz)

## 1. Welches Problem es löst

Ein Team von Coding-Agenten driftet auf vorhersehbare Weise ab. Der
koordinierende Agent fängt an, selbst zu reparieren. Der Agent, der den Code
geschrieben hat, erklärt ihn für fertig. Eine fehlschlagende Prüfung wird
still abgeschwächt. Ein Ticket wird umformuliert, bis es zu dem passt, was
gebaut wurde. Entscheidungen aus einer Session sind in der nächsten weg.

Incipit setzt gegen jeden dieser Fälle einen Mechanismus statt einer Bitte:

| Abdrift | Mechanismus |
|---|---|
| Der Koordinator erledigt die Arbeit selbst | Der Lead hat kein Werkzeug, um eine Datei zu schreiben oder einen Befehl auszuführen |
| Der Erbauer erklärt die eigene Arbeit für fertig | Nur der Tester kann schließen; was der Tester selbst baut, braucht einen Auditor |
| Prüfungen werden abgeschwächt, damit sie bestehen | Prüfskripte gehören dem Tester; der Implementierer kann sie nicht schreiben |
| Tickets werden nachträglich umformuliert | Tickets wachsen nur; weniger verlangen geht nur über ein neues Ticket und den Owner |
| Evidenz wird nachträglich geändert | Evidenz wird beim Eintragen gehasht; beim Schließen entsteht eine Quittung |
| Die Spec ändert sich unbemerkt | Kein Agent kann die Spec schreiben; Änderungen sind Vorschläge, die der Owner übernimmt |
| Entscheidungen verschwinden zwischen Sessions | Ein Entscheidungslog, das nur wächst und das der Lead bei jedem Start liest |

Die Person, der das Projekt gehört, heißt durchgehend **Owner**. Der Owner
entscheidet, die Agenten empfehlen.

## 2. Systeme und Voraussetzungen

### Betriebssysteme

| System | Stand |
|---|---|
| macOS auf Apple Silicon | Hauptplattform. Gebaut und benutzt auf einem Mac mini M4; der gesamte Ablauf (Einrichtung, Import, Teamarbeit, Upgrades) ist dort gelaufen. |
| Linux x86_64 | Grundsätzlich unterstützt. Das Binary wird unter Linux gebaut, und die Testsuite (242 Fälle) besteht dort. Ein vollständiges Projekt ist unter Linux noch nicht in einer Claude-Code-Session gelaufen. |
| macOS auf Intel, Linux auf ARM | Sollte funktionieren, weil nichts architekturspezifisch ist. Nicht getestet. |
| Windows | Nicht unterstützt. Die Skripte sind bash, der Hook ist ein Shell-Befehl, und die Pfade setzen `$HOME` voraus. WSL könnte gehen, ist aber ungetestet. |

### Software

- **Claude Code** mit Plugin-Unterstützung, Projekt-Agenten, der Einstellung
  `agent`, `PreToolUse`-Hooks, die den aufrufenden Agenten melden, und
  Projekt-MCP-Servern. Entwickelt gegen Claude Code 2.1.
- **Rust-Toolchain** (`cargo`), um das Binary `incipit` zu bauen. Entwickelt
  mit Rust 1.97; der Code nutzt Edition 2021 und nichts Neues. Auf einem
  Rechner mit vorgebautem Binary nicht nötig (siehe Abschnitt 15).
- **bash** ab 3.2. Die Skripte verzichten auf alles, was neuer ist als die
  bash, die macOS mitbringt.
- **git**. Quittungen halten den Commit fest; die Einrichtung committet vor
  der Aktivierung.
- **rsync** oder **tar** für den Import. `rsync` wird genommen, wenn es da
  ist.
- Netzzugang für den ersten Build, um drei Crates samt Abhängigkeiten zu
  laden: `serde`, `serde_json`, `sha2`.

Systemweit wird nichts installiert. Das Binary liegt in
`~/.incipit/bin/incipit`, die Build-Ausgabe in `~/.incipit/build`.

## 3. Installation

Direkt aus GitHub installieren. Das Repository ist sein eigener Marketplace:

```
claude plugin marketplace add tuxie01/Incipit
claude plugin install incipit@incipit
```

Oder aus einem lokalen Klon, entweder für eine Session laden:

```
git clone https://github.com/tuxie01/Incipit.git
claude --plugin-dir /pfad/zu/Incipit
```

oder den Klon dauerhaft installieren:

```
claude plugin marketplace add /pfad/zu/Incipit
claude plugin install incipit@incipit
```

Das Binary installiert `/incipit:newproject` bei der ersten Benutzung selbst.
Von Hand installieren oder aktualisieren:

```
bash /pfad/zu/Incipit/scripts/install-server.sh
```

Das Skript vergleicht die installierte Version mit dem Quelltext und tut
nichts, wenn beide übereinstimmen. Es nimmt ein vorgebautes Binary aus
`server/dist/`, wenn eines zum Rechner passt (`incipit-Darwin-arm64`,
`incipit-Linux-x86_64`), sonst baut es mit `cargo`.

Optional, damit `incipit` ohne vollen Pfad funktioniert:

```
export PATH="$HOME/.incipit/bin:$PATH"
```

Das Plugin wird nur für die Einrichtung gebraucht. Ein eingerichtetes Projekt
trägt Agenten, Hooks und Konfiguration selbst und braucht nur noch das
Binary.

## 4. Ein Projekt einrichten

In einem leeren oder bestehenden Verzeichnis, in Claude Code:

```
/incipit:newproject
```

Die Einrichtung läuft in fünf Phasen. Jede wird in `.incipit/state.json`
festgehalten, sodass sie an jeder Stelle abbrechen und mit demselben Befehl
weiterlaufen kann.

| Phase | Was passiert | Fertig, wenn |
|---|---|---|
| scaffold | Die Vorlagen werden kopiert. Vorhandene Dateien bleiben. | Alle Scaffold-Dateien existieren |
| grill | Interview, schreibt `SPEC.md` und `.incipit/config.json` | Der Owner die Spec freigibt |
| toolchain | Build-, Lint- und Testwerkzeuge; jedes Kriterium bekommt eine Prüfung | `scripts/check.sh` komplett durchläuft |
| team | Agenten-Briefs und `CLAUDE.md` bekommen die Projektspezifika | Kein Platzhalter mehr übrig ist |
| activate | Die Einrichtung wird committet, der Lead wird zur Hauptsession | Der Owner bestätigt |

`incipit phase show` prüft die Dateien, nicht nur den Status. Ein von Hand
geleerter Ordner, in dem die versteckte Statusdatei liegen blieb, wird als
`scaffold: incomplete` gemeldet.

### Ein bestehendes Projekt importieren

Um ein altes, unfertiges Projekt neu zu starten, beantwortest du die Frage
nach der Scaffold-Phase mit dessen Pfad oder rufst selbst
`/incipit:import <verzeichnis>` auf. Das alte Projekt kann alles Mögliche
sein; es muss kein Claude-Code-Projekt gewesen sein.

- Der alte Stand wird nach `legacy/` kopiert, ohne Versionsverwaltungsdaten,
  Abhängigkeiten und Build-Ausgaben, und schreibgeschützt. Das
  Quellverzeichnis wird nicht verändert.
- Alte Agenten-Anweisungen (`CLAUDE.md`, `.claude/`, `.mcp.json`, `AGENTS.md`)
  werden umbenannt, damit sie im neuen Projekt nicht als gültige Anweisungen
  geladen werden.
- Die Session fragt, wessen Code das ist, unter welcher Lizenz er steht und
  ob `legacy/` in die Versionsverwaltung kommt.
- Danach liest sie die Kopie und schreibt `docs/legacy-survey.md`: Zweck,
  Technik, eine Zeile je Baustein mit seinem Zustand, was unfertig ist, was
  sich aufzuheben lohnt. Sie urteilt durch Lesen und behauptet nie, dass
  etwas funktioniert.
- Die Grill-Session setzt auf dieser Bestandsaufnahme auf und klärt je
  Baustein: `adopt`, `rewrite` oder `drop`.

### Die Grill-Session

Eine Frage pro Runde, mit Empfehlung, wo es eine echte Wahl gibt. Die Session
arbeitet der Reihe nach ab: Kontext, Ziel, Nicht-Ziele, vorhandene Arbeit,
Abnahmekriterien, Referenz oder Orakel, Randbedingungen, Verzeichnisstruktur,
Toolchain, Versionsverwaltung, Audit-Regel, Team, Owner-Entscheidungen.

Drei Regeln sind die wichtigsten:

- Jedes Abnahmekriterium bekommt eine Nummer (`AC-1`, ...) und den Vermerk, ob
  sein Prüfweg `proven` oder `unproven` ist. Unbelegt ist erlaubt; die nächste
  Phase probiert jeden aus.
- Einen Namen, eine Abkürzung oder einen Pfad, den die Session nicht sicher
  einordnen kann, liest sie sofort oder fragt nach. Sie bildet ihn nie auf
  etwas Ähnliches ab, das sie kennt.
- Alles in der Spec stammt vom Owner oder aus einer Datei. Die Session fügt
  keine eigenen Anforderungen hinzu.

### Die Toolchain-Phase

Leitet aus der Spec die konkrete Toolchain ab und holt für die ganze Liste
die Freigabe des Owners ein, auch für Details wie Compiler-Flags. Sie
schreibt `docs/test-plan.md` mit einer Zeile je Kriterium, probiert jeden
unbelegten Prüfweg aus und baut `scripts/check.sh`: einen Befehl, der Build,
Lint und Tests ausführt, je Stufe `PASS`, `FAIL` oder `SKIPPED` ausgibt und
immer bis zur letzten Zeile läuft.

Gebaut wird das Harness, nicht die Testsuite: so viel, dass jede Art von
Prüfung nachweislich funktioniert, mit einem Selbsttest, der ohne das Produkt
besteht. Ein Kriterium, das sich in dieser Umgebung nicht prüfen lässt, geht
zurück an den Owner.

### Aktivierung

Vor der Aktivierung prüft die Session die Git-Identität, committet die
Einrichtung, führt nach deiner Bestätigung `incipit activate` aus und
committet auch das. Die Aktivierung trägt `"agent": "lead"` in
`.claude/settings.json` ein: Jede neue Session im Projekt startet dann als
Lead. `incipit deactivate` im Terminal macht das rückgängig, etwa um die
Grill-Session zu wiederholen.

## 5. Das Team

Fünf Rollen sind immer dabei.

| Rolle | Schreibt | An Tickets | Startet Agenten | Shell |
|---|---|---|---|---|
| lead | nichts | liest; führt das Entscheidungslog | ja | nein |
| architect | Dokumentation | legt an, ergänzt; schlägt Spec-Änderungen vor | nein | nein |
| implementer | Produktquelltext | setzt eigene Tickets auf `implemented` | nein | ja |
| tester | Tests, Prüfskripte, Evidenz | Urteil, **Schließen**; baut Tester-Tickets | nein | ja |
| auditor | Evidenz | Audit-Ergebnis | nein | nur lesendes git |

Zwei Rollen sind optional. Die Grill-Session fragt danach; `incipit role add
<name>` schaltet eine später zu.

| Rolle | Schreibt | Zweck |
|---|---|---|
| security | Evidenz | Prüft Tickets mit `security_required` auf Sicherheitsmängel. Ein solches Ticket schließt nicht ohne ihr PASS. Nur lesendes git. |
| researcher | `docs/research/` | Beantwortet eine präzise Frage an der Primärquelle und schreibt die Antwort mit Fundstellen auf. Hat Websuche. |

Die genauen Schreibpfade jeder Rolle stehen in `.incipit/config.json` unter
`roles`, als Listen von Pfadpräfixen `write_allow` und `write_deny`. Eine
Sperre gewinnt immer über eine Erlaubnis. Die Briefs verweisen auf diese
Datei, statt die Pfade zu wiederholen, damit eine Regeländerung keinen Brief
veraltet zurücklässt.

Der Lead ist die Hauptsession. Worker startet nur der Lead, jeden auf genau
einem Ticket.

## 6. Tickets

Tickets liegen als JSON-Dateien in `.incipit/tasks/` und werden nur über den
Ticket-Server gelesen und geändert.

### Zustände

```
open -> in_progress -> implemented -> accepted -> closed
         ^   |  ^            |
         |   v  |            +-- Erbauer zieht seine Arbeit zurück: in_progress
         | blocked
         +-- FAIL bei Urteil, Audit oder Security-Prüfung: zurück auf open

jeder unfertige Zustand -> withdrawn   (nur der Owner)
```

### Wer baut: die Ticket-Rolle

Jedes Ticket hat eine `role`. `implementer`-Tickets baut der Implementierer,
der Tester prüft sie. `tester`-Tickets sind für Testwerkzeuge, Harness-Arbeit
und Prüfungen: Der Tester baut und prüft sie, und der Server erzwingt
`audit_required`, damit der Erbauer nie der einzige Richter ist.

### Tickets wachsen nur

Ein Ticket wird nie umgeschrieben. Was es verlangt, steht mit dem Anlegen
fest. Solange es offen oder blockiert ist, kann der Architect eine
Klarstellung, ein weiteres Kriterium oder eine neue Priorität ergänzen oder
die Audit- oder Security-Pflicht einschalten. Nichts lässt sich entfernen
oder umformulieren. Um weniger zu verlangen, legt der Architect ein neues
Ticket mit `supersedes` an, und der Owner zieht das alte zurück. Jede
Abschwächung läuft damit über den Owner.

### Schließen

`task_close` wird verweigert, solange nicht alles davon gilt:

- das Ticket ist `accepted`, wofür ein PASS-Urteil des Testers nötig ist,
- das Urteil wurde nach der letzten Implementierung eingetragen,
- bei `audit_required`: ein PASS-Audit nach der letzten Implementierung,
- bei `security_required`: eine PASS-Security-Prüfung nach der letzten
  Implementierung,
- jede Evidenzdatei ist seit dem Eintragen unverändert.

Urteile, Audits und Prüfungen brauchen eine nicht leere Evidenzdatei unter
`.incipit/evidence/`. Die Datei wird in diesem Moment gehasht (SHA-256).

### Quittungen

Beim Schließen schreibt der Server eine Quittung ins Ticket:

- den Git-Commit und ob das Arbeitsverzeichnis sauber war (Ticket-Dateien
  und Vorschläge bleiben bei dieser Prüfung außen vor, weil der Server sie
  selbst ändert),
- jede Evidenzdatei mit ihrem Hash,
- einen Hash über Titel, Scope, Kriterien und Ergänzungen des Tickets.

`incipit ticket verify` prüft alle geschlossenen Tickets gegen ihre
Quittungen und meldet jede Evidenzdatei, die fehlt oder geändert wurde.

### Werkzeuge

| Werkzeug | Rolle | Zweck |
|---|---|---|
| `task_list`, `task_get` | alle | lesen |
| `task_create` | architect | Ticket anlegen |
| `task_amend` | architect | ein offenes oder blockiertes Ticket ergänzen |
| `task_status` | die Rolle des Tickets | open, in_progress, blocked, implemented |
| `task_comment` | alle außer dem Lead | einzeilige Notiz |
| `task_verdict` | tester | PASS oder FAIL mit Evidenz |
| `task_audit` | auditor | PASS oder FAIL mit Evidenz |
| `task_security` | security | PASS oder FAIL mit Evidenz |
| `task_close` | tester | schließen und Quittung schreiben |
| `spec_propose` | architect | Spec-Änderung vorschlagen |
| `decision_log` | lead | ans Entscheidungslog anhängen |
| `decision_list` | alle | Entscheidungslog lesen |

## 7. Änderungen an der Spec

`SPEC.md` ist die maßgebliche Quelle, und kein Agent kann sie schreiben.
Ändert eine Entscheidung des Owners die Spec, reicht der Architect mit
`spec_propose` einen Vorschlag ein: eine Liste exakter Ersetzungen, von denen
jede auf genau eine Stelle der aktuellen Datei passen muss. Einen Vorschlag,
der sich nicht anwenden ließe, lehnt der Server ab.

Der Owner sieht ihn sich an und entscheidet:

```
incipit spec show P-0001
incipit spec apply P-0001      # oder: incipit spec reject P-0001 "Grund"
git diff SPEC.md
```

Ein Vorschlag wird ganz oder gar nicht angewendet. Hat sich die Spec seit dem
Einreichen geändert und passt eine Stelle nicht mehr, wird nichts angewendet.

## 8. Das Entscheidungslog

Der Lead erinnert sich zwischen Sessions an nichts und schreibt keine
Dateien. Was der Owner entschieden hat und was noch offen ist, stünde sonst
nur in einem Chat, der nach einem Neustart weg ist.

Deshalb führt der Lead mit `decision_log` ein Log: Entscheidungen (in den
Worten des Owners), Fragen an den Owner, Antworten und Notizen für die
nächste Session. Einträge werden nur angehängt. Eine Frage schließt sich
durch eine Antwort, die auf sie verweist. Zu Beginn jeder Session liest der
Lead das Log und setzt bei den offenen Fragen an.

```
incipit decisions          # offene Fragen und die letzten Einträge
incipit decisions open
incipit decisions all
```

## 9. Wie Agenten Arbeit übergeben

Agenten sprechen in zwei festen Blöcken. Details bleiben im Ticket und in den
Evidenzdateien; die Blöcke tragen Nummern, Pfade und Urteile. Das hält den
Verkehr zwischen den Agenten klein.

Lead an Worker:

```
HANDOVER
ticket: T-0007
role: implementer
order: implement | build | verify | close | audit | security | research | plan
scope: <ein Satz>
spec: SPEC.md#<Abschnitt>
accept: <"ticket" oder die Kriterien, auf die es bei diesem Auftrag ankommt>
stop: <wann anhalten und melden statt weiterzumachen>
```

Worker an Lead:

```
STATUS
ticket: T-0007
role: implementer
verdict: DONE | PASS | FAIL | BLOCKED
state: <Ticket-Zustand jetzt>
evidence: <Pfad oder "-">
note: <eine Zeile oder "-">
```

Der Worker liest das Ticket selbst; Ticket- und Spec-Text werden nie in einen
Block kopiert. Der Lead liest eine Evidenzdatei nur bei `FAIL` oder
`BLOCKED` und bestätigt den Ticket-Zustand vor dem nächsten Schritt mit
`task_get`.

## 10. Wie die Regeln durchgesetzt werden

Drei Schichten, jede unabhängig von den anderen.

1. **Werkzeuglisten.** Jeder Brief in `.claude/agents/` nennt genau die
   Werkzeuge seiner Rolle. `task_close` steht nur beim Tester. In der Liste
   des Leads fehlt jedes Werkzeug, das eine Datei schreibt oder einen Befehl
   ausführt.
2. **Guard-Hook.** Ein `PreToolUse`-Hook führt vor jedem Schreibzugriff,
   jedem Shell-Befehl und jedem Ticket-Aufruf `incipit guard` aus. Er
   erkennt den aufrufenden Agenten und prüft den Aufruf gegen die Rolle:
   Ticket-Werkzeuge gegen eine feste Tabelle, Schreibzugriffe gegen
   `write_allow` und `write_deny`. Tickets, Vorschläge und das
   Entscheidungslog schreibt nur der Server, und das gilt für alle, auch für
   die Owner-Session. Die Konfiguration des Teams selbst (`.claude/`,
   `.mcp.json`, die Steuerdateien in `.incipit/`) ist für jede Rolle
   gesperrt. Fehlt das Binary, verweigert der Hook den Aufruf, statt ihn
   durchzulassen.
3. **Ticket-Server.** Setzt die Ticket-Regeln durch, egal wer aufruft: die
   Zustandsfolge, die Ticket-Rolle, die Evidenzpflicht, die Hashes.

**Berechtigungsabfragen.** Claude Code fragt den Owner, bevor ein Agent ein
Werkzeug des Ticket-Servers zum ersten Mal benutzt. Ein Agent im Hintergrund
kann diese Abfrage nicht anzeigen, sein Aufruf wird dann abgelehnt; bei
`task_close` ist das vorgekommen. Incipit gibt seine eigenen Werkzeuge nicht
von sich aus frei. Der Owner entscheidet: die Abfrage bestätigen, wenn sie
kommt, oder `"permissions": {"allow": ["mcp__incipit"]}` in
`.claude/settings.json` eintragen. Der Eintrag überspringt nur die Abfrage.
Der Guard prüft weiterhin jeden Aufruf gegen die Rolle.

**Nur lesendes git.** Auditor, Security-Reviewer und Researcher urteilen
durch Lesen, aber Fragen wie „unverändert seit“ oder „welche Commits gehören
zu diesem Ticket“ lassen sich nur mit git beantworten. Ihre Shell führt
deshalb genau einen lesenden git-Befehl pro Aufruf aus (`log`, `show`,
`diff`, `ls-files`, `blame` und einige mehr) und sonst nichts: keine Pipes,
Umleitungen, Variablen oder Verkettungen, keine schreibenden Optionen wie
`--output`, kein `git -c`. Diese Prüfung ist exakt, weil ein Befehl ohne
Shell-Operatoren genau ein Programm mit seinen Argumenten ist.

**git selbst.** Keine Rolle darf die git-Konfiguration ändern (`git config`,
`git -c`) oder unter `.git/` schreiben, denn die git-Konfiguration bestimmt,
welche Programme git ausführt, etwa Hooks, und zwar für jede Rolle, die
danach committet.

Was exakt ist und was nicht:

- Schreibzugriffe über die Werkzeuge Edit und Write werden exakt geprüft.
  Pfade werden einschließlich Symlinks aufgelöst und ohne Rücksicht auf
  Groß- und Kleinschreibung verglichen, sodass weder `.Incipit/Tasks` noch
  ein Link ins Ticket-Verzeichnis durchkommt.
- Shell-Befehle werden per Textvergleich geprüft. Das stoppt direkte
  Zugriffe auf geschützte Pfade und die Benutzung der `incipit`-CLI durch
  eine Rolle. Einen absichtlich verschleierten Befehl stoppt es nicht. Rollen
  mit Shell (Implementierer, Tester) werden deshalb durch ihre Briefs und
  durch das gehalten, was der Auditor liest, nicht durch den Guard allein.

## 11. Befehle für den Owner

Im Terminal im Projektverzeichnis ausführen. `incipit` findet das Projekt
über das aktuelle Verzeichnis.

```
incipit status                         was ansteht und welche Schritte bei dir liegen
incipit ticket list [status]
incipit ticket show <id>
incipit ticket withdraw <id> <grund>   ein Ticket aus der Warteschlange nehmen
incipit ticket role <id> implementer|tester
incipit ticket verify                  geschlossene Tickets gegen Quittungen prüfen
incipit spec list | show <id> | apply <id> | reject <id> [grund]
incipit decisions [open|all]
incipit role add security|researcher
incipit phase [show] | done <name> | reset <name>
incipit activate | deactivate
incipit upgrade
incipit version
```

`incipit status` ist der Befehl zum Merken. Er listet Tickets, offene
Vorschläge, offene Fragen und den Git-Stand auf und endet mit den Schritten,
die bei dir liegen, jeweils mit dem fertigen Befehl.

Rollen können die CLI nicht benutzen: Der Guard lehnt jeden Shell-Befehl
eines Agenten ab, der sie aufruft.

## 12. Dateien in einem Projekt

```
SPEC.md                      was gebaut wird und wie „fertig“ geprüft wird
CLAUDE.md                    kurze Orientierung für jeden Agenten
.mcp.json                    startet den Ticket-Server
.claude/settings.json        Guard-Hook, der Lead als Hauptagent
.claude/agents/              ein Brief je Rolle
.incipit/config.json         Befehle, Struktur, Rollenregeln, Policies
.incipit/protocol.md         Übergabeblöcke, Zustände, wer was darf
.incipit/state.json          Einrichtungsphasen
.incipit/tasks/              Tickets (nur der Server)
.incipit/proposals/          Spec-Vorschläge (nur der Server)
.incipit/decisions.json      Entscheidungslog (nur der Server)
.incipit/evidence/           Evidenz von Tester, Auditor, Security
.incipit/import.txt          Herkunft eines Imports, falls vorhanden
docs/test-plan.md            welche Prüfung welches Kriterium belegt
docs/legacy-survey.md        Bestandsaufnahme eines Imports, falls vorhanden
docs/research/               Notizen des Researchers, falls vorhanden
scripts/check.sh             der eine Prüfbefehl
legacy/                      schreibgeschützte Kopie eines Imports, falls vorhanden
```

Alles außer `legacy/` (falls du es ausgeschlossen hast) und den
Build-Ausgaben ist zum Committen gedacht, auch die Tickets, das
Entscheidungslog und die Evidenz. Sie sind das Protokoll davon, wie das
Projekt entstanden ist.

## 13. Der Alltag mit dem Team

Claude Code im Projektverzeichnis mit einem einfachen `claude` starten. Die
Session ist der Lead. Ein guter erster Auftrag:

```
Leite die ersten Tickets aus SPEC.md und docs/test-plan.md ab. Nur das
Fundament, höchstens zehn. Danach stoppen und mir die Liste zeigen.
```

Sieh dir die Liste an und lass dann ein Ticket komplett durchlaufen, bevor
der Lead mehrere am Stück abarbeitet. Der übliche Weg eines Tickets:

1. Der Lead gibt es dem Implementierer, der baut, committet und es auf
   `implemented` setzt.
2. Der Tester prüft jedes Kriterium selbst, schreibt die Evidenz, committet,
   trägt das Urteil ein und schließt.
3. Ist ein Audit oder eine Security-Prüfung Pflicht, lässt der Tester das
   Ticket auf `accepted`, der Lead schickt den Auditor oder den Reviewer und
   danach noch einmal den Tester zum Schließen.
4. Ein `FAIL` setzt das Ticket zurück auf `open`, mit Verweis auf die
   Evidenzdatei.

Zu dir bringt der Lead: alles von der Liste der Owner-Entscheidungen,
Scope-Änderungen, Lücken oder Widersprüche in der Spec, das dritte `FAIL` am
selben Ticket und jeden Wunsch, eine Prüfung abzuschwächen. Manche Schritte
liegen allein bei dir, und der Lead nennt dir den Befehl: ein Ticket
zurückziehen, einen Spec-Vorschlag übernehmen.

Implementierer und Tester committen mit `git add -A`. Das nimmt den
Ticket-Stand und die Dokumente der Rollen ohne Shell mit.

`/incipit:feedback` fragt eine Session, wo das Plugin selbst im Weg war.

## 14. Ein Projekt aktualisieren

Ein Projekt trägt Kopien der Briefs und des Protokolls. Nach einem Update des
Plugins Claude Code im Projekt beenden und ausführen:

```
bash /pfad/zu/Incipit/scripts/install-server.sh
cd /pfad/zum/projekt
incipit upgrade
git add -A && git commit -m "Upgrade Incipit"
```

`incipit upgrade` ersetzt in jedem Brief die Werkzeugliste, hängt einen
Abschnitt mit den geänderten Regeln an und tauscht `.incipit/protocol.md`
aus. Text, der für das Projekt geschrieben wurde, bleibt stehen. Der Befehl
lässt sich gefahrlos wiederholen.

## 15. Grenzen und offene Punkte

- **Ticket-Backend:** nur Dateien im Repository. GitHub Issues ist geplant;
  die Schnittstelle für weitere Backends ist vorhanden.
- **Vorgebaute Binaries:** noch nicht veröffentlicht, jeder Rechner baut
  also mit `cargo`. Der Release-Workflow `.github/workflows/release.yml` baut
  sie, sobald ein Versions-Tag gepusht wird; `install-server.sh` lädt sie
  noch nicht herunter.
- **Shell-Sperre:** Textvergleich, wie in Abschnitt 10 beschrieben.
- **Plattformen:** im Einsatz auf macOS mit Apple Silicon. Linux ist durch
  Build und Testsuite abgedeckt, nicht durch einen vollständigen
  Projektlauf. Windows wird nicht unterstützt.
- **Ein Lead zur Zeit:** Zwei Claude-Code-Sessions im selben Projekt sind
  untereinander nicht abgestimmt, abgesehen von der Dateisperre des
  Ticket-Speichers.
- **Projekttypen:** eine generische Vorlage. Vorlagen für bestimmte
  Projektarten (Rust, Embedded, Retro) sind als weitere Verzeichnisse unter
  `templates/` geplant, auswählbar mit `INCIPIT_TEMPLATE=<name>`.
- **Evidenz ohne Ausführen:** Auditor, Security-Reviewer und Researcher
  lesen Code und git-Historie, bauen und starten aber nichts. Was sich nur
  durch Ausführen des Produkts bestätigen lässt, wird so gemeldet und dem
  Tester überlassen.

## 16. Incipit weiterentwickeln

```
.claude-plugin/              Plugin- und Marketplace-Manifest
skills/                      newproject, import, grill, toolchain, team, feedback
templates/base/              was scaffold in ein Projekt kopiert
templates/optional-agents/   Briefs der optionalen Rollen
scripts/                     install-server.sh, scaffold.sh, import.sh
server/                      das Binary incipit (Rust)
.github/workflows/           Release-Workflow: baut die Binaries bei einem Versions-Tag
```

Das Binary ist ein Programm mit drei Gesichtern: der Ticket-Server (MCP über
stdio, JSON-RPC von Hand geschrieben), der Guard und die Owner-CLI. Die
Briefs der Rollen und das Protokoll sind einkompiliert; deshalb funktionieren
`incipit upgrade` und `incipit role add` ohne den Plugin-Ordner.

```
cargo test --release --manifest-path server/Cargo.toml
cargo build --release --manifest-path server/Cargo.toml
bash server/tests/smoke.sh server/target/release/incipit
claude plugin validate .
```

`server/tests/smoke.sh` treibt das gebaute Binary von außen: den
Ticket-Lebenszyklus über MCP, jede Guard-Entscheidung, die Owner-Befehle und
das Upgrade eines älteren Projekts.

Ändert sich eine Regel, kommt sie in die Vorlagen-Briefs und als datierter
Abschnitt nach `server/src/owner.rs` (`update_note`), damit bestehende
Projekte sie über `incipit upgrade` erhalten.

## Lizenz

Copyright (C) 2026 Ingo Uhlemann

Incipit ist freie Software: Du kannst es unter den Bedingungen der GNU
General Public License, Version 3, wie von der Free Software Foundation
veröffentlicht, weitergeben und verändern. Es wird in der Hoffnung
verbreitet, dass es nützlich ist, aber ohne jede Gewährleistung, sogar ohne
die implizite Gewährleistung der Marktreife oder der Eignung für einen
bestimmten Zweck. Der vollständige Text steht in [LICENSE](LICENSE); maßgeblich
ist die englische Originalfassung.
