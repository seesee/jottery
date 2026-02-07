<script lang="ts">
  import { _, locale } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import ScreenshotPlaceholder from './ScreenshotPlaceholder.svelte';
  import LanguagePicker from './LanguagePicker.svelte';
  import { AVAILABLE_LOCALES } from '../services/i18nService';

  /** Feature with light/dark screenshots (paths are relative, base is added dynamically) */
  interface WebFeature {
    key: string;
    icon: string;
    screenshotLight: string;
    screenshotDark: string;
  }

  export let onGetStarted: () => void;

  let isDarkMode = false;
  let currentCarouselSlide = 0;
  let carouselInterval: ReturnType<typeof setInterval>;
  let lightboxImage: { src: string; alt: string } | null = null;

  // Aspirational language mapping - shows a different language as a demo
  // to showcase multilingual capabilities
  const ASPIRATIONAL_LANG: Record<string, string> = {
    'en-GB': 'el',  // English shows Greek
    'en-US': 'el',
    'ja': 'en-GB',  // Japanese shows English
    'ko': 'en-GB',
    'zh': 'en-GB',
    'de': 'fr',     // German shows French
    'fr': 'de',
    'es': 'pt',
    'pt': 'es',
    'it': 'fr',
    'nl': 'de',
    'pl': 'de',
    'ru': 'en-GB',
    'tr': 'en-GB',
    'el': 'en-GB',
  };

  // Get the current locale, with fallback to en-GB
  $: currentLocale = $locale || 'en-GB';

  // Check if screenshots exist for a locale (we have screenshots for these languages)
  const AVAILABLE_SCREENSHOT_LOCALES = ['en-GB', 'en-US', 'de', 'el', 'es', 'fr', 'it', 'ja', 'ko'];

  // Get the screenshot base path for the current locale (with fallback)
  $: screenshotLocale = AVAILABLE_SCREENSHOT_LOCALES.includes(currentLocale) ? currentLocale : 'en-GB';
  $: screenshotBase = `/screenshots/${screenshotLocale}`;

  // Get the aspirational language for the multilingual demo
  $: aspirationalLang = ASPIRATIONAL_LANG[currentLocale] || 'el';
  $: aspirationalScreenshotBase = `/screenshots/${AVAILABLE_SCREENSHOT_LOCALES.includes(aspirationalLang) ? aspirationalLang : 'en-GB'}`;

  // Get display name for aspirational language
  $: aspirationalLangName = AVAILABLE_LOCALES.find(l => l.code === aspirationalLang)?.name || 'Greek';

  function openLightbox(src: string, alt: string) {
    lightboxImage = { src, alt };
    document.body.style.overflow = 'hidden';
  }

  function closeLightbox() {
    lightboxImage = null;
    document.body.style.overflow = '';
  }

  function handleLightboxKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeLightbox();
    }
  }

  // Carousel screenshots - now use dynamic paths
  $: carouselScreenshots = {
    light: [
      { src: `${screenshotBase}/01-main-interface-light.png`, alt: 'Main Interface' },
      { src: `${screenshotBase}/01-main-interface-light-preview.png`, alt: 'Preview Mode' },
      { src: `${screenshotBase}/01-main-interface-light-travel-preview.png`, alt: 'Travel Itinerary - Preview' },
      { src: `${screenshotBase}/01-main-interface-light-travel.png`, alt: 'Travel Itinerary - Edit' },
    ],
    dark: [
      { src: `${screenshotBase}/01-main-interface-dark.png`, alt: 'Main Interface - Dark' },
      { src: `${screenshotBase}/01-main-interface-dark-preview.png`, alt: 'Preview Mode - Dark' },
      { src: `${screenshotBase}/01-main-interface-dark-travel-preview.png`, alt: 'Travel Itinerary - Preview - Dark' },
      { src: `${screenshotBase}/01-main-interface-dark-travel.png`, alt: 'Travel Itinerary - Edit - Dark' },
    ],
  };

  $: currentScreenshots = isDarkMode ? carouselScreenshots.dark : carouselScreenshots.light;

  function nextCarouselSlide() {
    currentCarouselSlide = (currentCarouselSlide + 1) % 4;
  }

  function goToCarouselSlide(index: number) {
    currentCarouselSlide = index;
  }

  function startCarousel() {
    // Auto-advance every 10 seconds
    carouselInterval = setInterval(nextCarouselSlide, 10000);
  }

  function stopCarousel() {
    if (carouselInterval) {
      clearInterval(carouselInterval);
    }
  }

  // Detect theme on mount and start carousel
  onMount(() => {
    const updateTheme = () => {
      // Check if document has dark class
      isDarkMode = document.documentElement.classList.contains('dark');
    };

    // Initial check
    updateTheme();

    // Watch for theme changes
    const observer = new MutationObserver(updateTheme);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });

    // Start carousel
    startCarousel();

    return () => {
      observer.disconnect();
      stopCarousel();
    };
  });

  const features = [
    { icon: '💾', key: 'localFirst' },
    { icon: '🔍', key: 'powerfulSearch' },
    { icon: '🏷️', key: 'tagging' },
    { icon: '📎', key: 'attachments' },
    { icon: '☁️', key: 'optionalSync' },
    { icon: '⌨️', key: 'keyboard' },
    { icon: '🎨', key: 'categories' },
    { icon: '🗂️', key: 'encrypted' },
  ];

  // Web features - paths are now relative, base is added via getScreenshot
  const webFeatureKeys = [
    { key: 'richEditor', icon: '✏️', light: '02-rich-editor-light.png', dark: '02-rich-editor-dark.png' },
    { key: 'multiSelect', icon: '☑️', light: '03-multi-select-light.png', dark: '03-multi-select-dark.png' },
    { key: 'versions', icon: '📜', light: '04-version-history-light.png', dark: '04-version-history-dark.png' },
    { key: 'calculator', icon: '🧮', light: '05-calculator-light.png', dark: '05-calculator-dark.png' },
    { key: 'outliner', icon: '📋', light: '09-outliner-light.png', dark: '09-outliner-dark.png' },
    { key: 'multilingual', icon: '🌍', light: '01-main-interface-light.png', dark: '01-main-interface-dark.png', useAspirational: true },
  ];

  // Build webFeatures reactively based on current locale
  $: webFeatures = webFeatureKeys.map(f => ({
    key: f.key,
    icon: f.icon,
    screenshotLight: f.useAspirational
      ? `${aspirationalScreenshotBase}/${f.light}`
      : `${screenshotBase}/${f.light}`,
    screenshotDark: f.useAspirational
      ? `${aspirationalScreenshotBase}/${f.dark}`
      : `${screenshotBase}/${f.dark}`,
  }));

  // TUI features - use dynamic paths
  $: tuiFeatures = [
    { key: 'terminal', icon: '💻', screenshot: `${screenshotBase}/tui-interface.gif` },
    { key: 'cli', icon: '⚡', screenshot: `${screenshotBase}/tui-cli.gif` },
    { key: 'piping', icon: '🔗', screenshot: `${screenshotBase}/tui-piping.gif` },
    { key: 'crossPlatform', icon: '🌐', screenshot: `${screenshotBase}/tui-sync.gif` },
  ];

  // Mobile screenshots - use dynamic paths
  $: mobileScreenshots = [
    { key: 'list', screenshotLight: `${screenshotBase}/06-mobile-list-light.png`, screenshotDark: `${screenshotBase}/06-mobile-list-dark.png` },
    { key: 'travel', screenshotLight: `${screenshotBase}/07-mobile-travel-light.png`, screenshotDark: `${screenshotBase}/07-mobile-travel-dark.png` },
    { key: 'calculator', screenshotLight: `${screenshotBase}/08-mobile-calculator-light.png`, screenshotDark: `${screenshotBase}/08-mobile-calculator-dark.png` },
  ];

  // Helper to get the appropriate mobile screenshot based on theme
  $: getMobileScreenshot = (item: { screenshotLight: string; screenshotDark: string }) => {
    return isDarkMode ? item.screenshotDark : item.screenshotLight;
  };

  // Helper to get the appropriate screenshot based on theme
  $: getScreenshot = (feature: WebFeature) => {
    if (isDarkMode && feature.screenshotDark) {
      return feature.screenshotDark;
    }
    return feature.screenshotLight;
  };
