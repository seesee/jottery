#!/usr/bin/env bash
#
# Generate TUI demo GIFs for multi-language screenshots
#
# This script runs VHS tapes with language-specific demo databases.
#
# Usage:
#   ./generate-tui-demo.sh [language] [tape]
#
# Examples:
#   ./generate-tui-demo.sh en-GB                    # All tapes for English
#   ./generate-tui-demo.sh en-GB tui-interface      # Specific tape
#   ./generate-tui-demo.sh                          # All languages, all tapes

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DEMO_DIR="$SCRIPT_DIR/tui-demo"
VHS_DIR="$SCRIPT_DIR/vhs"
OUTPUT_DIR="$PROJECT_ROOT/public/screenshots"
TUI_BIN_DIR="$PROJECT_ROOT/tui/target/release"

# Add jottery binary to PATH
export PATH="$TUI_BIN_DIR:$PATH"

# All supported languages
ALL_LANGUAGES=(
    "en-GB" "en-US" "de" "fr" "es" "pt" "it" "nl" "pl" "ru" "tr" "el" "ja" "ko" "zh"
)

# Available tapes
ALL_TAPES=(
    "tui-interface"
    "tui-cli"
    "tui-piping"
    "tui-sync"
)

# Colours for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if VHS is installed
check_vhs() {
    if ! command -v vhs &> /dev/null; then
        log_error "VHS is not installed. Install with: brew install vhs"
        exit 1
    fi
}

# Check if jottery binary exists
check_jottery() {
    if [[ ! -x "$TUI_BIN_DIR/jottery" ]]; then
        log_error "jottery binary not found at: $TUI_BIN_DIR/jottery"
        log_info "Build it with: cd $PROJECT_ROOT/tui && cargo build --release"
        exit 1
    fi
    log_info "Using jottery: $TUI_BIN_DIR/jottery"
}

# Map language code to LANG environment variable
get_locale_for_lang() {
    local lang="$1"
    case "$lang" in
        en-GB) echo "en_GB.UTF-8" ;;
        en-US) echo "en_US.UTF-8" ;;
        de)    echo "de_DE.UTF-8" ;;
        fr)    echo "fr_FR.UTF-8" ;;
        es)    echo "es_ES.UTF-8" ;;
        pt)    echo "pt_PT.UTF-8" ;;
        it)    echo "it_IT.UTF-8" ;;
        nl)    echo "nl_NL.UTF-8" ;;
        pl)    echo "pl_PL.UTF-8" ;;
        ru)    echo "ru_RU.UTF-8" ;;
        tr)    echo "tr_TR.UTF-8" ;;
        el)    echo "el_GR.UTF-8" ;;
        ja)    echo "ja_JP.UTF-8" ;;
        ko)    echo "ko_KR.UTF-8" ;;
        zh)    echo "zh_CN.UTF-8" ;;
        *)     echo "en_GB.UTF-8" ;;
    esac
}

# Get search term for each language (matches travel note content)
get_search_term() {
    local lang="$1"
    case "$lang" in
        en-GB) echo "lake district" ;;
        en-US) echo "yosemite" ;;
        de)    echo "schwarzwald" ;;
        fr)    echo "provence" ;;
        es)    echo "andalucia" ;;
        pt)    echo "algarve" ;;
        it)    echo "toscana" ;;
        nl)    echo "amsterdam" ;;
        pl)    echo "krakow" ;;
        ru)    echo "baikal" ;;
        tr)    echo "kapadokya" ;;
        el)    echo "santorini" ;;
        ja)    echo "kyoto" ;;
        ko)    echo "jeju" ;;
        zh)    echo "guilin" ;;
        *)     echo "travel" ;;
    esac
}

# Get note ID prefix for show command (travel note ID prefix)
get_note_id() {
    local lang="$1"
    # All demo notes use format: {lang}-d4e5f6a7-... for the travel note
    # Use short prefix that jottery's partial match will find
    case "$lang" in
        en-GB) echo "en-GB-d4" ;;
        en-US) echo "en-US-d4" ;;
        *)     echo "${lang}-d4" ;;
    esac
}

# Get sample note text for each language
get_sample_text() {
    local lang="$1"
    case "$lang" in
        en-GB) echo "Hello from the editor!" ;;
        en-US) echo "Hello from the editor!" ;;
        de)    echo "Hallo vom Editor!" ;;
        fr)    echo "Bonjour depuis l'editeur!" ;;
        es)    echo "Hola desde el editor!" ;;
        pt)    echo "Ola do editor!" ;;
        it)    echo "Ciao dall'editor!" ;;
        nl)    echo "Hallo vanuit de editor!" ;;
        pl)    echo "Witaj z edytora!" ;;
        ru)    echo "Privet iz redaktora!" ;;
        tr)    echo "Editordan merhaba!" ;;
        el)    echo "Geia apo ton editor!" ;;
        ja)    echo "edita kara konnichiwa!" ;;
        ko)    echo "editeoe-seo annyeong!" ;;
        zh)    echo "Lai zi bianji qi de wenhao!" ;;
        *)     echo "Hello from the editor!" ;;
    esac
}

