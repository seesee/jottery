/**
 * Screenshot generation for landing page placeholders
 * Run with: npx playwright test screenshots --project=firefox
 * Screenshots will be saved to: screenshots/en-GB/
 */

import { test, expect } from '@playwright/test';
import { readFile } from 'fs/promises';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

// Get the directory of this test file for resolving relative paths
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const DEMO_GENERATION_DIR = dirname(__dirname); // Parent of playwright/ is demo-generation/

// Normalize LANG environment variable to our file naming convention
function normalizeLang(lang: string | undefined): string {
  if (!lang) return 'en-GB';

  // Strip encoding suffix like .UTF-8
  let normalized = lang.replace(/\..*$/, '');

  // Convert underscore to hyphen (en_GB -> en-GB)
  normalized = normalized.replace('_', '-');

  // Handle just 'en' -> 'en-GB'
  if (normalized === 'en') return 'en-GB';

  // Map common variants
  const validLangs = ['en-GB', 'en-US', 'de', 'el', 'es', 'fr', 'it', 'ja', 'ko', 'nl', 'pl', 'pt', 'ru', 'tr', 'zh'];
  if (validLangs.includes(normalized)) return normalized;

  // Default to en-GB for unknown
  return 'en-GB';
}

const LANG = normalizeLang(process.env.LANG);
// Project root is parent of demo-generation
const PROJECT_ROOT = dirname(DEMO_GENERATION_DIR);
const SCREENSHOT_DIR = join(PROJECT_ROOT, 'screenshots', LANG);

// Helper to log screenshot save with absolute path
function logScreenshot(filename: string): void {
  console.log(`✓ Screenshot saved: ${join(SCREENSHOT_DIR, filename)}`);
}

// Map language codes to demo note files
function getDemoNotesFile(lang: string): string {
  // Check if a language-specific file exists
  const langFile = `jottery-demo-notes-${lang}.json`;
  // For now, fall back to the main demo file if no language-specific file
  // The language-specific files follow the pattern: jottery-demo-notes-{lang}.json
  return langFile;
}

// Language-specific tag colour mappings
// Only 2-3 tags per language to make colouring look intentional
const TAG_COLORS: Record<string, Record<string, string>> = {
  'en-GB': { recipe: 'orange', travel: 'blue', shopping: 'red' },
  'en-US': { recipe: 'orange', travel: 'blue', shopping: 'red' },
  'de': { rezept: 'orange', reise: 'blue', einkaufen: 'red' },
  'el': { 'συνταγή': 'orange', 'ταξίδι': 'blue', 'αγορές': 'red' },
  'es': { receta: 'orange', viaje: 'blue', compras: 'red' },
  'fr': { recette: 'orange', voyage: 'blue', courses: 'red' },
  'it': { ricetta: 'orange', viaggio: 'blue', spesa: 'red' },
  'ja': { 'レシピ': 'orange', '旅行': 'blue', '買い物': 'red' },
  'ko': { '레시피': 'orange', '여행': 'blue', '쇼핑': 'red' },
  'nl': { recept: 'orange', reis: 'blue', boodschappen: 'red' },
  'pl': { przepis: 'orange', 'podróż': 'blue', zakupy: 'red' },
  'pt': { receita: 'orange', viagem: 'blue', compras: 'red' },
  'ru': { 'рецепт': 'orange', 'путешествие': 'blue', 'покупки': 'red' },
  'tr': { tarif: 'orange', seyahat: 'blue', 'alışveriş': 'red' },
  'zh': { '食谱': 'orange', '旅行': 'blue', '购物': 'red' },
};

// Get tag colours for current language, falling back to en-GB
function getTagColors(lang: string): Record<string, string> {
  return TAG_COLORS[lang] || TAG_COLORS['en-GB'];
}

// Language-specific calculator content
// The calculator now supports Unicode variable names through internal ASCII aliasing
const CALCULATOR_CONTENT: Record<string, { comment: string; principal: string; rate: string; years: string }> = {
  'en-GB': { comment: '# Compound interest', principal: 'principal', rate: 'rate', years: 'years' },
  'en-US': { comment: '# Compound interest', principal: 'principal', rate: 'rate', years: 'years' },
  'de': { comment: '# Zinseszins', principal: 'Kapital', rate: 'Zinssatz', years: 'Jahre' },
  'el': { comment: '# Σύνθετος τόκος', principal: 'κεφάλαιο', rate: 'επιτόκιο', years: 'έτη' },
  'es': { comment: '# Interés compuesto', principal: 'capital', rate: 'tasa', years: 'años' },
  'fr': { comment: '# Intérêts composés', principal: 'capital', rate: 'taux', years: 'années' },
  'it': { comment: '# Interesse composto', principal: 'capitale', rate: 'tasso', years: 'anni' },
  'ja': { comment: '# 複利計算', principal: '元金', rate: '利率', years: '年数' },
  'ko': { comment: '# 복리 계산', principal: '원금', rate: '이율', years: '연수' },
  'nl': { comment: '# Samengestelde rente', principal: 'hoofdsom', rate: 'rente', years: 'jaren' },
  'pl': { comment: '# Procent składany', principal: 'kapitał', rate: 'stopa', years: 'lata' },
  'pt': { comment: '# Juros compostos', principal: 'capital', rate: 'taxa', years: 'anos' },
  'ru': { comment: '# Сложные проценты', principal: 'капитал', rate: 'ставка', years: 'лет' },
  'tr': { comment: '# Bileşik faiz', principal: 'anapara', rate: 'oran', years: 'yıl' },
  'zh': { comment: '# 复利计算', principal: '本金', rate: '利率', years: '年数' },
};

// Get calculator content for current language
function getCalculatorContent(lang: string) {
  return CALCULATOR_CONTENT[lang] || CALCULATOR_CONTENT['en-GB'];
}