</script>

<div class="landing-page">
  <!-- Language Picker (floating in header area) -->
  <div class="language-picker-container">
    <LanguagePicker />
  </div>

  <!-- Hero Section -->
  <section class="hero">
    <div class="container">
      <div class="hero-content">
        <h1 class="hero-title">
          {$_('landing.hero.title')}
        </h1>
        <p class="hero-subtitle">
          {$_('landing.hero.subtitle')}
        </p>
        <button
          class="cta-button"
          on:click={onGetStarted}
        >
          {$_('landing.hero.cta')}
        </button>
        <p class="hero-note">
          {$_('landing.hero.note')}
        </p>
      </div>

      <div class="hero-screenshot">
        <div
          class="carousel-container"
          on:mouseenter={stopCarousel}
          on:mouseleave={startCarousel}
          role="region"
          aria-label="Screenshot carousel"
        >
          {#each currentScreenshots as screenshot, index}
            <div
              class="carousel-slide"
              class:active={currentCarouselSlide === index}
            >
              <button
                type="button"
                class="screenshot-button"
                on:click={() => openLightbox(screenshot.src, screenshot.alt)}
                aria-label="View full size: {screenshot.alt}"
              >
                <img
                  src={screenshot.src}
                  alt={screenshot.alt}
                  class="hero-image"
                />
              </button>
            </div>
          {/each}

          <div class="carousel-indicators">
            {#each currentScreenshots as _, index}
              <button
                class="indicator"
                class:active={currentCarouselSlide === index}
                on:click={() => goToCarouselSlide(index)}
                aria-label={`Go to slide ${index + 1}`}
              ></button>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- Features Grid -->
  <section class="features-grid">
    <div class="container">
      <h2 class="section-title">{$_('landing.features.title')}</h2>
      <div class="grid">
        {#each features as feature}
          <div class="feature-card">
            <h3 class="feature-title"><span class="feature-icon">{feature.icon}</span> {$_(`landing.features.${feature.key}.title`)}</h3>
            <p class="feature-description">{$_(`landing.features.${feature.key}.description`)}</p>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <!-- Web Features -->
  <section class="web-features">
    <div class="container">
      <h2 class="section-title">{$_('landing.webFeatures.title')}</h2>
      <p class="section-subtitle">{$_('landing.webFeatures.subtitle')}</p>

      <div class="features-showcase">
        {#each webFeatures as feature, index}
          <div class="showcase-item" class:reverse={index % 2 === 1}>
            <div class="showcase-content">
              <h3 class="showcase-title"><span class="showcase-icon">{feature.icon}</span> {$_(`landing.webFeatures.${feature.key}.title`)}</h3>
              <p class="showcase-description">{$_(`landing.webFeatures.${feature.key}.description`)}</p>
              <ul class="showcase-list">
                {#each $_(`landing.webFeatures.${feature.key}.points`) as point}
                  <li>{point}</li>
                {/each}
              </ul>
            </div>
            <div class="showcase-screenshot">
              {#if getScreenshot(feature)}
                <button
                  type="button"
                  class="screenshot-button"
                  on:click={() => openLightbox(getScreenshot(feature), $_(`landing.webFeatures.${feature.key}.screenshotDescription`))}
                  aria-label="View full size: {$_(`landing.webFeatures.${feature.key}.screenshotDescription`)}"
                >
                  <img
                    src={getScreenshot(feature)}
                    alt={$_(`landing.webFeatures.${feature.key}.screenshotDescription`)}
                    class="showcase-image"
                  />
                </button>
              {:else}
                <ScreenshotPlaceholder
                  width={800}
                  height={600}
                  description={$_(`landing.webFeatures.${feature.key}.screenshotDescription`)}
                  icon={feature.icon}
                />
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <!-- TUI Features -->
  <section class="tui-features">
    <div class="container">
      <h2 class="section-title">{$_('landing.tuiFeatures.title')}</h2>
      <p class="section-subtitle">{$_('landing.tuiFeatures.subtitle')}</p>

      <div class="tui-grid">
        {#each tuiFeatures as feature}
          <div class="tui-card">
            <div class="tui-screenshot">
              {#if feature.screenshot}
                <button
                  type="button"
                  class="screenshot-button"
                  on:click={() => openLightbox(feature.screenshot, $_(`landing.tuiFeatures.${feature.key}.screenshotDescription`))}
                  aria-label="View full size: {$_(`landing.tuiFeatures.${feature.key}.screenshotDescription`)}"
                >
                  <img
                    src={feature.screenshot}
                    alt={$_(`landing.tuiFeatures.${feature.key}.screenshotDescription`)}
                    class="screenshot-image"
                  />
                </button>
              {:else}
                <ScreenshotPlaceholder
                  width={800}
                  height={500}
                  description={$_(`landing.tuiFeatures.${feature.key}.screenshotDescription`)}
                  icon={feature.icon}
                />
              {/if}
            </div>
            <div class="tui-content">
              <h3 class="tui-title"><span class="tui-icon">{feature.icon}</span> {$_(`landing.tuiFeatures.${feature.key}.title`)}</h3>
              <p class="tui-description">{$_(`landing.tuiFeatures.${feature.key}.description`)}</p>
              {#if $_(`landing.tuiFeatures.${feature.key}.code`) && !$_(`landing.tuiFeatures.${feature.key}.code`).startsWith('landing.')}
                <pre class="tui-code"><code>{$_(`landing.tuiFeatures.${feature.key}.code`)}</code></pre>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <!-- Mobile Features -->
  <section class="mobile-features">
    <div class="container">
      <h2 class="section-title">{$_('landing.mobileFeatures.title')}</h2>
      <p class="section-subtitle">{$_('landing.mobileFeatures.subtitle')}</p>

      <div class="mobile-grid">
        {#each mobileScreenshots as item}
          <div class="mobile-card">
            <div class="mobile-screenshot">
              <button
                type="button"
                class="screenshot-button"
                on:click={() => openLightbox(getMobileScreenshot(item), $_(`landing.mobileFeatures.${item.key}.alt`))}
                aria-label="View full size: {$_(`landing.mobileFeatures.${item.key}.alt`)}"
              >
                <img
                  src={getMobileScreenshot(item)}
                  alt={$_(`landing.mobileFeatures.${item.key}.alt`)}
                  class="mobile-image"
                />
              </button>
            </div>
            <div class="mobile-caption">
              {$_(`landing.mobileFeatures.${item.key}.caption`)}
            </div>
          </div>
        {/each}
      </div>
    </div>
  </section>

  <!-- Security & Privacy -->
  <section class="security">
    <div class="container">
      <div class="security-content">
        <h2 class="section-title">{$_('landing.security.title')}</h2>
        <p class="security-lead">{$_('landing.security.lead')}</p>

        <div class="security-grid">
          <div class="security-item">
            <h3 class="security-item-title"><span class="security-icon">🔐</span> {$_('landing.security.encryption.title')}</h3>
            <p class="security-item-description">{$_('landing.security.encryption.description')}</p>
          </div>
          <div class="security-item">
            <h3 class="security-item-title"><span class="security-icon">🏠</span> {$_('landing.security.selfHosted.title')}</h3>
            <p class="security-item-description">{$_('landing.security.selfHosted.description')}</p>
          </div>
          <div class="security-item">
            <h3 class="security-item-title"><span class="security-icon">🚫</span> {$_('landing.security.noTracking.title')}</h3>
            <p class="security-item-description">{$_('landing.security.noTracking.description')}</p>
          </div>
        </div>
      </div>
    </div>
  </section>

  <!-- Final CTA -->
  <section class="final-cta">
    <div class="container">
      <h2 class="cta-title">{$_('landing.finalCta.title')}</h2>
      <p class="cta-subtitle">{$_('landing.finalCta.subtitle')}</p>
      <button
        class="cta-button cta-button-large"
        on:click={onGetStarted}
      >
        {$_('landing.hero.cta')}
      </button>
      <div class="cta-links">
        <a
          href="https://github.com/seesee/jottery"
          target="_blank"
          rel="noopener noreferrer"
          class="cta-link"
        >
          <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
            <path fill-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z" clip-rule="evenodd"/>
          </svg>
          {$_('landing.finalCta.viewOnGitHub')}
        </a>
      </div>
    </div>
  </section>

  <!-- Lightbox Modal -->
  {#if lightboxImage}
    <div
      class="lightbox-overlay"
      on:click={closeLightbox}
      on:keydown={handleLightboxKeydown}
      role="dialog"
      aria-modal="true"
      aria-label="Full size image view"
      tabindex="-1"
    >
      <button
        type="button"
        class="lightbox-close"
        on:click={closeLightbox}
        aria-label="Close lightbox"
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <div class="lightbox-content" on:click|stopPropagation>
        <img
          src={lightboxImage.src}
          alt={lightboxImage.alt}
          class="lightbox-image"
        />
      </div>
    </div>
  {/if}
</div>

<style>
  .landing-page {
    background: #ffffff;
    min-height: 100vh;
    position: relative;
  }

  :global(.dark) .landing-page {
    background: #111827;
  }

  /* Language Picker Container */
  .language-picker-container {
    position: absolute;
    top: 1rem;
    right: 1rem;
    z-index: 50;
  }

  @media (min-width: 768px) {
    .language-picker-container {
      top: 1.5rem;
      right: 2rem;
    }
  }

  .container {
    max-width: 1280px;
    margin: 0 auto;
    padding: 0 1rem;
  }

  /* Hero Section */
  .hero {
    padding: 4rem 0;
    text-align: center;
  }

  @media (min-width: 768px) {
    .hero {
      padding: 6rem 0;
    }
  }

  .hero-content {
    margin-bottom: 3rem;
  }

  .hero-title {
    font-size: 2.5rem;
    font-weight: 800;
    line-height: 1.2;
    margin-bottom: 1.5rem;
    color: #111827;
  }

  @media (min-width: 768px) {
    .hero-title {
      font-size: 4rem;
    }
  }

  :global(.dark) .hero-title {
    color: #f9fafb;
  }

  .hero-subtitle {
    font-size: 1.25rem;
    line-height: 1.75;
    color: #64748b;
    max-width: 48rem;
    margin: 0 auto 2rem;
  }

  @media (min-width: 768px) {
    .hero-subtitle {
      font-size: 1.5rem;
    }
  }

  :global(.dark) .hero-subtitle {
    color: #94a3b8;
  }

  .cta-button {
    display: inline-block;
    padding: 1rem 2rem;
    font-size: 1.125rem;
    font-weight: 600;
    color: white;
    background: #2563eb;
    border: none;
    border-radius: 0.5rem;
    cursor: pointer;
    transition: background 0.2s;
  }

  .cta-button:hover {
    background: #1d4ed8;
  }

  .cta-button-large {
    padding: 1.25rem 3rem;
    font-size: 1.25rem;
  }

  .hero-note {
    margin-top: 1rem;
    font-size: 0.875rem;
    color: #64748b;
  }

  :global(.dark) .hero-note {
    color: #94a3b8;
  }

  .hero-screenshot {
    max-width: 1200px;
    margin: 0 auto;
  }

  .hero-image {
    width: 100%;
    height: auto;
    border-radius: 0.75rem;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
    transition: transform 0.3s, box-shadow 0.3s;
  }

  .hero-image:hover {
    transform: translateY(-4px);
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
  }

  :global(.dark) .hero-image {
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.5), 0 10px 10px -5px rgba(0, 0, 0, 0.3);
  }

  /* Carousel */
  .carousel-container {
    position: relative;
    width: 100%;
  }

  .carousel-slide {
    opacity: 0;
    transition: opacity 1s ease-in-out;
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    pointer-events: none;
  }

  .carousel-slide.active {
    opacity: 1;
    position: relative;
    pointer-events: auto;
  }

  .carousel-indicators {
    position: absolute;
    bottom: 1.5rem;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    gap: 0.75rem;
    z-index: 10;
  }

  .indicator {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.6);
    border: 2px solid rgba(255, 255, 255, 0.9);
    cursor: pointer;
    padding: 0;
    transition: all 0.3s;
  }

  .indicator:hover {
    background: rgba(255, 255, 255, 0.8);
    transform: scale(1.1);
  }

  .indicator.active {
    background: rgba(255, 255, 255, 1);
    transform: scale(1.3);
  }

  :global(.dark) .indicator {
    background: rgba(100, 116, 139, 0.6);
    border-color: rgba(148, 163, 184, 0.9);
  }

  :global(.dark) .indicator:hover {
    background: rgba(148, 163, 184, 0.8);
  }

  :global(.dark) .indicator.active {
    background: rgba(148, 163, 184, 1);
  }

  /* Features Grid */
  .features-grid {
    padding: 4rem 0;
    background: #f9fafb;
  }

  :global(.dark) .features-grid {
    background: #1f2937;
  }

  .section-title {
    font-size: 1.875rem;
    font-weight: 700;
    text-align: center;
    margin-bottom: 1rem;
    color: #111827;
  }

  @media (min-width: 768px) {
    .section-title {
      font-size: 2.25rem;
    }
  }

  :global(.dark) .section-title {
    color: #f9fafb;
  }

  .section-subtitle {
    text-align: center;
    font-size: 1.125rem;
    color: #64748b;
    max-width: 48rem;
    margin: 0 auto 3rem;
  }

  :global(.dark) .section-subtitle {
    color: #94a3b8;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2rem;
    margin-top: 3rem;
  }

  @media (min-width: 640px) {
    .grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  @media (min-width: 1024px) {
    .grid {
      grid-template-columns: repeat(4, 1fr);
    }
  }

  .feature-card {
    padding: 1.5rem;
    background: #ffffff;
    border-radius: 0.5rem;
    border: 1px solid #e5e7eb;
  }

  :global(.dark) .feature-card {
    background: #111827;
    border-color: #374151;
  }

  .feature-icon {
    font-size: 1.125rem;
  }

  .feature-title {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: #111827;
  }

  :global(.dark) .feature-title {
    color: #f9fafb;
  }

  .feature-description {
    font-size: 0.875rem;
    line-height: 1.5;
    color: #64748b;
  }

  :global(.dark) .feature-description {
    color: #94a3b8;
  }

  /* Web Features */
  .web-features {
    padding: 4rem 0;
  }

  .features-showcase {
    margin-top: 4rem;
    display: flex;
    flex-direction: column;
    gap: 4rem;
  }

  .showcase-item {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2rem;
    align-items: center;
  }

  @media (min-width: 1024px) {
    .showcase-item {
      grid-template-columns: 1fr 1fr;
    }

    .showcase-item.reverse {
      direction: rtl;
    }

    .showcase-item.reverse > * {
      direction: ltr;
    }
  }

  .showcase-content {
    padding: 2rem;
  }

  .showcase-screenshot {
    overflow: hidden;
    border-radius: 0.75rem;
  }

  .showcase-image {
    width: 100%;
    height: auto;
    border-radius: 0.75rem;
    box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05);
    transition: transform 0.3s, box-shadow 0.3s;
  }

  .showcase-image:hover {
    transform: scale(1.02);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  :global(.dark) .showcase-image {
    box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.3), 0 4px 6px -2px rgba(0, 0, 0, 0.2);
  }

  .showcase-icon {
    font-size: 1.5rem;
  }

  .showcase-title {
    font-size: 1.5rem;
    font-weight: 600;
    margin-bottom: 1rem;
    color: #111827;
  }

  :global(.dark) .showcase-title {
    color: #f9fafb;
  }

  .showcase-description {
    font-size: 1.125rem;
    line-height: 1.75;
    color: #64748b;
    margin-bottom: 1.5rem;
  }

  :global(.dark) .showcase-description {
    color: #94a3b8;
  }

  .showcase-list {
    list-style: none;
    padding: 0;
  }

  .showcase-list li {
    padding-left: 1.5rem;
    margin-bottom: 0.75rem;
    color: #475569;
    position: relative;
  }

  .showcase-list li::before {
    content: "✓";
    position: absolute;
    left: 0;
    color: #059669;
    font-weight: 600;
  }

  :global(.dark) .showcase-list li {
    color: #cbd5e1;
  }

  /* TUI Features */
  .tui-features {
    padding: 4rem 0;
    background: #f9fafb;
  }

  :global(.dark) .tui-features {
    background: #1f2937;
  }

  .tui-grid {
    margin-top: 3rem;
    display: grid;
    grid-template-columns: 1fr;
    gap: 3rem;
  }

  @media (min-width: 768px) {
    .tui-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .tui-card {
    background: #ffffff;
    border-radius: 0.5rem;
    overflow: hidden;
    border: 1px solid #e5e7eb;
  }

  :global(.dark) .tui-card {
    background: #111827;
    border-color: #374151;
  }

  .tui-screenshot {
    width: 100%;
  }

  .screenshot-image {
    width: 100%;
    height: auto;
    display: block;
  }

  .tui-content {
    padding: 2rem;
  }

  .tui-icon {
    font-size: 1.25rem;
  }

  .tui-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #111827;
  }

  :global(.dark) .tui-title {
    color: #f9fafb;
  }

  .tui-description {
    font-size: 1rem;
    line-height: 1.625;
    color: #64748b;
    margin-bottom: 1rem;
  }

  :global(.dark) .tui-description {
    color: #94a3b8;
  }

  .tui-code {
    background: #1e293b;
    color: #e2e8f0;
    padding: 1rem;
    border-radius: 0.5rem;
    font-size: 0.875rem;
    overflow-x: auto;
  }

  :global(.dark) .tui-code {
    background: #0f172a;
  }

  /* Mobile Features */
  .mobile-features {
    padding: 4rem 0;
  }

  .mobile-grid {
    margin-top: 3rem;
    display: flex;
    justify-content: center;
    gap: 2rem;
    flex-wrap: wrap;
  }

  .mobile-card {
    flex: 0 0 auto;
    max-width: 280px;
    text-align: center;
  }

  .mobile-screenshot {
    border-radius: 1.5rem;
    overflow: hidden;
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.15), 0 10px 10px -5px rgba(0, 0, 0, 0.08);
    background: #1e293b;
    padding: 0.5rem;
  }

  :global(.dark) .mobile-screenshot {
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.4), 0 10px 10px -5px rgba(0, 0, 0, 0.3);
  }

  .mobile-image {
    width: 100%;
    height: auto;
    border-radius: 1rem;
    display: block;
  }

  .mobile-caption {
    margin-top: 1rem;
    font-size: 1rem;
    font-weight: 500;
    color: #475569;
  }

  :global(.dark) .mobile-caption {
    color: #94a3b8;
  }

  /* Security Section */
  .security {
    padding: 4rem 0;
  }

  .security-content {
    text-align: center;
  }

  .security-lead {
    font-size: 1.25rem;
    line-height: 1.75;
    color: #64748b;
    max-width: 48rem;
    margin: 0 auto 3rem;
  }

  :global(.dark) .security-lead {
    color: #94a3b8;
  }

  .security-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 2rem;
    margin-top: 3rem;
  }

  @media (min-width: 768px) {
    .security-grid {
      grid-template-columns: repeat(3, 1fr);
    }
  }

  .security-item {
    padding: 1.5rem;
    background: #f9fafb;
    border-radius: 0.5rem;
    border: 1px solid #e5e7eb;
  }

  :global(.dark) .security-item {
    background: #1f2937;
    border-color: #374151;
  }

  .security-icon {
    font-size: 1.125rem;
  }

  .security-item-title {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #111827;
  }

  :global(.dark) .security-item-title {
    color: #f9fafb;
  }

  .security-item-description {
    font-size: 0.875rem;
    line-height: 1.625;
    color: #64748b;
  }

  :global(.dark) .security-item-description {
    color: #94a3b8;
  }

  /* Final CTA */
  .final-cta {
    padding: 4rem 0;
    background: #111827;
    text-align: center;
    color: white;
  }

  :global(.dark) .final-cta {
    background: #030712;
  }

  .cta-title {
    font-size: 1.875rem;
    font-weight: 700;
    margin-bottom: 1rem;
    color: white;
  }

  @media (min-width: 768px) {
    .cta-title {
      font-size: 2.25rem;
    }
  }

  .cta-subtitle {
    font-size: 1.125rem;
    line-height: 1.75;
    margin-bottom: 2rem;
    color: #9ca3af;
  }

  .final-cta .cta-button {
    background: #2563eb;
    color: white;
  }

  .final-cta .cta-button:hover {
    background: #1d4ed8;
  }

  .cta-links {
    margin-top: 2rem;
    display: flex;
    justify-content: center;
    gap: 2rem;
  }

  .cta-link {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    color: white;
    text-decoration: none;
    font-weight: 500;
    transition: opacity 0.2s;
  }

  .cta-link:hover {
    opacity: 0.8;
  }

  /* Screenshot Button */
  .screenshot-button {
    display: block;
    width: 100%;
    padding: 0;
    margin: 0;
    border: none;
    background: none;
    cursor: zoom-in;
    transition: transform 0.2s;
  }

  .screenshot-button:hover {
    transform: scale(1.01);
  }

  .screenshot-button:focus {
    outline: 2px solid #2563eb;
    outline-offset: 4px;
    border-radius: 0.5rem;
  }

  /* Lightbox */
  .lightbox-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    z-index: 9999;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    cursor: zoom-out;
    animation: lightbox-fade-in 0.2s ease-out;
  }

  @keyframes lightbox-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .lightbox-close {
    position: absolute;
    top: 1rem;
    right: 1rem;
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.1);
    border: none;
    border-radius: 50%;
    color: white;
    cursor: pointer;
    transition: background 0.2s, transform 0.2s;
    z-index: 10001;
  }

  .lightbox-close:hover {
    background: rgba(255, 255, 255, 0.2);
    transform: scale(1.1);
  }

  .lightbox-close:focus {
    outline: 2px solid white;
    outline-offset: 2px;
  }

  .lightbox-content {
    max-width: 95vw;
    max-height: 95vh;
    cursor: default;
    animation: lightbox-zoom-in 0.2s ease-out;
  }

  @keyframes lightbox-zoom-in {
    from {
      transform: scale(0.95);
      opacity: 0;
    }
    to {
      transform: scale(1);
      opacity: 1;
    }
  }

  .lightbox-image {
    max-width: 100%;
    max-height: 90vh;
    object-fit: contain;
    border-radius: 0.5rem;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
  }

</style>