# Get quick note text for piping demo
get_quick_note() {
    local lang="$1"
    case "$lang" in
        en-GB) echo "Quick reminder - Update documentation" ;;
        en-US) echo "Quick reminder - Update documentation" ;;
        de)    echo "Kurze Erinnerung - Dokumentation aktualisieren" ;;
        fr)    echo "Rappel rapide - Mettre a jour la documentation" ;;
        es)    echo "Recordatorio rapido - Actualizar documentacion" ;;
        pt)    echo "Lembrete rapido - Atualizar documentacao" ;;
        it)    echo "Promemoria veloce - Aggiornare documentazione" ;;
        nl)    echo "Snelle herinnering - Documentatie bijwerken" ;;
        pl)    echo "Szybkie przypomnienie - Zaktualizuj dokumentacje" ;;
        ru)    echo "Napominanie - Obnovit dokumentaciyu" ;;
        tr)    echo "Hizli hatirlatma - Dokumantasyonu guncelle" ;;
        el)    echo "Aplo ypomnimatario - Enimerosi tekmiriosis" ;;
        ja)    echo "Memo - Dokyumento wo koushin" ;;
        ko)    echo "Memo - Munseo eobdeiteu" ;;
        zh)    echo "Kuaisu beiwang - Gengxin wendang" ;;
        *)     echo "Quick reminder - Update documentation" ;;
    esac
}

# Get sync demo comments for each language
get_sync_help_comment() {
    local lang="$1"
    case "$lang" in
        de)    echo "# Jottery Sync-Hilfe anzeigen" ;;
        fr)    echo "# Afficher l'aide de synchronisation" ;;
        es)    echo "# Mostrar ayuda de sincronizacion" ;;
        pt)    echo "# Mostrar ajuda de sincronizacao" ;;
        it)    echo "# Mostra guida sincronizzazione" ;;
        nl)    echo "# Synchronisatie hulp weergeven" ;;
        pl)    echo "# Pokaz pomoc synchronizacji" ;;
        ru)    echo "# Pokazat spravku po sinhronizacii" ;;
        tr)    echo "# Senkronizasyon yardimini goster" ;;
        el)    echo "# Emfanisi voitheias syngchronismou" ;;
        ja)    echo "# Sync herupu wo hyouji" ;;
        ko)    echo "# Donggi-hwa dou-um pyosi" ;;
        zh)    echo "# Xianshi tongbu bangzhu" ;;
        *)     echo "# Show Jottery Sync help" ;;
    esac
}

get_register_comment() {
    local lang="$1"
    case "$lang" in
        de)    echo "# Client ueber CLI registrieren" ;;
        fr)    echo "# Enregistrer votre client via CLI" ;;
        es)    echo "# Registrar cliente desde CLI" ;;
        pt)    echo "# Registrar cliente via CLI" ;;
        it)    echo "# Registra client da CLI" ;;
        nl)    echo "# Client registreren via CLI" ;;
        pl)    echo "# Zarejestruj klienta przez CLI" ;;
        ru)    echo "# Zaregistrirovat klient cherez CLI" ;;
        tr)    echo "# CLI uzerinden istemci kaydet" ;;
        el)    echo "# Eggraphi pelati mesw CLI" ;;
        ja)    echo "# CLI kara client touroku" ;;
        ko)    echo "# CLI-eseo keullaieontu deungnok" ;;
        zh)    echo "# Tonguo CLI zhuce kehu duan" ;;
        *)     echo "# Register your client from the CLI" ;;
    esac
}

get_force_sync_comment() {
    local lang="$1"
    case "$lang" in
        de)    echo "# Synchronisation ueber CLI erzwingen" ;;
        fr)    echo "# Forcer la synchronisation via CLI" ;;
        es)    echo "# Forzar sincronizacion desde CLI" ;;
        pt)    echo "# Forcar sincronizacao via CLI" ;;
        it)    echo "# Forza sincronizzazione da CLI" ;;
        nl)    echo "# Synchronisatie forceren via CLI" ;;
        pl)    echo "# Wymus synchronizacje przez CLI" ;;
        ru)    echo "# Prinuditelno sinhronizirovat cherez CLI" ;;
        tr)    echo "# CLI uzerinden senkronizasyonu zorla" ;;
        el)    echo "# Anagkastikos syngchronismos mesw CLI" ;;
        ja)    echo "# CLI kara sync wo jikkou" ;;
        ko)    echo "# CLI-eseo donggi-hwa silhaeng" ;;
        zh)    echo "# Tonguo CLI qiangzhi tongbu" ;;
        *)     echo "# Force a sync from the CLI" ;;
    esac
}