// Language-specific mobile calculator content (budget example)
// The calculator now supports Unicode variable names through internal ASCII aliasing
const MOBILE_CALCULATOR_CONTENT: Record<string, { comment: string; income: string; rent: string }> = {
  'en-GB': { comment: '# Budget calc', income: 'income', rent: 'rent' },
  'en-US': { comment: '# Budget calc', income: 'income', rent: 'rent' },
  'de': { comment: '# Budget', income: 'Einkommen', rent: 'Miete' },
  'el': { comment: '# Προϋπολογισμός', income: 'εισόδημα', rent: 'ενοίκιο' },
  'es': { comment: '# Presupuesto', income: 'ingresos', rent: 'alquiler' },
  'fr': { comment: '# Budget', income: 'revenus', rent: 'loyer' },
  'it': { comment: '# Budget', income: 'entrate', rent: 'affitto' },
  'ja': { comment: '# 予算計算', income: '収入', rent: '家賃' },
  'ko': { comment: '# 예산 계산', income: '수입', rent: '임대료' },
  'nl': { comment: '# Budget', income: 'inkomen', rent: 'huur' },
  'pl': { comment: '# Budżet', income: 'dochód', rent: 'czynsz' },
  'pt': { comment: '# Orçamento', income: 'renda', rent: 'aluguel' },
  'ru': { comment: '# Бюджет', income: 'доход', rent: 'аренда' },
  'tr': { comment: '# Bütçe', income: 'gelir', rent: 'kira' },
  'zh': { comment: '# 预算计算', income: '收入', rent: '房租' },
};

// Get mobile calculator content for current language
function getMobileCalculatorContent(lang: string) {
  return MOBILE_CALCULATOR_CONTENT[lang] || MOBILE_CALCULATOR_CONTENT['en-GB'];
}

