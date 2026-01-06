/**
 * Syntax highlighting language registry for highlight.js
 * Provides metadata and dynamic loading for syntax languages
 */

export interface SyntaxLanguage {
  id: string;
  name: string;
  aliases: string[];
  category: 'core' | 'web' | 'systems' | 'data' | 'other';
  estimatedSize: number; // in KB
}

/**
 * Core languages that are always bundled (most common)
 */
export const CORE_LANGUAGES: SyntaxLanguage[] = [
  { id: 'javascript', name: 'JavaScript', aliases: ['js', 'jsx', 'mjs', 'cjs'], category: 'core', estimatedSize: 8 },
  { id: 'typescript', name: 'TypeScript', aliases: ['ts', 'tsx'], category: 'core', estimatedSize: 9 },
  { id: 'python', name: 'Python', aliases: ['py', 'gyp', 'ipython'], category: 'core', estimatedSize: 10 },
  { id: 'markdown', name: 'Markdown', aliases: ['md', 'mkdown', 'mkd'], category: 'core', estimatedSize: 5 },
  { id: 'json', name: 'JSON', aliases: ['jsonc'], category: 'core', estimatedSize: 3 },
  { id: 'xml', name: 'HTML/XML', aliases: ['html', 'xhtml', 'rss', 'atom', 'xjb', 'xsd', 'xsl', 'plist', 'svg'], category: 'core', estimatedSize: 7 },
  { id: 'css', name: 'CSS', aliases: [], category: 'core', estimatedSize: 6 },
  { id: 'bash', name: 'Bash/Shell', aliases: ['sh', 'zsh'], category: 'core', estimatedSize: 5 },
  { id: 'calc', name: 'Calculator (REPL)', aliases: ['repl', 'repl-calc'], category: 'core', estimatedSize: 180 },
];

/**
 * Web development languages
 */
export const WEB_LANGUAGES: SyntaxLanguage[] = [
  { id: 'php', name: 'PHP', aliases: [], category: 'web', estimatedSize: 11 },
  { id: 'ruby', name: 'Ruby', aliases: ['rb', 'gemspec', 'podspec', 'thor', 'irb'], category: 'web', estimatedSize: 9 },
  { id: 'scss', name: 'SCSS', aliases: [], category: 'web', estimatedSize: 7 },
  { id: 'sass', name: 'Sass', aliases: [], category: 'web', estimatedSize: 6 },
  { id: 'less', name: 'Less', aliases: [], category: 'web', estimatedSize: 6 },
  { id: 'stylus', name: 'Stylus', aliases: ['styl'], category: 'web', estimatedSize: 5 },
  { id: 'handlebars', name: 'Handlebars', aliases: ['hbs', 'html.hbs', 'html.handlebars', 'htmlbars'], category: 'web', estimatedSize: 5 },
];

/**
 * Systems programming languages
 */
export const SYSTEMS_LANGUAGES: SyntaxLanguage[] = [
  { id: 'java', name: 'Java', aliases: ['jsp'], category: 'systems', estimatedSize: 12 },
  { id: 'csharp', name: 'C#', aliases: ['cs'], category: 'systems', estimatedSize: 10 },
  { id: 'go', name: 'Go', aliases: ['golang'], category: 'systems', estimatedSize: 8 },
  { id: 'rust', name: 'Rust', aliases: ['rs'], category: 'systems', estimatedSize: 10 },
  { id: 'swift', name: 'Swift', aliases: [], category: 'systems', estimatedSize: 11 },
  { id: 'kotlin', name: 'Kotlin', aliases: ['kt', 'kts'], category: 'systems', estimatedSize: 10 },
  { id: 'scala', name: 'Scala', aliases: [], category: 'systems', estimatedSize: 9 },
  { id: 'c', name: 'C', aliases: ['h'], category: 'systems', estimatedSize: 8 },
  { id: 'cpp', name: 'C++', aliases: ['cc', 'c++', 'h++', 'hpp', 'hh', 'hxx', 'cxx'], category: 'systems', estimatedSize: 15 },
  { id: 'objectivec', name: 'Objective-C', aliases: ['mm', 'objc', 'obj-c', 'obj-c++', 'objective-c++'], category: 'systems', estimatedSize: 9 },
];

