// French documentation
export const documentation = `# Documentation Jottery

## Table des matières

- [Premiers pas](#premiers-pas)
- [Créer et modifier des notes](#creer-modifier-notes)
- [Coloration syntaxique](#coloration-syntaxique)
- [Mode calculatrice](#mode-calculatrice)
- [Recherche](#recherche)
  - [Recherche simple](#recherche-simple)
  - [Recherche par étiquettes](#recherche-par-etiquettes)
  - [Modificateurs de recherche avancés](#modificateurs-recherche-avances)
- [Sélection multiple et opérations groupées](#selection-multiple-operations-groupees)
- [Historique des versions](#historique-versions)
- [Raccourcis clavier](#raccourcis-clavier)
- [Synchronisation](#synchronisation)
- [Sécurité et confidentialité](#securite-confidentialite)
- [Importation et exportation](#importation-exportation)

---

## Premiers pas

Jottery est une application de prise de notes chiffrée axée sur la confidentialité. Toutes vos notes sont chiffrées localement avec le chiffrement **AES-256-GCM** avant d'être stockées.

> **Important :** Votre mot de passe est la clé de chiffrement. Si vous le perdez, vos notes ne pourront pas être récupérées. Il n'existe aucune fonctionnalité de réinitialisation de mot de passe.

---

## Créer et modifier des notes

| Action | Comment faire |
|--------|---------------|
| **Créer une note** | Cliquez sur "+ Nouvelle note" ou appuyez sur \`Alt+N\` |
| **Modifier une note** | Cliquez sur une note dans la liste pour l'ouvrir |
| **Sauvegarde automatique** | Les modifications sont automatiquement enregistrées pendant la saisie |
| **Fermer une note** | Appuyez sur \`Escape\` ou cliquez sur une autre note |
| **Épingler une note** | Cliquez sur l'icône d'épingle pour la maintenir en haut |
| **Supprimer une note** | Cliquez sur le menu (⋮) et sélectionnez "Supprimer" |

---

## Coloration syntaxique

Utilisez le menu déroulant de langue dans l'éditeur pour activer la coloration syntaxique. Les langages pris en charge incluent :

- **Markdown** - avec aperçu en direct et coloration des blocs de code
- **JavaScript/TypeScript** - prise en charge de la syntaxe ES6+
- **Python** - y compris les f-strings et les décorateurs
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - expressions mathématiques interactives

---

## Mode calculatrice

Définissez le langage syntaxique sur **Calc** pour utiliser la calculatrice interactive. Chaque ligne est évaluée comme une expression mathématique, avec les résultats affichés en ligne.

### Fonctionnalités

- **Arithmétique de base :** \`2 + 3 * 4\` → \`14\`
- **Variables :** \`x = 10\` puis \`x * 2\` → \`20\`
- **Constantes :** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Fonctions :** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Puissance :** \`2^10\` ou \`2**10\` → \`1024\`
- **Factorielle :** \`5!\` → \`120\`
- **Commentaires :** Les lignes commençant par \`#\` sont ignorées

### Fonctions disponibles

| Catégorie | Fonctions |
|-----------|-----------|
| **Base** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Puissances** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trigonométrie** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hyperbolique** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Exemple

\`\`\`
# Calculer les intérêts composés
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Recherche

### Recherche simple

Saisissez dans la zone de recherche pour trouver des notes. La recherche examine à la fois le contenu des notes et les étiquettes.

| Syntaxe | Description |
|---------|-------------|
| \`mot\` | Notes contenant "mot" |
| \`mot1 mot2\` | Notes contenant les deux mots (ET) |
| \`"phrase exacte"\` | Notes contenant la phrase exacte |
| \`-mot\` | Exclure les notes contenant "mot" |

### Recherche par étiquettes

| Syntaxe | Description |
|---------|-------------|
| \`#etiquette\` | Notes avec cette étiquette |
| \`#etiq1 #etiq2\` | Notes avec les deux étiquettes (ET) |
| \`#etiq1 \\| #etiq2\` | Notes avec l'une ou l'autre étiquette (OU) |

### Modificateurs de recherche avancés

| Modificateur | Description | Exemple |
|--------------|-------------|---------|
| \`has:attachment\` | Notes avec pièces jointes | \`has:attachment\` |
| \`created:>DATE\` | Créé après la date | \`created:>2024-01-01\` |
| \`created:<DATE\` | Créé avant la date | \`created:<2024-06-30\` |
| \`created:DATE..DATE\` | Créé dans la plage de dates | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATE\` | Modifié après la date | \`modified:>2024-01-01\` |
| \`modified:<DATE\` | Modifié avant la date | \`modified:<2024-06-30\` |
| \`words:>N\` | Plus de N mots | \`words:>100\` |
| \`words:<N\` | Moins de N mots | \`words:<50\` |
| \`words:N..M\` | Nombre de mots dans la plage | \`words:50..200\` |

**Combinaison de modificateurs :** \`#projet has:attachment modified:>2024-01-01 words:>100\`

---

## Sélection multiple et opérations groupées

Sélectionnez plusieurs notes pour effectuer des actions groupées.

### Sélectionner des notes

| Action | Comment faire |
|--------|---------------|
| **Basculer la sélection** | \`Ctrl/Cmd + Clic\` sur une note |
| **Sélection par plage** | \`Shift + Clic\` pour sélectionner depuis la dernière sélection |
| **Sélectionner tout ce qui est visible** | Cliquez sur "Tout sélectionner" dans la barre d'outils |
| **Effacer la sélection** | Appuyez sur \`Escape\` ou cliquez sur "Annuler" |

### Actions groupées

Lorsque des notes sont sélectionnées, une barre d'outils apparaît en bas avec ces options :

- **Ajouter des étiquettes** - Ajouter des étiquettes à toutes les notes sélectionnées
- **Supprimer des étiquettes** - Supprimer des étiquettes spécifiques des notes sélectionnées
- **Exporter** - Exporter les notes sélectionnées au format JSON
- **Combiner** - Fusionner les notes sélectionnées en une seule (ordonnées par date de création)
- **Supprimer** - Déplacer les notes sélectionnées vers la corbeille

---

## Historique des versions

Jottery crée automatiquement des instantanés de version lors de la synchronisation des notes.

| Action | Comment faire |
|--------|---------------|
| **Ouvrir l'historique** | Cliquez sur le menu ⋮ → "Historique des versions" ou appuyez sur \`Alt+H\` |
| **Voir une version** | Cliquez sur une version pour voir son contenu |
| **Comparer** | Les différences sont automatiquement mises en évidence |
| **Restaurer** | Cliquez sur "Restaurer" pour revenir à une version précédente |

---

## Raccourcis clavier

Tous les raccourcis clavier sont personnalisables dans Paramètres → Raccourcis clavier.

### Raccourcis par défaut

| Raccourci | Action |
|-----------|--------|
| \`Ctrl/Cmd + K\` | Activer la recherche |
| \`Alt + N\` | Créer une nouvelle note |
| \`Ctrl/Cmd + Z\` | Annuler |
| \`Ctrl/Cmd + Shift + Z\` | Rétablir |
| \`Alt + H\` | Historique des versions |
| \`Alt + I\` | Informations sur la note |
| \`Escape\` | Fermer la note / Effacer la sélection |
| \`Ctrl/Cmd + ,\` | Ouvrir les paramètres |

### Raccourcis de sélection multiple

| Raccourci | Action |
|-----------|--------|
| \`Ctrl/Cmd + Clic\` | Basculer la sélection de la note |
| \`Shift + Clic\` | Sélection par plage |
| \`Ctrl/Cmd + A\` | Sélectionner toutes les notes filtrées |

---

## Synchronisation

Jottery prend en charge la synchronisation auto-hébergée entre appareils.

### Configuration

1. Allez dans **Paramètres → Synchronisation**
2. Entrez l'URL de votre serveur auto-hébergé
3. **Premier appareil :** Cliquez sur "Enregistrer l'appareil" pour créer les identifiants de synchronisation
4. **Autres appareils :** Utilisez "Utiliser des identifiants existants" avec vos identifiants de synchronisation

> **Important :** Tous les appareils doivent utiliser le **même mot de passe** pour déchiffrer les notes. Le mot de passe n'est jamais envoyé au serveur.

### Fonctionnement

- Les notes sont chiffrées **avant** de quitter votre appareil
- Le serveur ne stocke que des données chiffrées
- La synchronisation s'effectue automatiquement lorsque vous êtes en ligne
- Les conflits sont résolus selon le principe du dernier écrit gagne

---

## Sécurité et confidentialité

| Fonctionnalité | Description |
|----------------|-------------|
| **Chiffrement** | AES-256-GCM pour tout le contenu des notes et les étiquettes |
| **Chiffrement local** | Tout le chiffrement s'effectue dans votre navigateur |
| **Mot de passe** | Jamais stocké ni transmis |
| **Verrouillage automatique** | Protège les notes en cas d'inactivité (par défaut : 15 minutes) |
| **Aucun suivi** | Aucune analyse ni script tiers |
| **Open source** | Code source complet disponible sur GitHub |

> **Conseil :** Utilisez un gestionnaire de mots de passe pour générer et stocker un mot de passe fort et unique pour Jottery. Comme il n'y a pas de récupération de mot de passe, perdre votre mot de passe signifie perdre définitivement l'accès à vos notes.

### Changer votre mot de passe

Comme votre mot de passe est la clé de chiffrement, il n'y a pas de moyen direct de le changer. Cependant, vous pouvez effectivement changer votre mot de passe en :

1. **Exportant** toutes vos notes (Paramètres → Importation/Exportation → Exporter)
2. **Effaçant** vos données locales ou en utilisant un nouveau navigateur/appareil
3. **Configurant** Jottery avec votre nouveau mot de passe
4. **Important** vos notes exportées

Vos notes seront rechiffrées avec le nouveau mot de passe.

---

## Importation et exportation

### Exporter

1. Allez dans **Paramètres → Importation/Exportation**
2. Cliquez sur "Exporter toutes les notes"
3. Choisissez un emplacement pour enregistrer le fichier JSON

> **Attention :** Les exportations sont **non chiffrées**. Stockez-les en toute sécurité !

### Importer

1. Allez dans **Paramètres → Importation/Exportation**
2. Cliquez sur "Importer des notes"
3. Sélectionnez un fichier JSON précédemment exporté
4. Les notes seront fusionnées avec les données existantes (les doublons sont ignorés)

### Exportation groupée

Sélectionnez plusieurs notes et cliquez sur "Exporter" pour exporter uniquement les notes sélectionnées.
`;