// Language-specific outliner content
const OUTLINER_CONTENT: Record<string, string[]> = {
  'en-GB': [
    'Project Roadmap',
    'Phase 1: Foundation',
    'Set up project structure',
    'Configure build tools',
    'Write initial tests',
    'Phase 2: Core Features',
    'User authentication',
    'Login/logout',
    'Password reset',
    'Data synchronisation',
    'Real-time updates',
    'Conflict resolution',
    'Phase 3: Polish',
    'UI/UX improvements',
    'Performance optimisation',
    'Documentation',
  ],
  'en-US': [
    'Project Roadmap',
    'Phase 1: Foundation',
    'Set up project structure',
    'Configure build tools',
    'Write initial tests',
    'Phase 2: Core Features',
    'User authentication',
    'Login/logout',
    'Password reset',
    'Data synchronization',
    'Real-time updates',
    'Conflict resolution',
    'Phase 3: Polish',
    'UI/UX improvements',
    'Performance optimization',
    'Documentation',
  ],
  'ja': [
    'プロジェクト計画',
    'フェーズ1：基盤構築',
    'プロジェクト構造の設定',
    'ビルドツールの構成',
    '初期テストの作成',
    'フェーズ2：主要機能',
    'ユーザー認証',
    'ログイン・ログアウト',
    'パスワードリセット',
    'データ同期',
    'リアルタイム更新',
    '競合解決',
    'フェーズ3：仕上げ',
    'UI/UXの改善',
    'パフォーマンス最適化',
    'ドキュメント作成',
  ],
  'de': [
    'Projektplan',
    'Phase 1: Grundlagen',
    'Projektstruktur einrichten',
    'Build-Tools konfigurieren',
    'Erste Tests schreiben',
    'Phase 2: Kernfunktionen',
    'Benutzerauthentifizierung',
    'Anmeldung/Abmeldung',
    'Passwort zurücksetzen',
    'Datensynchronisation',
    'Echtzeit-Updates',
    'Konfliktlösung',
    'Phase 3: Feinschliff',
    'UI/UX-Verbesserungen',
    'Leistungsoptimierung',
    'Dokumentation',
  ],
  'fr': [
    'Plan du projet',
    'Phase 1 : Fondations',
    'Configurer la structure du projet',
    'Configurer les outils de build',
    'Écrire les tests initiaux',
    'Phase 2 : Fonctionnalités principales',
    'Authentification utilisateur',
    'Connexion/déconnexion',
    'Réinitialisation du mot de passe',
    'Synchronisation des données',
    'Mises à jour en temps réel',
    'Résolution des conflits',
    'Phase 3 : Finitions',
    'Améliorations UI/UX',
    'Optimisation des performances',
    'Documentation',
  ],
  'es': [
    'Plan del proyecto',
    'Fase 1: Fundamentos',
    'Configurar estructura del proyecto',
    'Configurar herramientas de compilación',
    'Escribir pruebas iniciales',
    'Fase 2: Funciones principales',
    'Autenticación de usuarios',
    'Inicio/cierre de sesión',
    'Restablecimiento de contraseña',
    'Sincronización de datos',
    'Actualizaciones en tiempo real',
    'Resolución de conflictos',
    'Fase 3: Pulido',
    'Mejoras de UI/UX',
    'Optimización del rendimiento',
    'Documentación',
  ],
  'ko': [
    '프로젝트 로드맵',
    '1단계: 기반 구축',
    '프로젝트 구조 설정',
    '빌드 도구 구성',
    '초기 테스트 작성',
    '2단계: 핵심 기능',
    '사용자 인증',
    '로그인/로그아웃',
    '비밀번호 재설정',
    '데이터 동기화',
    '실시간 업데이트',
    '충돌 해결',
    '3단계: 마무리',
    'UI/UX 개선',
    '성능 최적화',
    '문서화',
  ],
  'zh': [
    '项目路线图',
    '第一阶段：基础建设',
    '设置项目结构',
    '配置构建工具',
    '编写初始测试',
    '第二阶段：核心功能',
    '用户认证',
    '登录/登出',
    '密码重置',
    '数据同步',
    '实时更新',
    '冲突解决',
    '第三阶段：完善',
    'UI/UX改进',
    '性能优化',
    '文档编写',
  ],
  'el': [
    'Οδικός χάρτης έργου',
    'Φάση 1: Θεμέλια',
    'Ρύθμιση δομής έργου',
    'Διαμόρφωση εργαλείων build',
    'Γραφή αρχικών δοκιμών',
    'Φάση 2: Βασικές λειτουργίες',
    'Ταυτοποίηση χρήστη',
    'Σύνδεση/Αποσύνδεση',
    'Επαναφορά κωδικού',
    'Συγχρονισμός δεδομένων',
    'Ενημερώσεις πραγματικού χρόνου',
    'Επίλυση συγκρούσεων',
    'Φάση 3: Τελειοποίηση',
    'Βελτιώσεις UI/UX',
    'Βελτιστοποίηση απόδοσης',
    'Τεκμηρίωση',
  ],
  'it': [
    'Roadmap del progetto',
    'Fase 1: Fondamenta',
    'Configurare la struttura del progetto',
    'Configurare gli strumenti di build',
    'Scrivere i test iniziali',
    'Fase 2: Funzionalità principali',
    'Autenticazione utente',
    'Login/logout',
    'Ripristino password',
    'Sincronizzazione dati',
    'Aggiornamenti in tempo reale',
    'Risoluzione conflitti',
    'Fase 3: Rifinitura',
    'Miglioramenti UI/UX',
    'Ottimizzazione prestazioni',
    'Documentazione',
  ],
  'nl': [
    'Project routekaart',
    'Fase 1: Fundament',
    'Projectstructuur opzetten',
    'Build-tools configureren',
    'Eerste tests schrijven',
    'Fase 2: Kernfuncties',
    'Gebruikersauthenticatie',
    'Inloggen/uitloggen',
    'Wachtwoord resetten',
    'Gegevenssynchronisatie',
    'Realtime updates',
    'Conflictoplossing',
    'Fase 3: Afwerking',
    'UI/UX-verbeteringen',
    'Prestatieoptimalisatie',
    'Documentatie',
  ],
  'pl': [
    'Plan projektu',
    'Faza 1: Fundamenty',
    'Konfiguracja struktury projektu',
    'Konfiguracja narzędzi budowania',
    'Pisanie początkowych testów',
    'Faza 2: Główne funkcje',
    'Uwierzytelnianie użytkownika',
    'Logowanie/wylogowanie',
    'Reset hasła',
    'Synchronizacja danych',
    'Aktualizacje w czasie rzeczywistym',
    'Rozwiązywanie konfliktów',
    'Faza 3: Szlify',
    'Ulepszenia UI/UX',
    'Optymalizacja wydajności',
    'Dokumentacja',
  ],
  'pt': [
    'Roteiro do projeto',
    'Fase 1: Fundamentos',
    'Configurar estrutura do projeto',
    'Configurar ferramentas de build',
    'Escrever testes iniciais',
    'Fase 2: Funcionalidades principais',
    'Autenticação de usuário',
    'Login/logout',
    'Redefinição de senha',
    'Sincronização de dados',
    'Atualizações em tempo real',
    'Resolução de conflitos',
    'Fase 3: Polimento',
    'Melhorias de UI/UX',
    'Otimização de desempenho',
    'Documentação',
  ],
  'ru': [
    'Дорожная карта проекта',
    'Этап 1: Фундамент',
    'Настройка структуры проекта',
    'Настройка инструментов сборки',
    'Написание начальных тестов',
    'Этап 2: Основные функции',
    'Аутентификация пользователя',
    'Вход/выход',
    'Сброс пароля',
    'Синхронизация данных',
    'Обновления в реальном времени',
    'Разрешение конфликтов',
    'Этап 3: Доработка',
    'Улучшения UI/UX',
    'Оптимизация производительности',
    'Документация',
  ],
  'tr': [
    'Proje yol haritası',
    'Aşama 1: Temel',
    'Proje yapısını kurma',
    'Build araçlarını yapılandırma',
    'İlk testleri yazma',
    'Aşama 2: Temel özellikler',
    'Kullanıcı kimlik doğrulaması',
    'Giriş/çıkış',
    'Şifre sıfırlama',
    'Veri senkronizasyonu',
    'Gerçek zamanlı güncellemeler',
    'Çakışma çözümü',
    'Aşama 3: Cilalama',
    'UI/UX iyileştirmeleri',
    'Performans optimizasyonu',
    'Dokümantasyon',
  ],
};

// Get outliner content for current language (falls back to en-GB)
function getOutlinerContent(lang: string): string[] {
  return OUTLINER_CONTENT[lang] || OUTLINER_CONTENT['en-GB'];
}

// Helper function to set the app's UI language
async function setAppLanguage(page: any, lang: string) {
  // Map our language codes to the app's locale codes
  const localeMap: Record<string, string> = {
    'en-GB': 'en-GB',
    'en-US': 'en-US',
    'de': 'de',
    'el': 'el',
    'es': 'es',
    'fr': 'fr',
    'it': 'it',
    'ja': 'ja',
    'ko': 'ko',
    'nl': 'nl',
    'pl': 'pl',
    'pt': 'pt',
    'ru': 'ru',
    'tr': 'tr',
    'zh': 'zh',
  };

  const locale = localeMap[lang] || 'en-GB';

  // Open settings using stable data-testid
  const settingsButton = page.locator('[data-testid="btn-settings"]');
  await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
  await settingsButton.click();
  await page.waitForTimeout(500);

  // Navigate to General tab using stable ID
  const generalTab = page.locator('#tab-general');
  await generalTab.waitFor({ state: 'visible' });
  await generalTab.click();
  await page.waitForTimeout(500);

  // Find the language dropdown and select the target locale
  const modal = page.locator('[role="dialog"]').first();
  const languageDropdown = modal.locator('select').filter({ has: page.locator(`option[value="${locale}"]`) }).first();
  await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
  await languageDropdown.selectOption(locale);
  await page.waitForTimeout(500);

  // Save settings using stable data-testid
  const saveButton = page.locator('[data-testid="btn-settings-save"]');
  await saveButton.waitFor({ state: 'visible' });
  await saveButton.click();

  // Wait for toast to disappear and close modal
  await page.waitForTimeout(2000);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
}

