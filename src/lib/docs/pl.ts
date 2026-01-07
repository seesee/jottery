// Polish documentation
export const documentation = `# Dokumentacja Jottery

## Spis treści

- [Pierwsze kroki](#pierwsze-kroki)
- [Tworzenie i edycja notatek](#tworzenie-i-edycja-notatek)
- [Podświetlanie składni](#podswietlanie-skladni)
- [Tryb kalkulatora](#tryb-kalkulatora)
- [Wyszukiwanie](#wyszukiwanie)
  - [Wyszukiwanie podstawowe](#wyszukiwanie-podstawowe)
  - [Wyszukiwanie po tagach](#wyszukiwanie-po-tagach)
  - [Zaawansowane modyfikatory wyszukiwania](#zaawansowane-modyfikatory-wyszukiwania)
- [Wielokrotne zaznaczanie i operacje zbiorcze](#wielokrotne-zaznaczanie-i-operacje-zbiorcze)
- [Historia wersji](#historia-wersji)
- [Skróty klawiszowe](#skroty-klawiszowe)
- [Synchronizacja](#synchronizacja)
- [Bezpieczeństwo i prywatność](#bezpieczenstwo-i-prywatnosc)
- [Import i eksport](#import-i-eksport)

---

## Pierwsze kroki

Jottery to aplikacja do tworzenia notatek skupiona na prywatności i szyfrowaniu. Wszystkie Państwa notatki są szyfrowane lokalnie przy użyciu szyfrowania **AES-256-GCM** przed zapisaniem.

> **Ważne:** Państwa hasło jest kluczem szyfrowania. Jeśli je Państwo utracą, notatek nie będzie można odzyskać. Nie ma funkcji resetowania hasła.

---

## Tworzenie i edycja notatek

| Akcja | Jak to zrobić |
|-------|---------------|
| **Utwórz notatkę** | Kliknij "+ Nowa notatka" lub naciśnij \`Alt+N\` |
| **Edytuj notatkę** | Kliknij notatkę na liście, aby ją otworzyć |
| **Automatyczny zapis** | Zmiany są automatycznie zapisywane podczas pisania |
| **Zamknij notatkę** | Naciśnij \`Escape\` lub kliknij inną notatkę |
| **Przypnij notatkę** | Kliknij ikonę pinezki, aby utrzymać ją na górze |
| **Usuń notatkę** | Kliknij menu (⋮) i wybierz "Usuń" |

---

## Podświetlanie składni

Użyj listy rozwijanej języków w edytorze, aby włączyć podświetlanie składni. Obsługiwane języki to:

- **Markdown** - z podglądem na żywo i podświetlaniem bloków kodu
- **JavaScript/TypeScript** - obsługa składni ES6+
- **Python** - w tym f-stringi i dekoratory
- **JSON, HTML, CSS, SQL**
- **Bash/Shell, Perl**
- **Calculator** - interaktywne wyrażenia matematyczne

---

## Tryb kalkulatora

Ustaw język składni na **Calc**, aby korzystać z interaktywnego kalkulatora. Każda linia jest obliczana jako wyrażenie matematyczne, a wyniki są wyświetlane w tekście.

### Funkcje

- **Podstawowa arytmetyka:** \`2 + 3 * 4\` → \`14\`
- **Zmienne:** \`x = 10\` następnie \`x * 2\` → \`20\`
- **Stałe:** \`pi\`, \`e\`, \`tau\`, \`phi\`
- **Funkcje:** \`sqrt(16)\` → \`4\`, \`sin(pi/2)\` → \`1\`
- **Potęgowanie:** \`2^10\` lub \`2**10\` → \`1024\`
- **Silnia:** \`5!\` → \`120\`
- **Komentarze:** Linie zaczynające się od \`#\` są ignorowane

### Dostępne funkcje

| Kategoria | Funkcje |
|-----------|---------|
| **Podstawowe** | \`abs\`, \`floor\`, \`ceil\`, \`round\`, \`min\`, \`max\` |
| **Potęgi** | \`sqrt\`, \`cbrt\`, \`exp\`, \`ln\`, \`log\`, \`log10\` |
| **Trygonometria** | \`sin\`, \`cos\`, \`tan\`, \`asin\`, \`acos\`, \`atan\` |
| **Hiperboliczne** | \`sinh\`, \`cosh\`, \`tanh\`, \`asinh\`, \`acosh\`, \`atanh\` |

### Przykład

\`\`\`
# Oblicz procent składany
principal = 1000
rate = 0.05
years = 10
principal * (1 + rate)^years
\`\`\`

---

## Wyszukiwanie

### Wyszukiwanie podstawowe

Wpisz w pole wyszukiwania, aby znaleźć notatki. Wyszukiwanie obejmuje zarówno zawartość notatek, jak i tagi.

| Składnia | Opis |
|----------|------|
| \`słowo\` | Notatki zawierające "słowo" |
| \`słowo1 słowo2\` | Notatki zawierające oba słowa (AND) |
| \`"dokładna fraza"\` | Notatki zawierające dokładną frazę |
| \`-słowo\` | Wyklucz notatki zawierające "słowo" |

### Wyszukiwanie po tagach

| Składnia | Opis |
|----------|------|
| \`#nazwatagu\` | Notatki z tym tagiem |
| \`#tag1 #tag2\` | Notatki z oboma tagami (AND) |
| \`#tag1 \\| #tag2\` | Notatki z którymkolwiek tagiem (OR) |

### Zaawansowane modyfikatory wyszukiwania

| Modyfikator | Opis | Przykład |
|-------------|------|----------|
| \`has:attachment\` | Notatki z załącznikami | \`has:attachment\` |
| \`created:>DATA\` | Utworzone po dacie | \`created:>2024-01-01\` |
| \`created:<DATA\` | Utworzone przed datą | \`created:<2024-06-30\` |
| \`created:DATA..DATA\` | Utworzone w zakresie dat | \`created:2024-01-01..2024-06-30\` |
| \`modified:>DATA\` | Zmodyfikowane po dacie | \`modified:>2024-01-01\` |
| \`modified:<DATA\` | Zmodyfikowane przed datą | \`modified:<2024-06-30\` |
| \`words:>N\` | Więcej niż N słów | \`words:>100\` |
| \`words:<N\` | Mniej niż N słów | \`words:<50\` |
| \`words:N..M\` | Liczba słów w zakresie | \`words:50..200\` |

**Łączenie modyfikatorów:** \`#projekt has:attachment modified:>2024-01-01 words:>100\`

---

## Wielokrotne zaznaczanie i operacje zbiorcze

Zaznacz wiele notatek, aby wykonać operacje zbiorcze.

### Zaznaczanie notatek

| Akcja | Jak to zrobić |
|-------|---------------|
| **Przełącz zaznaczenie** | \`Ctrl/Cmd + Kliknięcie\` na notatce |
| **Zaznaczenie zakresu** | \`Shift + Kliknięcie\` aby zaznaczyć od ostatnio wybranej |
| **Zaznacz wszystkie widoczne** | Kliknij "Zaznacz wszystkie" na pasku narzędzi |
| **Wyczyść zaznaczenie** | Naciśnij \`Escape\` lub kliknij "Anuluj" |

### Operacje zbiorcze

Gdy notatki są zaznaczone, na dole pojawia się pasek narzędzi z następującymi opcjami:

- **Dodaj tagi** - Dodaj tagi do wszystkich zaznaczonych notatek
- **Usuń tagi** - Usuń określone tagi z zaznaczonych notatek
- **Eksportuj** - Eksportuj zaznaczone notatki jako JSON
- **Połącz** - Scal zaznaczone notatki w jedną (uporządkowane według daty utworzenia)
- **Usuń** - Przenieś zaznaczone notatki do kosza

---

## Historia wersji

Jottery automatycznie tworzy migawki wersji podczas synchronizacji notatek.

| Akcja | Jak to zrobić |
|-------|---------------|
| **Otwórz historię** | Kliknij menu ⋮ → "Historia wersji" lub naciśnij \`Alt+H\` |
| **Wyświetl wersję** | Kliknij wersję, aby zobaczyć jej zawartość |
| **Porównaj** | Różnice są automatycznie podświetlane |
| **Przywróć** | Kliknij "Przywróć", aby powrócić do poprzedniej wersji |

---

## Skróty klawiszowe

Wszystkie skróty klawiszowe można dostosować w Ustawienia → Skróty klawiszowe.

### Domyślne skróty

| Skrót | Akcja |
|-------|-------|
| \`Ctrl/Cmd + K\` | Fokus na wyszukiwaniu |
| \`Alt + N\` | Utwórz nową notatkę |
| \`Ctrl/Cmd + Z\` | Cofnij |
| \`Ctrl/Cmd + Shift + Z\` | Ponów |
| \`Alt + H\` | Historia wersji |
| \`Alt + I\` | Informacje o notatce |
| \`Escape\` | Zamknij notatkę / Wyczyść zaznaczenie |
| \`Ctrl/Cmd + ,\` | Otwórz ustawienia |

### Skróty wielokrotnego zaznaczania

| Skrót | Akcja |
|-------|-------|
| \`Ctrl/Cmd + Kliknięcie\` | Przełącz zaznaczenie notatki |
| \`Shift + Kliknięcie\` | Zaznaczenie zakresu |
| \`Ctrl/Cmd + A\` | Zaznacz wszystkie przefiltrowane notatki |

---

## Synchronizacja

Jottery obsługuje samodzielnie hostowaną synchronizację między urządzeniami.

### Konfiguracja

1. Przejdź do **Ustawienia → Synchronizacja**
2. Wprowadź adres URL swojego samodzielnie hostowanego serwera
3. **Pierwsze urządzenie:** Kliknij "Zarejestruj urządzenie", aby utworzyć dane uwierzytelniające synchronizacji
4. **Inne urządzenia:** Użyj "Użyj istniejących danych uwierzytelniających" ze swoimi danymi synchronizacji

> **Ważne:** Wszystkie urządzenia muszą używać **tego samego hasła** do odszyfrowywania notatek. Hasło nigdy nie jest wysyłane na serwer.

### Jak to działa

- Notatki są szyfrowane **przed** opuszczeniem Państwa urządzenia
- Serwer przechowuje tylko zaszyfrowane dane
- Synchronizacja odbywa się automatycznie, gdy urządzenie jest online
- Konflikty są rozwiązywane metodą "ostatni zapis wygrywa"

---

## Bezpieczeństwo i prywatność

| Funkcja | Opis |
|---------|------|
| **Szyfrowanie** | AES-256-GCM dla całej zawartości notatek i tagów |
| **Lokalne szyfrowanie** | Całe szyfrowanie odbywa się w przeglądarce |
| **Hasło** | Nigdy nie jest przechowywane ani przesyłane |
| **Automatyczna blokada** | Chroni notatki podczas bezczynności (domyślnie: 15 minut) |
| **Brak śledzenia** | Zero analityki lub skryptów firm trzecich |
| **Otwarte źródło** | Pełny kod źródłowy dostępny na GitHub |

> **Wskazówka:** Użyj menedżera haseł do wygenerowania i przechowywania silnego, unikalnego hasła dla Jottery. Ponieważ nie ma możliwości odzyskania hasła, jego utrata oznacza trwałą utratę dostępu do notatek.

### Zmiana hasła

Ponieważ Państwa hasło jest kluczem szyfrowania, nie ma bezpośredniego sposobu na jego zmianę. Można jednak skutecznie zmienić hasło poprzez:

1. **Eksport** wszystkich notatek (Ustawienia → Import/Eksport → Eksportuj)
2. **Wyczyszczenie** lokalnych danych lub użycie nowej przeglądarki/urządzenia
3. **Skonfigurowanie** Jottery z nowym hasłem
4. **Import** wyeksportowanych notatek

Państwa notatki zostaną ponownie zaszyfrowane nowym hasłem.

---

## Import i eksport

### Eksport

1. Przejdź do **Ustawienia → Import/Eksport**
2. Kliknij "Eksportuj wszystkie notatki"
3. Wybierz lokalizację do zapisania pliku JSON

> **Ostrzeżenie:** Eksporty są **niezaszyfrowane**. Przechowuj je bezpiecznie!

### Import

1. Przejdź do **Ustawienia → Import/Eksport**
2. Kliknij "Importuj notatki"
3. Wybierz wcześniej wyeksportowany plik JSON
4. Notatki zostaną scalone z istniejącymi danymi (duplikaty są pomijane)

### Eksport zbiorczy

Zaznacz wiele notatek i kliknij "Eksportuj", aby wyeksportować tylko wybrane notatki.
`;
