// German documentation
export const documentation = `# Jottery Dokumentation

## Inhaltsverzeichnis

- [Erste Schritte](#erste-schritte)
- [Notizen erstellen & bearbeiten](#notizen-erstellen-bearbeiten)
- [Syntaxhervorhebung](#syntaxhervorhebung)
- [Rechnermodus](#rechnermodus)
- [Suche](#suche)
  - [Einfache Suche](#einfache-suche)
  - [Tag-Suche](#tag-suche)
  - [Erweiterte Suchmodifikatoren](#erweiterte-suchmodifikatoren)
- [Mehrfachauswahl & Massenoperationen](#mehrfachauswahl-massenoperationen)
- [Versionshistorie](#versionshistorie)
- [Tastenkombinationen](#tastenkombinationen)
- [Synchronisation](#synchronisation)
- [Sicherheit & Datenschutz](#sicherheit-datenschutz)
- [Import & Export](#import-export)

---

## Erste Schritte

Jottery ist eine datenschutzorientierte, verschluesselte Notizanwendung. Alle Ihre Notizen werden lokal mit **AES-256-GCM**-Verschluesselung verschluesselt, bevor sie gespeichert werden.

> **Wichtig:** Ihr Passwort ist der Verschluesselungsschluessel. Wenn Sie es verlieren, koennen Ihre Notizen nicht wiederhergestellt werden. Es gibt keine Funktion zum Zuruecksetzen des Passworts.

---

## Notizen erstellen & bearbeiten

| Aktion | So geht's |
|--------|-----------|
| **Notiz erstellen** | Klicken Sie auf "+ Neue Notiz" oder druecken Sie \`Alt+N\` |
| **Notiz bearbeiten** | Klicken Sie auf eine Notiz in der Liste, um sie zu oeffnen |
| **Automatisches Speichern** | Aenderungen werden automatisch beim Tippen gespeichert |
| **Notiz schliessen** | Druecken Sie \`Escape\` oder klicken Sie auf eine andere Notiz |
| **Notiz anheften** | Klicken Sie auf das Anheften-Symbol, um sie oben zu halten |
| **Notiz loeschen** | Klicken Sie auf das Menue (⋮) und waehlen Sie "Loeschen" |

---

## Syntaxhervorhebung

Verwenden Sie das Sprach-Dropdown im Editor, um die Syntaxhervorhebung zu aktivieren. Unterstuetzte Sprachen sind:

- **Markdown** - mit Live-Vorschau und Code-Block-Hervorhebung
- **JavaScript/TypeScript** - ES6+ Syntaxunterstuetzung
- **Python** - einschliesslich f-Strings und Decorators
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - interaktive mathematische Ausdruecke

---

## Rechnermodus

Setzen Sie die Syntaxsprache auf **Calc**, um den interaktiven Rechner zu verwenden. Jede Zeile wird als mathematischer Ausdruck ausgewertet, wobei die Ergebnisse inline angezeigt werden.

### Funktionen

- **Grundrechenarten:** \`2 + 3 * 4\` → \`14\`
- **Variablen:** \`x = 10\` dann \`x * 2\` → \`20\`
- **Konstanten:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Funktionen:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Potenz:** \`2^10\` oder \`2**10\` → \`1024\`
- **Fakultaet:** \`5!\` → \`120\`
- **Kommentare:** Zeilen, die mit \`#\` beginnen, werden ignoriert

### Verfuegbare Funktionen

| Kategorie | Funktionen |
|-----------|------------|
| **Basis** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Potenzen** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometrie** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hyperbolisch** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Beispiel

\`\`\`
# Zinseszins berechnen
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Suche

### Einfache Suche

Tippen Sie in das Suchfeld, um Notizen zu finden. Die Suche durchsucht sowohl den Notizinhalt als auch die Tags.

| Syntax | Beschreibung |
|--------|--------------|
| \`wort\` | Notizen, die "wort" enthalten |
| \`wort1 wort2\` | Notizen, die beide Woerter enthalten (UND) |
| \`"exakter ausdruck"\` | Notizen, die den exakten Ausdruck enthalten |
| \`-wort\` | Notizen ausschliessen, die "wort" enthalten |

### Tag-Suche

| Syntax | Beschreibung |
|--------|--------------|
| \`#tagname\` | Notizen mit diesem Tag |
| \`#tag1 #tag2\` | Notizen mit beiden Tags (UND) |
| \`#tag1 \\| #tag2\` | Notizen mit einem der Tags (ODER) |

### Erweiterte Suchmodifikatoren

| Modifikator | Beschreibung | Beispiel |
|-------------|--------------|----------|
| \`has:attachment\` | Notizen mit Anhaengen | \`has:attachment\` |
| \`created:>DATE\` | Nach Datum erstellt | \`created:>2024-01-01\` |
| \`created:<DATE\` | Vor Datum erstellt | \`created:<2024-06-30\` |
| \`created:DATE..DATE\` | Im Datumsbereich erstellt | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATE\` | Nach Datum geaendert | \`modified:>2024-01-01\` |
| \`modified:<DATE\` | Vor Datum geaendert | \`modified:<2024-06-30\` |
| \`words:>N\` | Mehr als N Woerter | \`words:>100\` |
| \`words:<N\` | Weniger als N Woerter | \`words:<50\` |
| \`words:N..M\` | Wortanzahl im Bereich | \`words:50..200\` |

**Modifikatoren kombinieren:** \`#project has:attachment modified:>2024-01-01 words:>100\`

---

## Mehrfachauswahl & Massenoperationen

Waehlen Sie mehrere Notizen aus, um Massenaktionen durchzufuehren.

### Notizen auswaehlen

| Aktion | So geht's |
|--------|-----------|
| **Auswahl umschalten** | \`Ctrl/Cmd + Klick\` auf eine Notiz |
| **Bereichsauswahl** | \`Shift + Klick\` um ab der letzten Auswahl zu markieren |
| **Alle sichtbaren auswaehlen** | Klicken Sie auf "Alle auswaehlen" in der Werkzeugleiste |
| **Auswahl aufheben** | Druecken Sie \`Escape\` oder klicken Sie auf "Abbrechen" |

### Massenaktionen

Wenn Notizen ausgewaehlt sind, erscheint eine Werkzeugleiste unten mit diesen Optionen:

- **Tags hinzufuegen** - Tags zu allen ausgewaehlten Notizen hinzufuegen
- **Tags entfernen** - Bestimmte Tags von ausgewaehlten Notizen entfernen
- **Exportieren** - Ausgewaehlte Notizen als JSON exportieren
- **Zusammenfuehren** - Ausgewaehlte Notizen zu einer zusammenfuehren (nach Erstellungsdatum sortiert)
- **Loeschen** - Ausgewaehlte Notizen in den Papierkorb verschieben

---

## Versionshistorie

Jottery erstellt automatisch Versions-Snapshots beim Synchronisieren von Notizen.

| Aktion | So geht's |
|--------|-----------|
| **Historie oeffnen** | Klicken Sie auf ⋮ Menue → "Versionshistorie" oder druecken Sie \`Alt+H\` |
| **Version anzeigen** | Klicken Sie auf eine Version, um deren Inhalt zu sehen |
| **Vergleichen** | Unterschiede werden automatisch hervorgehoben |
| **Wiederherstellen** | Klicken Sie auf "Wiederherstellen", um zu einer frueheren Version zurueckzukehren |

---

## Tastenkombinationen

Alle Tastenkombinationen sind anpassbar unter Einstellungen → Tastenkombinationen.

### Standard-Tastenkombinationen

| Tastenkombination | Aktion |
|-------------------|--------|
| \`Ctrl/Cmd + K\` | Suche fokussieren |
| \`Alt + N\` | Neue Notiz erstellen |
| \`Ctrl/Cmd + Z\` | Rueckgaengig |
| \`Ctrl/Cmd + Shift + Z\` | Wiederholen |
| \`Alt + H\` | Versionshistorie |
| \`Alt + I\` | Notiz-Info |
| \`Escape\` | Notiz schliessen / Auswahl aufheben |
| \`Ctrl/Cmd + ,\` | Einstellungen oeffnen |

### Mehrfachauswahl-Tastenkombinationen

| Tastenkombination | Aktion |
|-------------------|--------|
| \`Ctrl/Cmd + Klick\` | Notizauswahl umschalten |
| \`Shift + Klick\` | Bereichsauswahl |
| \`Ctrl/Cmd + A\` | Alle gefilterten Notizen auswaehlen |

---

## Synchronisation

Jottery unterstuetzt selbst gehostete Synchronisation ueber mehrere Geraete.

### Einrichtung

1. Gehen Sie zu **Einstellungen → Synchronisation**
2. Geben Sie Ihre selbst gehostete Server-URL ein
3. **Erstes Geraet:** Klicken Sie auf "Geraet registrieren", um Sync-Anmeldedaten zu erstellen
4. **Weitere Geraete:** Verwenden Sie "Vorhandene Anmeldedaten verwenden" mit Ihren Sync-Anmeldedaten

> **Wichtig:** Alle Geraete muessen das **gleiche Passwort** verwenden, um Notizen zu entschluesseln. Das Passwort wird niemals an den Server gesendet.

### So funktioniert es

- Notizen werden **vor** dem Verlassen Ihres Geraets verschluesselt
- Der Server speichert nur verschluesselte Daten
- Die Synchronisation erfolgt automatisch, wenn Sie online sind
- Konflikte werden nach dem Prinzip "Letzter Schreibvorgang gewinnt" geloest

---

## Sicherheit & Datenschutz

| Funktion | Beschreibung |
|----------|--------------|
| **Verschluesselung** | AES-256-GCM fuer alle Notizinhalte und Tags |
| **Lokale Verschluesselung** | Alle Verschluesselung erfolgt in Ihrem Browser |
| **Passwort** | Wird niemals gespeichert oder uebertragen |
| **Automatische Sperre** | Schuetzt Notizen bei Inaktivitaet (Standard: 15 Minuten) |
| **Kein Tracking** | Keine Analysen oder Drittanbieter-Skripte |
| **Open Source** | Vollstaendiger Quellcode auf GitHub verfuegbar |

> **Tipp:** Verwenden Sie einen Passwort-Manager, um ein starkes, einzigartiges Passwort fuer Jottery zu generieren und zu speichern. Da es keine Passwort-Wiederherstellung gibt, bedeutet der Verlust Ihres Passworts den dauerhaften Verlust des Zugangs zu Ihren Notizen.

### Passwort aendern

Da Ihr Passwort der Verschluesselungsschluessel ist, gibt es keine direkte Moeglichkeit, es zu aendern. Sie koennen Ihr Passwort jedoch effektiv aendern, indem Sie:

1. Alle Ihre Notizen **exportieren** (Einstellungen → Import/Export → Exportieren)
2. Ihre lokalen Daten **loeschen** oder einen neuen Browser/ein neues Geraet verwenden
3. Jottery mit Ihrem neuen Passwort **einrichten**
4. Ihre exportierten Notizen **importieren**

Ihre Notizen werden mit dem neuen Passwort neu verschluesselt.

---

## Import & Export

### Exportieren

1. Gehen Sie zu **Einstellungen → Import/Export**
2. Klicken Sie auf "Alle Notizen exportieren"
3. Waehlen Sie einen Speicherort fuer die JSON-Datei

> **Warnung:** Exporte sind **unverschluesselt**. Bewahren Sie sie sicher auf!

### Importieren

1. Gehen Sie zu **Einstellungen → Import/Export**
2. Klicken Sie auf "Notizen importieren"
3. Waehlen Sie eine zuvor exportierte JSON-Datei
4. Notizen werden mit vorhandenen Daten zusammengefuehrt (Duplikate werden uebersprungen)

### Massenexport

Waehlen Sie mehrere Notizen aus und klicken Sie auf "Exportieren", um nur ausgewaehlte Notizen zu exportieren.
`;