// Helper function to import demo notes via Settings UI
async function importDemoNotes(page: any, demoFile: string = getDemoNotesFile(LANG)) {
  const demoNotesPath = join(DEMO_GENERATION_DIR, demoFile);

  // Open settings using stable data-testid
  const settingsButton = page.locator('[data-testid="btn-settings"]');
  await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
  await settingsButton.click();
  await page.waitForTimeout(500);

  // Navigate to Advanced tab using stable ID
  const advancedTab = page.locator('#tab-advanced');
  await advancedTab.waitFor({ state: 'visible' });
  await advancedTab.click();
  await page.waitForTimeout(500);

  // Find and click the Import button using stable data-testid
  const importButton = page.locator('[data-testid="btn-import-notes"]');
  await importButton.waitFor({ state: 'visible' });

  // Set up file chooser handler before clicking
  const fileChooserPromise = page.waitForEvent('filechooser');
  await importButton.click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(demoNotesPath);

  // Wait for import to complete - look for the Done button using stable data-testid
  const doneButton = page.locator('[data-testid="btn-import-done"]');
  await doneButton.waitFor({ state: 'visible', timeout: 15000 });
  await doneButton.click();
  await page.waitForTimeout(500);

  // Close settings modal
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);

  // Verify modal is closed
  const modalCheck = page.locator('[role="dialog"]').first();
  const isModalOpen = await modalCheck.isVisible().catch(() => false);

  if (isModalOpen) {
    // Try one more escape
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);
  }
}

// Helper to set tag colors directly in IndexedDB and update the settings store
async function setTagColors(page: any, tagColors: Record<string, string>) {
  await page.evaluate(async (colors: Record<string, string>) => {
    // Import settingsRepository and settings store from the global app context
    // @ts-ignore - These are available in the browser context
    const { settingsRepository, settings, DEFAULT_SETTINGS } = window.__appContext || {};

    if (!settingsRepository || !settings) {
      throw new Error('App context not available - settingsRepository or settings store not found');
    }

    // Update settings via the repository (which writes to IndexedDB)
    const currentSettings = await settingsRepository.get();
    const updatedSettings = {
      ...currentSettings,
      tagColors: colors,
    };
    await settingsRepository.update(updatedSettings);

    // Update the settings store to reflect the change immediately
    settings.set({ ...DEFAULT_SETTINGS, ...updatedSettings });
  }, tagColors);

  await page.waitForTimeout(500);
}

