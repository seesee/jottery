// Italian documentation
export const documentation = `# Documentazione di Jottery

## Indice

- [Per Iniziare](#per-iniziare)
- [Creazione e Modifica delle Note](#creazione-e-modifica-delle-note)
- [Evidenziazione della Sintassi](#evidenziazione-della-sintassi)
- [Modalita Calcolatrice](#modalita-calcolatrice)
- [Ricerca](#ricerca)
  - [Ricerca Base](#ricerca-base)
  - [Ricerca per Tag](#ricerca-per-tag)
  - [Modificatori di Ricerca Avanzati](#modificatori-di-ricerca-avanzati)
- [Selezione Multipla e Operazioni in Blocco](#selezione-multipla-e-operazioni-in-blocco)
- [Cronologia delle Versioni](#cronologia-delle-versioni)
- [Scorciatoie da Tastiera](#scorciatoie-da-tastiera)
- [Sincronizzazione](#sincronizzazione)
- [Sicurezza e Privacy](#sicurezza-e-privacy)
- [Importazione ed Esportazione](#importazione-ed-esportazione)

---

## Per Iniziare

Jottery e un'applicazione per appunti crittografata e incentrata sulla privacy. Tutte le Sue note vengono crittografate localmente utilizzando la crittografia **AES-256-GCM** prima di essere memorizzate.

> **Importante:** La Sua password e la chiave di crittografia. Se la perde, le Sue note non potranno essere recuperate. Non esiste alcuna funzionalita di reimpostazione della password.

---

## Creazione e Modifica delle Note

| Azione | Come fare |
|--------|-----------|
| **Creare una nota** | Cliccare su "+ Nuova Nota" o premere \`Alt+N\` |
| **Modificare una nota** | Cliccare su una nota nell'elenco per aprirla |
| **Salvataggio automatico** | Le modifiche vengono salvate automaticamente durante la digitazione |
| **Chiudere una nota** | Premere \`Escape\` o cliccare su un'altra nota |
| **Fissare una nota** | Cliccare sull'icona della puntina per mantenerla in cima |
| **Eliminare una nota** | Cliccare sul menu (⋮) e selezionare "Elimina" |

---

## Evidenziazione della Sintassi

Utilizzare il menu a discesa della lingua nell'editor per abilitare l'evidenziazione della sintassi. I linguaggi supportati includono:

- **Markdown** - con anteprima in tempo reale ed evidenziazione dei blocchi di codice
- **JavaScript/TypeScript** - supporto per sintassi ES6+
- **Python** - inclusi f-string e decoratori
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - espressioni matematiche interattive

---

## Modalita Calcolatrice

Impostare il linguaggio di sintassi su **Calc** per utilizzare la calcolatrice interattiva. Ogni riga viene valutata come un'espressione matematica, con i risultati mostrati in linea.

### Funzionalita

- **Aritmetica base:** \`2 + 3 * 4\` → \`14\`
- **Variabili:** \`x = 10\` poi \`x * 2\` → \`20\`
- **Costanti:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Funzioni:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Potenza:** \`2^10\` o \`2**10\` → \`1024\`
- **Fattoriale:** \`5!\` → \`120\`
- **Commenti:** Le righe che iniziano con \`#\` vengono ignorate

### Funzioni Disponibili

| Categoria | Funzioni |
|-----------|----------|
| **Base** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Potenze** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonometria** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Iperboliche** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Esempio

\`\`\`
# Calcolare l'interesse composto
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Ricerca

### Ricerca Base

Digitare nella casella di ricerca per trovare le note. La ricerca esamina sia il contenuto delle note che i tag.

| Sintassi | Descrizione |
|----------|-------------|
| \`parola\` | Note contenenti "parola" |
| \`parola1 parola2\` | Note contenenti entrambe le parole (AND) |
| \`"frase esatta"\` | Note contenenti la frase esatta |
| \`-parola\` | Escludere le note contenenti "parola" |

### Ricerca per Tag

| Sintassi | Descrizione |
|----------|-------------|
| \`#nometag\` | Note con questo tag |
| \`#tag1 #tag2\` | Note con entrambi i tag (AND) |
| \`#tag1 \\| #tag2\` | Note con uno dei due tag (OR) |

### Modificatori di Ricerca Avanzati

| Modificatore | Descrizione | Esempio |
|--------------|-------------|---------|
| \`has:attachment\` | Note con allegati | \`has:attachment\` |
| \`created:>DATA\` | Create dopo la data | \`created:>2024-01-01\` |
| \`created:<DATA\` | Create prima della data | \`created:<2024-06-30\` |
| \`created:DATA..DATA\` | Create nell'intervallo di date | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATA\` | Modificate dopo la data | \`modified:>2024-01-01\` |
| \`modified:<DATA\` | Modificate prima della data | \`modified:<2024-06-30\` |
| \`words:>N\` | Piu di N parole | \`words:>100\` |
| \`words:<N\` | Meno di N parole | \`words:<50\` |
| \`words:N..M\` | Conteggio parole nell'intervallo | \`words:50..200\` |

**Combinazione di modificatori:** \`#progetto has:attachment modified:>2024-01-01 words:>100\`

---

## Selezione Multipla e Operazioni in Blocco

Selezionare piu note per eseguire azioni in blocco.

### Selezione delle Note

| Azione | Come fare |
|--------|-----------|
| **Attivare/disattivare selezione** | \`Ctrl/Cmd + Click\` su una nota |
| **Selezione intervallo** | \`Shift + Click\` per selezionare dall'ultima selezionata |
| **Selezionare tutte le visibili** | Cliccare su "Seleziona Tutto" nella barra degli strumenti |
| **Annullare selezione** | Premere \`Escape\` o cliccare su "Annulla" |

### Azioni in Blocco

Quando le note sono selezionate, appare una barra degli strumenti in basso con queste opzioni:

- **Aggiungi Tag** - Aggiungere tag a tutte le note selezionate
- **Rimuovi Tag** - Rimuovere tag specifici dalle note selezionate
- **Esporta** - Esportare le note selezionate come JSON
- **Combina** - Unire le note selezionate in una sola (ordinate per data di creazione)
- **Elimina** - Spostare le note selezionate nel cestino

---

## Cronologia delle Versioni

Jottery crea automaticamente istantanee delle versioni durante la sincronizzazione delle note.

| Azione | Come fare |
|--------|-----------|
| **Aprire la cronologia** | Cliccare sul menu ⋮ → "Cronologia Versioni" o premere \`Alt+H\` |
| **Visualizzare versione** | Cliccare su una versione per vederne il contenuto |
| **Confrontare** | Le differenze vengono evidenziate automaticamente |
| **Ripristinare** | Cliccare su "Ripristina" per tornare a una versione precedente |

---

## Scorciatoie da Tastiera

Tutte le scorciatoie da tastiera sono personalizzabili in Impostazioni → Scorciatoie da Tastiera.

### Scorciatoie Predefinite

| Scorciatoia | Azione |
|-------------|--------|
| \`Ctrl/Cmd + K\` | Attivare la ricerca |
| \`Alt + N\` | Creare nuova nota |
| \`Ctrl/Cmd + Z\` | Annulla |
| \`Ctrl/Cmd + Shift + Z\` | Ripristina |
| \`Alt + H\` | Cronologia versioni |
| \`Alt + I\` | Informazioni nota |
| \`Escape\` | Chiudere nota / Annullare selezione |
| \`Ctrl/Cmd + ,\` | Aprire impostazioni |

### Scorciatoie per Selezione Multipla

| Scorciatoia | Azione |
|-------------|--------|
| \`Ctrl/Cmd + Click\` | Attivare/disattivare selezione nota |
| \`Shift + Click\` | Selezione intervallo |
| \`Ctrl/Cmd + A\` | Selezionare tutte le note filtrate |

---

## Sincronizzazione

Jottery supporta la sincronizzazione self-hosted tra dispositivi.

### Configurazione

1. Andare su **Impostazioni → Sincronizzazione**
2. Inserire l'URL del proprio server self-hosted
3. **Primo dispositivo:** Cliccare su "Registra Dispositivo" per creare le credenziali di sincronizzazione
4. **Altri dispositivi:** Usare "Usa Credenziali Esistenti" con le proprie credenziali di sincronizzazione

> **Importante:** Tutti i dispositivi devono utilizzare la **stessa password** per decrittare le note. La password non viene mai inviata al server.

### Come Funziona

- Le note vengono crittografate **prima** di lasciare il dispositivo
- Il server memorizza solo dati crittografati
- La sincronizzazione avviene automaticamente quando si e online
- I conflitti vengono risolti con il criterio dell'ultima scrittura

---

## Sicurezza e Privacy

| Caratteristica | Descrizione |
|----------------|-------------|
| **Crittografia** | AES-256-GCM per tutto il contenuto delle note e i tag |
| **Crittografia locale** | Tutta la crittografia avviene nel browser |
| **Password** | Mai memorizzata o trasmessa |
| **Blocco automatico** | Protegge le note quando inattivo (predefinito: 15 minuti) |
| **Nessun tracciamento** | Zero analytics o script di terze parti |
| **Open source** | Codice sorgente completo disponibile su GitHub |

> **Suggerimento:** Utilizzare un gestore di password per generare e memorizzare una password forte e unica per Jottery. Poiche non esiste il recupero della password, perderla significa perdere l'accesso alle proprie note permanentemente.

### Modifica della Password

Poiche la password e la chiave di crittografia, non esiste un modo diretto per cambiarla. Tuttavia, e possibile cambiare effettivamente la password:

1. **Esportare** tutte le note (Impostazioni → Importa/Esporta → Esporta)
2. **Cancellare** i dati locali o utilizzare un nuovo browser/dispositivo
3. **Configurare** Jottery con la nuova password
4. **Importare** le note esportate

Le note verranno ricrittografate con la nuova password.

---

## Importazione ed Esportazione

### Esportazione

1. Andare su **Impostazioni → Importa/Esporta**
2. Cliccare su "Esporta Tutte le Note"
3. Scegliere una posizione per salvare il file JSON

> **Attenzione:** Le esportazioni sono **non crittografate**. Conservarle in modo sicuro!

### Importazione

1. Andare su **Impostazioni → Importa/Esporta**
2. Cliccare su "Importa Note"
3. Selezionare un file JSON precedentemente esportato
4. Le note verranno unite ai dati esistenti (i duplicati vengono saltati)

### Esportazione in Blocco

Selezionare piu note e cliccare su "Esporta" per esportare solo le note selezionate.
`;