/**
 * Data and configuration languages
 */
export const DATA_LANGUAGES: SyntaxLanguage[] = [
  { id: 'sql', name: 'SQL', aliases: [], category: 'data', estimatedSize: 8 },
  { id: 'yaml', name: 'YAML', aliases: ['yml'], category: 'data', estimatedSize: 6 },
  { id: 'toml', name: 'TOML', aliases: [], category: 'data', estimatedSize: 5 },
  { id: 'ini', name: 'INI', aliases: [], category: 'data', estimatedSize: 3 },
  { id: 'properties', name: 'Properties', aliases: [], category: 'data', estimatedSize: 3 },
  { id: 'diff', name: 'Diff', aliases: ['patch'], category: 'data', estimatedSize: 4 },
  { id: 'dockerfile', name: 'Dockerfile', aliases: ['docker'], category: 'data', estimatedSize: 4 },
];

/**
 * Other languages
 */
export const OTHER_LANGUAGES: SyntaxLanguage[] = [
  { id: 'perl', name: 'Perl', aliases: ['pl', 'pm'], category: 'other', estimatedSize: 9 },
  { id: 'r', name: 'R', aliases: [], category: 'other', estimatedSize: 7 },
  { id: 'matlab', name: 'MATLAB', aliases: [], category: 'other', estimatedSize: 8 },
  { id: 'lua', name: 'Lua', aliases: [], category: 'other', estimatedSize: 5 },
  { id: 'vim', name: 'Vim Script', aliases: [], category: 'other', estimatedSize: 6 },
  { id: 'powershell', name: 'PowerShell', aliases: ['pwsh', 'ps', 'ps1'], category: 'other', estimatedSize: 8 },
  { id: 'graphql', name: 'GraphQL', aliases: ['gql'], category: 'other', estimatedSize: 5 },
  { id: 'elixir', name: 'Elixir', aliases: ['ex', 'exs'], category: 'other', estimatedSize: 7 },
  { id: 'erlang', name: 'Erlang', aliases: ['erl'], category: 'other', estimatedSize: 8 },
  { id: 'haskell', name: 'Haskell', aliases: ['hs'], category: 'other', estimatedSize: 9 },
  { id: 'clojure', name: 'Clojure', aliases: ['clj', 'cljc', 'cljs', 'edn'], category: 'other', estimatedSize: 8 },
];

/**
 * All available languages
 */
export const ALL_LANGUAGES: SyntaxLanguage[] = [
  ...CORE_LANGUAGES,
  ...WEB_LANGUAGES,
  ...SYSTEMS_LANGUAGES,
  ...DATA_LANGUAGES,
  ...OTHER_LANGUAGES,
];

/**
 * Get language by ID or alias
 */
export function findLanguage(idOrAlias: string): SyntaxLanguage | undefined {
  const normalized = idOrAlias.toLowerCase();
  return ALL_LANGUAGES.find(
    lang => lang.id === normalized || lang.aliases.includes(normalized)
  );
}

/**
 * Get all aliases for a language including its ID
 */
export function getAllAliases(lang: SyntaxLanguage): string[] {
  return [lang.id, ...lang.aliases];
}

/**
 * Default enabled languages (core + SQL)
 */
export const DEFAULT_ENABLED_LANGUAGES = [
  ...CORE_LANGUAGES.map(l => l.id),
  'sql', // SQL is very common
];

/**
 * Calculate total estimated size for a set of languages
 */
export function calculateTotalSize(languageIds: string[]): number {
  return languageIds.reduce((total, id) => {
    const lang = ALL_LANGUAGES.find(l => l.id === id);
    return total + (lang?.estimatedSize || 0);
  }, 0);
}