test.describe('Landing Page Screenshots - Light Mode', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload with theme parameter and set up password
    await page.goto('/?theme=light');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language before importing notes
    if (LANG !== 'en-GB') {
      await setAppLanguage(page, LANG);
    }

    // Import demo notes
    await importDemoNotes(page);

    // Set tag colors using language-specific tag names
    await setTagColors(page, getTagColors(LANG));

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);
  });

  test('01-light. Main Interface - Note List and Editor', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note (should be pinned at top)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-light.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-light.png');
  });

  test('01a-light. Main Interface - Preview Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button (handle multiple languages)
    const previewButton = page.locator('[data-testid="btn-preview"]');
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-light-preview.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-light-preview.png');
  });

  test('01b-light. Main Interface - Travel Note Preview', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button (handle multiple languages)
    const previewButton = page.locator('[data-testid="btn-preview"]');
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-light-travel-preview.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-light-travel-preview.png');
  });

  test('01c-light. Main Interface - Travel Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-light-travel.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-light-travel.png');
  });

  test('02-light. Rich Editor - Recipe Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click a content-rich note (Python in original demo, Recipe in language-specific)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    // Try to find Python QuickSort first (original demo), fall back to 3rd note (recipe in language-specific demos)
    let contentNote = noteListItems.filter({ hasText: /Python Quick Sort|QuickSort/i }).first();
    const hasContentNote = await contentNote.isVisible().catch(() => false);
    if (!hasContentNote) {
      contentNote = noteListItems.nth(2); // 3rd note (0-indexed) is the recipe
    }
    await contentNote.waitFor({ state: 'visible' });
    await contentNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/02-rich-editor-light.png`,
      fullPage: false,
    });

    logScreenshot('02-rich-editor-light.png');
  });

  test('03-light. Multi-Select - Bulk Operations', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Select 3 notes using Cmd/Ctrl+Click (skip first note as it's pinned)
    const noteListItems = page.locator('button.note-list-item');
    const noteCount = await noteListItems.count();

    // Use Meta (Cmd) on Mac, Control on other platforms
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';

    // Click notes 1-3 (skip index 0 as Welcome note is pinned)
    for (let i = 1; i <= Math.min(3, noteCount - 1); i++) {
      await noteListItems.nth(i).click({ modifiers: [modifier] });
      await page.waitForTimeout(400);
    }

    // Wait for toolbar to appear and settle
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/03-multi-select-light.png`,
      fullPage: false,
    });

    logScreenshot('03-multi-select-light.png');
  });

  test('04-light. Version History', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Create version history by making edits and navigating away/back to trigger sync
    const editor = page.locator('.cm-content').first();
    const welcomeNote = noteListItems.first(); // Use first note (welcome) instead of searching by title

    // Edit 1: Add packing list start
    await editor.click();
    await page.keyboard.press('End'); // Go to end of document
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('## Packing List');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Passport');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Camera');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 1)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Edit 2: Add more items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Travel adapter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Guidebook');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 2)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Edit 3: Add final items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Comfortable shoes');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Rain jacket');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 3)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Open the more menu to access version history
    const moreButton = page.locator('button[aria-label="More actions"]').first();
    await moreButton.waitFor({ state: 'visible', timeout: 5000 });
    await moreButton.click();
    await page.waitForTimeout(500);

    // Click Version History option (look for 📜 emoji)
    const versionHistoryButton = page.locator('button').filter({ hasText: /📜/ }).first();
    await versionHistoryButton.waitFor({ state: 'visible', timeout: 5000 });
    await versionHistoryButton.click();
    await page.waitForTimeout(1000);

    // Wait for modal to appear
    const modal = page.locator('[role="dialog"]').first();
    await modal.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {
      console.warn('⚠️ Version history modal did not appear');
    });

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/04-version-history-light.png`,
      fullPage: false,
    });

    logScreenshot('04-version-history-light.png');
  });

  test('05-light. REPL Calculator', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('[data-testid="btn-new-note"]');
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples (results auto-evaluate in gray)
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    // Get language-specific calculator content
    const calc = getCalculatorContent(LANG);

    await page.keyboard.type(calc.comment);
    await page.keyboard.press('Enter');
    await page.keyboard.type(`${calc.principal} = 1000`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.rate} = 0.05`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.years} = 10`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.principal} * (1 + ${calc.rate})^${calc.years}`);
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/05-calculator-light.png`,
      fullPage: false,
    });

    logScreenshot('05-calculator-light.png');
  });
});

test.describe('Landing Page Screenshots - Dark Mode', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language before importing notes
    if (LANG !== 'en-GB') {
      await setAppLanguage(page, LANG);
    }

    // Import demo notes
    await importDemoNotes(page);

    // Set tag colors using language-specific tag names
    await setTagColors(page, getTagColors(LANG));

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);

    // Apply dark theme manually
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500); // Wait for colors to update
  });

  test('01-dark. Main Interface - Note List and Editor', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-dark.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-dark.png');
  });

  test('01a-dark. Main Interface - Preview Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button (handle multiple languages)
    const previewButton = page.locator('[data-testid="btn-preview"]');
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-preview.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-dark-preview.png');
  });

  test('01b-dark. Main Interface - Travel Note Preview', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Click Preview button (handle multiple languages)
    const previewButton = page.locator('[data-testid="btn-preview"]');
    await previewButton.waitFor({ state: 'visible' });
    await previewButton.click();
    await page.waitForTimeout(500);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-travel-preview.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-dark-travel-preview.png');
  });

  test('01c-dark. Main Interface - Travel Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01-main-interface-dark-travel.png`,
      fullPage: false,
    });

    logScreenshot('01-main-interface-dark-travel.png');
  });

  test('02-dark. Rich Editor - Recipe Note', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Find and click a content-rich note
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    let contentNote = noteListItems.filter({ hasText: /Python Quick Sort|QuickSort/i }).first();
    const hasContentNote = await contentNote.isVisible().catch(() => false);
    if (!hasContentNote) {
      contentNote = noteListItems.nth(2); // 3rd note is the recipe
    }
    await contentNote.waitFor({ state: 'visible' });
    await contentNote.click();
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/02-rich-editor-dark.png`,
      fullPage: false,
    });

    logScreenshot('02-rich-editor-dark.png');
  });

  test('03-dark. Multi-Select - Bulk Operations', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Select 3 notes using Cmd/Ctrl+Click (skip first note as it's pinned)
    const noteListItems = page.locator('button.note-list-item');
    const noteCount = await noteListItems.count();

    // Use Meta (Cmd) on Mac, Control on other platforms
    const modifier = process.platform === 'darwin' ? 'Meta' : 'Control';

    // Click notes 1-3 (skip index 0 as Welcome note is pinned)
    for (let i = 1; i <= Math.min(3, noteCount - 1); i++) {
      await noteListItems.nth(i).click({ modifiers: [modifier] });
      await page.waitForTimeout(400);
    }

    // Wait for toolbar to appear and settle
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/03-multi-select-dark.png`,
      fullPage: false,
    });

    logScreenshot('03-multi-select-dark.png');
  });

  test('04-dark. Version History', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Click on the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Create version history by making edits and navigating away/back to trigger sync
    const editor = page.locator('.cm-content').first();
    const welcomeNote = noteListItems.first();

    // Edit 1: Add packing list start
    await editor.click();
    await page.keyboard.press('End'); // Go to end of document
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('## Packing List');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Passport');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Camera');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 1)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Edit 2: Add more items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Travel adapter');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Guidebook');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 2)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Edit 3: Add final items
    await editor.click();
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Comfortable shoes');
    await page.keyboard.press('Enter');
    await page.keyboard.type('- Rain jacket');
    await page.waitForTimeout(500);

    // Navigate away and back to trigger sync (creates version 3)
    await welcomeNote.click();
    await page.waitForTimeout(1000);
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Open the more menu to access version history
    const moreButton = page.locator('button[aria-label="More actions"]').first();
    await moreButton.waitFor({ state: 'visible', timeout: 5000 });
    await moreButton.click();
    await page.waitForTimeout(500);

    // Click Version History option (look for 📜 emoji)
    const versionHistoryButton = page.locator('button').filter({ hasText: /📜/ }).first();
    await versionHistoryButton.waitFor({ state: 'visible', timeout: 5000 });
    await versionHistoryButton.click();
    await page.waitForTimeout(1000);

    // Wait for modal to appear
    const modal = page.locator('[role="dialog"]').first();
    await modal.waitFor({ state: 'visible', timeout: 5000 }).catch(() => {
      console.warn('⚠️ Version history modal did not appear');
    });

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/04-version-history-dark.png`,
      fullPage: false,
    });

    logScreenshot('04-version-history-dark.png');
  });

  test('05-dark. REPL Calculator', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('[data-testid="btn-new-note"]');
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples (results auto-evaluate in gray)
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    // Get language-specific calculator content
    const calc = getCalculatorContent(LANG);

    await page.keyboard.type(calc.comment);
    await page.keyboard.press('Enter');
    await page.keyboard.type(`${calc.principal} = 1000`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.rate} = 0.05`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.years} = 10`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(400);

    await page.keyboard.type(`${calc.principal} * (1 + ${calc.rate})^${calc.years}`);
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/05-calculator-dark.png`,
      fullPage: false,
    });

    logScreenshot('05-calculator-dark.png');
  });
});

