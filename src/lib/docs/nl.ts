// Dutch documentation
export const documentation = `# Jottery Documentatie

## Inhoudsopgave

- [Aan de slag](#aan-de-slag)
- [Notities maken en bewerken](#notities-maken-en-bewerken)
- [Syntaxisaccentuering](#syntaxisaccentuering)
- [Rekenmodus](#rekenmodus)
- [Zoeken](#zoeken)
  - [Eenvoudig zoeken](#eenvoudig-zoeken)
  - [Zoeken op labels](#zoeken-op-labels)
  - [Geavanceerde zoekmodificatoren](#geavanceerde-zoekmodificatoren)
- [Meervoudige selectie en bulkbewerkingen](#meervoudige-selectie-en-bulkbewerkingen)
- [Versiegeschiedenis](#versiegeschiedenis)
- [Sneltoetsen](#sneltoetsen)
- [Synchronisatie](#synchronisatie)
- [Beveiliging en privacy](#beveiliging-en-privacy)
- [Importeren en exporteren](#importeren-en-exporteren)

---

## Aan de slag

Jottery is een privacygerichte, versleutelde notitie-applicatie. Al uw notities worden lokaal versleuteld met **AES-256-GCM** encryptie voordat ze worden opgeslagen.

> **Belangrijk:** Uw wachtwoord is de encryptiesleutel. Als u dit verliest, kunnen uw notities niet worden hersteld. Er is geen wachtwoordherstelfunctie.

---

## Notities maken en bewerken

| Actie | Hoe te doen |
|-------|-------------|
| **Een notitie maken** | Klik op "+ Nieuwe notitie" of druk op \`Alt+N\` |
| **Een notitie bewerken** | Klik op een notitie in de lijst om deze te openen |
| **Automatisch opslaan** | Wijzigingen worden automatisch opgeslagen terwijl u typt |
| **Een notitie sluiten** | Druk op \`Escape\` of klik op een andere notitie |
| **Een notitie vastpinnen** | Klik op het punaisepictogram om deze bovenaan te houden |
| **Een notitie verwijderen** | Klik op het menu (⋮) en selecteer "Verwijderen" |

---

## Syntaxisaccentuering

Gebruik het taalkeuzemenu in de editor om syntaxisaccentuering in te schakelen. Ondersteunde talen zijn onder andere:

- **Markdown** - met live preview en codeblok-accentuering
- **JavaScript/TypeScript** - ES6+ syntaxisondersteuning
- **Python** - inclusief f-strings en decorators
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - interactieve wiskundige expressies

---

## Rekenmodus

Stel de syntaxistaal in op **Calc** om de interactieve rekenmachine te gebruiken. Elke regel wordt geëvalueerd als een wiskundige expressie, met resultaten die inline worden weergegeven.

### Functies

- **Basisrekenen:** \`2 + 3 * 4\` → \`14\`
- **Variabelen:** \`x = 10\` dan \`x * 2\` → \`20\`
- **Constanten:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Functies:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Machtsverheffen:** \`2^10\` of \`2**10\` → \`1024\`
- **Faculteit:** \`5!\` → \`120\`
- **Opmerkingen:** Regels die beginnen met \`#\` worden genegeerd

### Beschikbare functies

| Categorie | Functies |
|-----------|----------|
| **Basis** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Machten** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometrie** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hyperbolisch** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Voorbeeld

\`\`\`
# Bereken samengestelde rente
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Zoeken

### Eenvoudig zoeken

Typ in het zoekvak om notities te vinden. De zoekopdracht doorzoekt zowel de notitie-inhoud als de labels.

| Syntax | Beschrijving |
|--------|--------------|
| \`woord\` | Notities die "woord" bevatten |
| \`woord1 woord2\` | Notities die beide woorden bevatten (EN) |
| \`"exacte zin"\` | Notities die de exacte zin bevatten |
| \`-woord\` | Notities die "woord" bevatten uitsluiten |

### Zoeken op labels

| Syntax | Beschrijving |
|--------|--------------|
| \`#labelnaam\` | Notities met dit label |
| \`#label1 #label2\` | Notities met beide labels (EN) |
| \`#label1 \\| #label2\` | Notities met een van beide labels (OF) |

### Geavanceerde zoekmodificatoren

| Modificator | Beschrijving | Voorbeeld |
|-------------|--------------|-----------|
| \`has:attachment\` | Notities met bijlagen | \`has:attachment\` |
| \`created:>DATUM\` | Aangemaakt na datum | \`created:>2024-01-01\` |
| \`created:<DATUM\` | Aangemaakt voor datum | \`created:<2024-06-30\` |
| \`created:DATUM..DATUM\` | Aangemaakt in datumbereik | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATUM\` | Gewijzigd na datum | \`modified:>2024-01-01\` |
| \`modified:<DATUM\` | Gewijzigd voor datum | \`modified:<2024-06-30\` |
| \`words:>N\` | Meer dan N woorden | \`words:>100\` |
| \`words:<N\` | Minder dan N woorden | \`words:<50\` |
| \`words:N..M\` | Woordenaantal in bereik | \`words:50..200\` |

**Modificatoren combineren:** \`#project has:attachment modified:>2024-01-01 words:>100\`

---

## Meervoudige selectie en bulkbewerkingen

Selecteer meerdere notities om bulkacties uit te voeren.

### Notities selecteren

| Actie | Hoe te doen |
|-------|-------------|
| **Selectie in-/uitschakelen** | \`Ctrl/Cmd + Klik\` op een notitie |
| **Bereikselectie** | \`Shift + Klik\` om te selecteren vanaf de laatst geselecteerde |
| **Alle zichtbare selecteren** | Klik op "Alles selecteren" in de werkbalk |
| **Selectie wissen** | Druk op \`Escape\` of klik op "Annuleren" |

### Bulkacties

Wanneer notities zijn geselecteerd, verschijnt er een werkbalk onderaan met deze opties:

- **Labels toevoegen** - Voeg labels toe aan alle geselecteerde notities
- **Labels verwijderen** - Verwijder specifieke labels van geselecteerde notities
- **Exporteren** - Exporteer geselecteerde notities als JSON
- **Samenvoegen** - Voeg geselecteerde notities samen tot één (gesorteerd op aanmaakdatum)
- **Verwijderen** - Verplaats geselecteerde notities naar de prullenbak

---

## Versiegeschiedenis

Jottery maakt automatisch versiemomentopnamen bij het synchroniseren van notities.

| Actie | Hoe te doen |
|-------|-------------|
| **Geschiedenis openen** | Klik op ⋮ menu → "Versiegeschiedenis" of druk op \`Alt+H\` |
| **Versie bekijken** | Klik op een versie om de inhoud te bekijken |
| **Vergelijken** | Verschillen worden automatisch gemarkeerd |
| **Herstellen** | Klik op "Herstellen" om terug te keren naar een eerdere versie |

---

## Sneltoetsen

Alle sneltoetsen zijn aanpasbaar via Instellingen → Sneltoetsen.

### Standaardsneltoetsen

| Sneltoets | Actie |
|-----------|-------|
| \`Ctrl/Cmd + K\` | Zoeken focussen |
| \`Alt + N\` | Nieuwe notitie maken |
| \`Ctrl/Cmd + Z\` | Ongedaan maken |
| \`Ctrl/Cmd + Shift + Z\` | Opnieuw uitvoeren |
| \`Alt + H\` | Versiegeschiedenis |
| \`Alt + I\` | Notitie-informatie |
| \`Escape\` | Notitie sluiten / Selectie wissen |
| \`Ctrl/Cmd + ,\` | Instellingen openen |

### Sneltoetsen voor meervoudige selectie

| Sneltoets | Actie |
|-----------|-------|
| \`Ctrl/Cmd + Klik\` | Notitieselectie in-/uitschakelen |
| \`Shift + Klik\` | Bereikselectie |
| \`Ctrl/Cmd + A\` | Alle gefilterde notities selecteren |

---

## Synchronisatie

Jottery ondersteunt zelf-gehoste synchronisatie tussen apparaten.

### Instellen

1. Ga naar **Instellingen → Synchronisatie**
2. Voer de URL van uw zelf-gehoste server in
3. **Eerste apparaat:** Klik op "Apparaat registreren" om synchronisatiegegevens aan te maken
4. **Andere apparaten:** Gebruik "Bestaande gegevens gebruiken" met uw synchronisatiegegevens

> **Belangrijk:** Alle apparaten moeten hetzelfde **wachtwoord** gebruiken om notities te ontsleutelen. Het wachtwoord wordt nooit naar de server verzonden.

### Hoe het werkt

- Notities worden versleuteld **voordat** ze uw apparaat verlaten
- De server slaat alleen versleutelde gegevens op
- Synchronisatie gebeurt automatisch wanneer u online bent
- Conflicten worden opgelost met "laatst-schrijft-wint"

---

## Beveiliging en privacy

| Functie | Beschrijving |
|---------|--------------|
| **Encryptie** | AES-256-GCM voor alle notitie-inhoud en labels |
| **Lokale encryptie** | Alle encryptie gebeurt in uw browser |
| **Wachtwoord** | Wordt nooit opgeslagen of verzonden |
| **Automatisch vergrendelen** | Beschermt notities bij inactiviteit (standaard: 15 minuten) |
| **Geen tracking** | Geen analytics of scripts van derden |
| **Open source** | Volledige broncode beschikbaar op GitHub |

> **Tip:** Gebruik een wachtwoordbeheerder om een sterk, uniek wachtwoord voor Jottery te genereren en op te slaan. Aangezien er geen wachtwoordherstel is, betekent het verliezen van uw wachtwoord permanent verlies van toegang tot uw notities.

### Uw wachtwoord wijzigen

Aangezien uw wachtwoord de encryptiesleutel is, is er geen directe manier om het te wijzigen. U kunt echter effectief uw wachtwoord wijzigen door:

1. Al uw notities te **exporteren** (Instellingen → Importeren/Exporteren → Exporteren)
2. Uw lokale gegevens te **wissen** of een nieuwe browser/apparaat te gebruiken
3. Jottery **in te stellen** met uw nieuwe wachtwoord
4. Uw geëxporteerde notities te **importeren**

Uw notities worden dan opnieuw versleuteld met het nieuwe wachtwoord.

---

## Importeren en exporteren

### Exporteren

1. Ga naar **Instellingen → Importeren/Exporteren**
2. Klik op "Alle notities exporteren"
3. Kies een locatie om het JSON-bestand op te slaan

> **Waarschuwing:** Exports zijn **niet versleuteld**. Bewaar ze veilig!

### Importeren

1. Ga naar **Instellingen → Importeren/Exporteren**
2. Klik op "Notities importeren"
3. Selecteer een eerder geëxporteerd JSON-bestand
4. Notities worden samengevoegd met bestaande gegevens (duplicaten worden overgeslagen)

### Bulkexport

Selecteer meerdere notities en klik op "Exporteren" om alleen de geselecteerde notities te exporteren.
`;