# Reset demo database to clean state
reset_demo_database() {
    local lang="$1"
    local lang_dir="$DEMO_DIR/$lang"
    local db_path="$lang_dir/jottery.db"
    local demo_notes="$SCRIPT_DIR/jottery-demo-notes-${lang}.json"
    local jottery="$TUI_BIN_DIR/jottery"
    local demo_password="demo-password-2026"

    # Remove existing database
    rm -f "$db_path" "$db_path-wal" "$db_path-shm" "$lang_dir/.jottery_remember" 2>/dev/null

    # Import demo notes
    "$jottery" --database "$db_path" import --input "$demo_notes" --password "$demo_password" >/dev/null 2>&1 || return 1

    # Store password for auto-unlock
    "$jottery" --database "$db_path" store-password --password "$demo_password" >/dev/null 2>&1 || return 1

    return 0
}

# Generate demo for a single language and tape
generate_tape() {
    local lang="$1"
    local tape_name="$2"
    local db_path="$DEMO_DIR/$lang/jottery.db"
    local tape_file="$VHS_DIR/${tape_name}.tape"
    local output_file="$OUTPUT_DIR/$lang/${tape_name}.gif"
    local demo_notes="$SCRIPT_DIR/jottery-demo-notes-${lang}.json"

    # Check demo notes file exists
    if [[ ! -f "$demo_notes" ]]; then
        log_warn "Demo notes not found: $demo_notes (skipping)"
        return 1
    fi

    # Reset database to clean state before each demo
    log_info "  Resetting database..."
    if ! reset_demo_database "$lang"; then
        log_error "  Failed to reset database for $lang"
        return 1
    fi

    # Check tape file exists
    if [[ ! -f "$tape_file" ]]; then
        log_error "Tape file not found: $tape_file"
        return 1
    fi

    # Create output directory
    mkdir -p "$(dirname "$output_file")"

    log_info "Generating: $lang / $tape_name"

    # Set environment variables for jottery and VHS tapes
    # JOTTERY_DB_PATH - database location
    # LANG - locale for UI language
    # TUI_BIN_DIR - path to jottery binary
    # DEMO_* - language-specific text for demos
    export JOTTERY_DB_PATH="$db_path"
    export TUI_BIN_DIR="$TUI_BIN_DIR"
    export LANG="$(get_locale_for_lang "$lang")"
    export DEMO_SEARCH="$(get_search_term "$lang")"
    export DEMO_TEXT="$(get_sample_text "$lang")"
    export DEMO_NOTE="$(get_quick_note "$lang")"
    export DEMO_SYNC_HELP="$(get_sync_help_comment "$lang")"
    export DEMO_REGISTER="$(get_register_comment "$lang")"
    export DEMO_FORCE_SYNC="$(get_force_sync_comment "$lang")"
    export DEMO_NOTE_ID="$(get_note_id "$lang")"

    log_info "  Locale: $LANG, Search: $DEMO_SEARCH"

    # Create temporary tape file with variables substituted
    local temp_tape=$(mktemp)
    envsubst < "$tape_file" > "$temp_tape"

    if vhs "$temp_tape" --output "$output_file" 2>&1; then
        rm -f "$temp_tape"
        log_success "  Created: $output_file"
        return 0
    else
        rm -f "$temp_tape"
        log_error "  Failed: $tape_name for $lang"
        return 1
    fi
}

# Generate all tapes for a language
generate_language() {
    local lang="$1"
    shift
    local tapes=("$@")

    if [[ ${#tapes[@]} -eq 0 ]]; then
        tapes=("${ALL_TAPES[@]}")
    fi

    for tape in "${tapes[@]}"; do
        generate_tape "$lang" "$tape" || true
    done
}

# Main
main() {
    echo ""
    echo "========================================"
    echo "  Jottery TUI Demo Generation"
    echo "========================================"
    echo ""

    check_vhs
    check_jottery

    # Parse arguments
    local languages=()
    local tapes=()

    if [[ $# -eq 0 ]]; then
        languages=("${ALL_LANGUAGES[@]}")
    elif [[ $# -eq 1 ]]; then
        languages=("$1")
    else
        languages=("$1")
        shift
        tapes=("$@")
    fi

    for lang in "${languages[@]}"; do
        generate_language "$lang" "${tapes[@]}"
    done

    echo ""
    log_success "Generation complete"
    echo ""
}

main "$@"