// Helper function to import demo notes on mobile (via hamburger menu)
async function importDemoNotesMobile(page: any, demoFile: string = getDemoNotesFile(LANG)) {
  const demoNotesPath = join(DEMO_GENERATION_DIR, demoFile);

  // On mobile, settings is behind the hamburger menu
  const hamburgerMenu = page.locator('[data-testid="btn-hamburger-menu"]');
  const isHamburgerVisible = await hamburgerMenu.isVisible().catch(() => false);

  if (isHamburgerVisible) {
    await hamburgerMenu.click();
    await page.waitForTimeout(500);
  }

  // Now find and click Settings (mobile menu)
  const settingsButton = page.locator('[data-testid="btn-settings-mobile"]');
  await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
  await settingsButton.click();
  await page.waitForTimeout(500);

  // Navigate to Advanced tab using stable ID
  const advancedTab = page.locator('#tab-advanced');
  await advancedTab.waitFor({ state: 'visible' });
  await advancedTab.click();
  await page.waitForTimeout(500);

  // Find and click the Import button
  const importButton = page.locator('[data-testid="btn-import-notes"]');
  await importButton.waitFor({ state: 'visible' });

  // Set up file chooser handler before clicking
  const fileChooserPromise = page.waitForEvent('filechooser');
  await importButton.click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(demoNotesPath);

  // Wait for import to complete - look for the Done button
  const doneButton = page.locator('[data-testid="btn-import-done"]');
  await doneButton.waitFor({ state: 'visible', timeout: 15000 });
  await doneButton.click();
  await page.waitForTimeout(500);

  // Close settings modal
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);

  // Verify modal is closed
  const modalCheck = page.locator('[role="dialog"]').first();
  const isModalOpen = await modalCheck.isVisible().catch(() => false);

  if (isModalOpen) {
    // Try one more escape
    await page.keyboard.press('Escape');
    await page.waitForTimeout(500);
  }
}

// Helper function to set the app's UI language on mobile (via hamburger menu)
async function setAppLanguageMobile(page: any, lang: string) {
  const localeMap: Record<string, string> = {
    'en-GB': 'en-GB', 'en-US': 'en-US', 'de': 'de', 'el': 'el', 'es': 'es',
    'fr': 'fr', 'it': 'it', 'ja': 'ja', 'ko': 'ko', 'nl': 'nl',
    'pl': 'pl', 'pt': 'pt', 'ru': 'ru', 'tr': 'tr', 'zh': 'zh',
  };
  const locale = localeMap[lang] || 'en-GB';

  // On mobile, settings is behind the hamburger menu
  const hamburgerMenu = page.locator('[data-testid="btn-hamburger-menu"]');
  const isHamburgerVisible = await hamburgerMenu.isVisible().catch(() => false);

  if (isHamburgerVisible) {
    await hamburgerMenu.click();
    await page.waitForTimeout(500);
  }

  // Open settings from mobile menu
  const settingsButton = page.locator('[data-testid="btn-settings-mobile"]');
  await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
  await settingsButton.click();
  await page.waitForTimeout(500);

  // Navigate to General tab using stable ID
  const generalTab = page.locator('#tab-general');
  await generalTab.waitFor({ state: 'visible' });
  await generalTab.click();
  await page.waitForTimeout(500);

  // Find the language dropdown and select the target locale
  const modal = page.locator('[role="dialog"]').first();
  const languageDropdown = modal.locator('select').filter({ has: page.locator(`option[value="${locale}"]`) }).first();
  await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
  await languageDropdown.selectOption(locale);
  await page.waitForTimeout(500);

  // Save settings using stable data-testid
  const saveButton = page.locator('[data-testid="btn-settings-save"]');
  await saveButton.waitFor({ state: 'visible' });
  await saveButton.click();

  // Wait for toast to disappear and close modal
  await page.waitForTimeout(2000);
  await page.keyboard.press('Escape');
  await page.waitForTimeout(500);
}

test.describe('Landing Page Screenshots - Mobile Light', () => {
  // iPhone 12 viewport
  const MOBILE_VIEWPORT = { width: 390, height: 844 };

  test.beforeEach(async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize(MOBILE_VIEWPORT);

    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload with light theme
    await page.goto('/?theme=light');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language before importing notes
    if (LANG !== 'en-GB') {
      await setAppLanguageMobile(page, LANG);
    }

    // Import demo notes (mobile version - handles hamburger menu)
    await importDemoNotesMobile(page);

    // Set tag colors using language-specific tag names
    await setTagColors(page, getTagColors(LANG));

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);
  });

  test('06-mobile-light. Note List View', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Take screenshot of the note list (mobile shows list by default)
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/06-mobile-list-light.png`,
      fullPage: false,
    });

    logScreenshot('06-mobile-list-light.png');
  });

  test('07-mobile-light. Travel Itinerary Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot of the note view
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/07-mobile-travel-light.png`,
      fullPage: false,
    });

    logScreenshot('07-mobile-travel-light.png');
  });

  test('08-mobile-light. Calculator Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Create a new note - on mobile this uses the mobile-specific new button
    const newButton = page.locator('[data-testid="btn-new-note-mobile"]');

    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    // Get language-specific mobile calculator content
    const mobileCalc = getMobileCalculatorContent(LANG);

    await page.keyboard.type(mobileCalc.comment);
    await page.keyboard.press('Enter');
    await page.keyboard.type(`${mobileCalc.income} = 5000`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(`${mobileCalc.rent} = 1500`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(`${mobileCalc.income} - ${mobileCalc.rent}`);
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/08-mobile-calculator-light.png`,
      fullPage: false,
    });

    logScreenshot('08-mobile-calculator-light.png');
  });
});

test.describe('Landing Page Screenshots - Mobile Dark', () => {
  // iPhone 12 viewport
  const MOBILE_VIEWPORT = { width: 390, height: 844 };

  test.beforeEach(async ({ page }) => {
    // Set mobile viewport
    await page.setViewportSize(MOBILE_VIEWPORT);

    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language before importing notes
    if (LANG !== 'en-GB') {
      await setAppLanguageMobile(page, LANG);
    }

    // Import demo notes (mobile version - handles hamburger menu)
    await importDemoNotesMobile(page);

    // Set tag colors using language-specific tag names
    await setTagColors(page, getTagColors(LANG));

    // Wait for tag colors to apply
    await page.waitForTimeout(1000);

    // Apply dark theme
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);
  });

  test('06-mobile-dark. Note List View', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Take screenshot of the note list (mobile shows list by default)
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/06-mobile-list-dark.png`,
      fullPage: false,
    });

    logScreenshot('06-mobile-list-dark.png');
  });

  test('07-mobile-dark. Travel Itinerary Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Find and click the travel note (always at index 3 in demo data)
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const travelNote = noteListItems.nth(3);
    await travelNote.waitFor({ state: 'visible' });
    await travelNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot of the note view
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/07-mobile-travel-dark.png`,
      fullPage: false,
    });

    logScreenshot('07-mobile-travel-dark.png');
  });

  test('08-mobile-dark. Calculator Note', async ({ page }) => {
    // Wait for app to settle
    await page.waitForTimeout(1000);

    // Create a new note - on mobile this is a "+" icon button
    const newButton = page.locator('[data-testid="btn-new-note-mobile"]');
    await newButton.waitFor({ state: 'visible' });
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select calc
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('calc');
    await page.waitForTimeout(1000);

    // Find the editor and focus it
    const editor = page.locator('.cm-content').first();
    await editor.click();
    await page.waitForTimeout(500);

    // Type calculator examples
    await page.keyboard.type('1 + 1');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2 + 2');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('10 * 5');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('100 / 4');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('2^8');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type('sqrt(144)');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    // Get language-specific mobile calculator content
    const mobileCalc = getMobileCalculatorContent(LANG);

    await page.keyboard.type(mobileCalc.comment);
    await page.keyboard.press('Enter');
    await page.keyboard.type(`${mobileCalc.income} = 5000`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(`${mobileCalc.rent} = 1500`);
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(`${mobileCalc.income} - ${mobileCalc.rent}`);
    await page.waitForTimeout(1000);

    // Take screenshot
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/08-mobile-calculator-dark.png`,
      fullPage: false,
    });

    logScreenshot('08-mobile-calculator-dark.png');
  });
});

test.describe('Landing Page Screenshots - Outliner Light', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload with theme parameter and set up password
    await page.goto('/?theme=light');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language
    if (LANG !== 'en-GB') {
      await setAppLanguage(page, LANG);
    }
  });

  test('09-light. Outliner Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('[data-testid="btn-new-note"]');
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select outliner
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('outliner');
    await page.waitForTimeout(1500);

    // Outliner mode uses contenteditable divs, not CodeMirror
    // Find the first outliner node content area and click it
    const outlinerContent = page.locator('[contenteditable="true"].content, .outliner-node .content[contenteditable]').first();
    await outlinerContent.waitFor({ state: 'visible', timeout: 10000 });
    await outlinerContent.click();
    await page.waitForTimeout(500);

    // Get language-specific outliner content
    const outline = getOutlinerContent(LANG);

    // Type the first item and create hierarchy using Tab for indentation and Enter for new items
    await page.keyboard.type(outline[0]); // Project Roadmap
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[1]); // Phase 1: Foundation
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab'); // Indent to create child
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[2]); // Set up project structure
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[3]); // Configure build tools
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[4]); // Write initial tests
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab'); // Unindent
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[5]); // Phase 2: Core Features
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[6]); // User authentication
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[7]); // Login/logout
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[8]); // Password reset
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[9]); // Data synchronisation
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[10]); // Real-time updates
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[11]); // Conflict resolution
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[12]); // Phase 3: Polish
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[13]); // UI/UX improvements
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[14]); // Performance optimisation
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[15]); // Documentation
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/09-outliner-light.png`,
      fullPage: false,
    });

    logScreenshot('09-outliner-light.png');
  });
});

test.describe('Landing Page Screenshots - Outliner Dark', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Set the app's UI language
    if (LANG !== 'en-GB') {
      await setAppLanguage(page, LANG);
    }

    // Apply dark theme
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);
  });

  test('09-dark. Outliner Mode', async ({ page }) => {
    // Wait for theme to fully apply
    await page.waitForTimeout(1000);

    // Create a new note
    const newButton = page.locator('[data-testid="btn-new-note"]');
    await newButton.click();
    await page.waitForTimeout(1000);

    // Find the language dropdown and select outliner
    const languageDropdown = page.locator('select').first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('outliner');
    await page.waitForTimeout(1500);

    // Outliner mode uses contenteditable divs, not CodeMirror
    // Find the first outliner node content area and click it
    const outlinerContent = page.locator('[contenteditable="true"].content, .outliner-node .content[contenteditable]').first();
    await outlinerContent.waitFor({ state: 'visible', timeout: 10000 });
    await outlinerContent.click();
    await page.waitForTimeout(500);

    // Get language-specific outliner content
    const outline = getOutlinerContent(LANG);

    // Type the first item and create hierarchy using Tab for indentation and Enter for new items
    await page.keyboard.type(outline[0]); // Project Roadmap
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[1]); // Phase 1: Foundation
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab'); // Indent to create child
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[2]); // Set up project structure
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[3]); // Configure build tools
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[4]); // Write initial tests
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab'); // Unindent
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[5]); // Phase 2: Core Features
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[6]); // User authentication
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[7]); // Login/logout
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[8]); // Password reset
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[9]); // Data synchronisation
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[10]); // Real-time updates
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[11]); // Conflict resolution
    await page.keyboard.press('Enter');
    await page.keyboard.press('Shift+Tab');
    await page.keyboard.press('Shift+Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[12]); // Phase 3: Polish
    await page.keyboard.press('Enter');
    await page.keyboard.press('Tab');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[13]); // UI/UX improvements
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[14]); // Performance optimisation
    await page.keyboard.press('Enter');
    await page.waitForTimeout(300);

    await page.keyboard.type(outline[15]); // Documentation
    await page.waitForTimeout(1000);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}/09-outliner-dark.png`,
      fullPage: false,
    });

    logScreenshot('09-outliner-dark.png');
  });
});

test.describe('Landing Page Screenshots - Greek UI Light', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload with theme parameter and set up password
    await page.goto('/?theme=light');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Import Greek demo notes
    await importDemoNotes(page, 'jottery-demo-notes-el.json');

    // Set tag colors for Greek tags
    await setTagColors(page, {
      'καλωσόρισμα': 'blue',
      'επίδειξη': 'purple',
      'συνταγή': 'orange',
      'ταξίδι': 'green',
      'αγορές': 'red',
    });

    await page.waitForTimeout(1000);

    // Change language to Greek via settings
    const settingsButton = page.locator('[data-testid="btn-settings"]');
    await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
    await settingsButton.click();
    await page.waitForTimeout(500);

    // Navigate to General tab where language dropdown is
    const generalTab = page.locator('#tab-general');
    await generalTab.waitFor({ state: 'visible' });
    await generalTab.click();
    await page.waitForTimeout(500);

    // Find the language dropdown and select Greek
    const modal = page.locator('[role="dialog"]').first();
    const languageDropdown = modal.locator('select').filter({ has: page.locator('option[value="el"]') }).first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('el');
    await page.waitForTimeout(1000);

    // Save settings to apply the language change
    const saveButton = page.locator('[data-testid="btn-settings-save"]');
    await saveButton.waitFor({ state: 'visible' });
    await saveButton.click();

    // Wait for toast to disappear (toast displays for ~3 seconds)
    await page.waitForTimeout(4000);
  });

  test('10-light. Greek UI - Multi-lingual', async ({ page }) => {
    // Wait for locale to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note (should be pinned at top - titled "Καλώς ήρθατε στο Jottery!")
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot showing Greek UI elements
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/10-greek-ui-light.png`,
      fullPage: false,
    });

    logScreenshot('10-greek-ui-light.png');
  });
});

test.describe('Landing Page Screenshots - Greek UI Dark', () => {
  test.beforeEach(async ({ page }) => {
    // Clear all storage before test
    await page.goto('/');
    await page.evaluate(() => {
      localStorage.clear();
      sessionStorage.clear();
      indexedDB.databases().then((dbs) => {
        dbs.forEach((db) => {
          if (db.name) indexedDB.deleteDatabase(db.name);
        });
      });
    });

    // Reload and set up password
    await page.goto('/');

    // Wait for landing page, then click "Try It Out"
    await page.waitForTimeout(1000);
    const tryItButton = page.locator('button').filter({ hasText: /Try It Out|Start using/i }).first();
    const isTryItVisible = await tryItButton.isVisible().catch(() => false);

    if (isTryItVisible) {
      await tryItButton.click();
      await page.waitForTimeout(500);
    }

    // Set up password
    const passwordInputs = page.locator('input[type="password"]');
    await passwordInputs.first().waitFor({ state: 'visible' });
    await passwordInputs.first().fill('screenshot-test-password');
    await passwordInputs.nth(1).fill('screenshot-test-password');
    await page.locator('button[type="submit"]').click();

    // Wait for app to load
    await page.waitForTimeout(2000);

    // Import Greek demo notes
    await importDemoNotes(page, 'jottery-demo-notes-el.json');

    // Set tag colors for Greek tags
    await setTagColors(page, {
      'καλωσόρισμα': 'blue',
      'επίδειξη': 'purple',
      'συνταγή': 'orange',
      'ταξίδι': 'green',
      'αγορές': 'red',
    });

    await page.waitForTimeout(1000);

    // Change language to Greek via settings
    const settingsButton = page.locator('[data-testid="btn-settings"]');
    await settingsButton.waitFor({ state: 'visible', timeout: 10000 });
    await settingsButton.click();
    await page.waitForTimeout(500);

    // Navigate to General tab where language dropdown is
    const generalTab = page.locator('#tab-general');
    await generalTab.waitFor({ state: 'visible' });
    await generalTab.click();
    await page.waitForTimeout(500);

    // Find the language dropdown and select Greek
    const modal = page.locator('[role="dialog"]').first();
    const languageDropdown = modal.locator('select').filter({ has: page.locator('option[value="el"]') }).first();
    await languageDropdown.waitFor({ state: 'visible', timeout: 5000 });
    await languageDropdown.selectOption('el');
    await page.waitForTimeout(1000);

    // Save settings to apply the language change
    const saveButton = page.locator('[data-testid="btn-settings-save"]');
    await saveButton.waitFor({ state: 'visible' });
    await saveButton.click();

    // Wait for toast to disappear (toast displays for ~3 seconds)
    await page.waitForTimeout(4000);

    // Apply dark theme
    await page.evaluate(() => {
      document.documentElement.classList.add('dark');
    });
    await page.waitForTimeout(500);
  });

  test('10-dark. Greek UI - Multi-lingual', async ({ page }) => {
    // Wait for locale to fully apply
    await page.waitForTimeout(1000);

    // Click on the welcome note (should be pinned at top - titled "Καλώς ήρθατε στο Jottery!")
    const noteListItems = page.locator('.note-list-item, [role="listitem"]');
    const firstNote = noteListItems.first();
    await firstNote.waitFor({ state: 'visible' });
    await firstNote.click();
    await page.waitForTimeout(1000);

    // Take screenshot showing Greek UI elements
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/10-greek-ui-dark.png`,
      fullPage: false,
    });

    logScreenshot('10-greek-ui-dark.png');
  });
});
