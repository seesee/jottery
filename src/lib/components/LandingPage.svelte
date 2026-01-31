<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import ScreenshotPlaceholder from './ScreenshotPlaceholder.svelte';

  /** Feature with light/dark screenshots */
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

  // Carousel screenshots
  const carouselScreenshots = {
    light: [
      { src: '/screenshots/en-GB/01-main-interface-light.png', alt: 'Main Interface' },
      { src: '/screenshots/en-GB/01-main-interface-light-preview.png', alt: 'Preview Mode' },
      { src: '/screenshots/en-GB/01-main-interface-light-japan-preview.png', alt: 'Japan Itinerary - Preview' },
      { src: '/screenshots/en-GB/01-main-interface-light-japan.png', alt: 'Japan Itinerary - Edit' },
    ],
    dark: [
      { src: '/screenshots/en-GB/01-main-interface-dark.png', alt: 'Main Interface - Dark' },
      { src: '/screenshots/en-GB/01-main-interface-dark-preview.png', alt: 'Preview Mode - Dark' },
      { src: '/screenshots/en-GB/01-main-interface-dark-japan-preview.png', alt: 'Japan Itinerary - Preview - Dark' },
      { src: '/screenshots/en-GB/01-main-interface-dark-japan.png', alt: 'Japan Itinerary - Edit - Dark' },
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

  const webFeatures = [
    {
      key: 'richEditor',
      icon: '✏️',
      screenshotLight: '/screenshots/en-GB/02-rich-editor-light.png',
      screenshotDark: '/screenshots/en-GB/02-rich-editor-dark.png',
    },
    {
      key: 'multiSelect',
      icon: '☑️',
      screenshotLight: '/screenshots/en-GB/03-multi-select-light.png',
      screenshotDark: '/screenshots/en-GB/03-multi-select-dark.png',
    },
    {
      key: 'versions',
      icon: '📜',
      screenshotLight: '/screenshots/en-GB/04-version-history-light.png',
      screenshotDark: '/screenshots/en-GB/04-version-history-dark.png',
    },
    {
      key: 'calculator',
      icon: '🧮',
      screenshotLight: '/screenshots/en-GB/05-calculator-light.png',
      screenshotDark: '/screenshots/en-GB/05-calculator-dark.png',
    },
  ];

  const tuiFeatures = [
    { key: 'terminal', icon: '💻', screenshot: '/screenshots/en-GB/tui-interface.gif' },
    { key: 'cli', icon: '⚡', screenshot: '/screenshots/en-GB/tui-cli.gif' },
    { key: 'piping', icon: '🔗', screenshot: '/screenshots/en-GB/tui-piping.gif' },
    { key: 'crossPlatform', icon: '🌐', screenshot: '/screenshots/en-GB/tui-sync.gif' },
  ];

  const mobileScreenshots = [
    { key: 'list', screenshotLight: '/screenshots/en-GB/06-mobile-list-light.png', screenshotDark: '/screenshots/en-GB/06-mobile-list-dark.png' },
    { key: 'japan', screenshotLight: '/screenshots/en-GB/07-mobile-japan-light.png', screenshotDark: '/screenshots/en-GB/07-mobile-japan-dark.png' },
    { key: 'calculator', screenshotLight: '/screenshots/en-GB/08-mobile-calculator-light.png', screenshotDark: '/screenshots/en-GB/08-mobile-calculator-dark.png' },
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
              <img
                src={screenshot.src}
                alt={screenshot.alt}
                class="hero-image"
              />
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
            <div class="feature-icon">{feature.icon}</div>
            <h3 class="feature-title">{$_(`landing.features.${feature.key}.title`)}</h3>
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
              <div class="showcase-icon">{feature.icon}</div>
              <h3 class="showcase-title">{$_(`landing.webFeatures.${feature.key}.title`)}</h3>
              <p class="showcase-description">{$_(`landing.webFeatures.${feature.key}.description`)}</p>
              <ul class="showcase-list">
                {#each $_(`landing.webFeatures.${feature.key}.points`) as point}
                  <li>{point}</li>
                {/each}
              </ul>
            </div>
            <div class="showcase-screenshot">
              {#if getScreenshot(feature)}
                <img
                  src={getScreenshot(feature)}
                  alt={$_(`landing.webFeatures.${feature.key}.screenshotDescription`)}
                  class="showcase-image"
                />
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
                <img
                  src={feature.screenshot}
                  alt={$_(`landing.tuiFeatures.${feature.key}.screenshotDescription`)}
                  class="screenshot-image"
                />
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
              <div class="tui-icon">{feature.icon}</div>
              <h3 class="tui-title">{$_(`landing.tuiFeatures.${feature.key}.title`)}</h3>
              <p class="tui-description">{$_(`landing.tuiFeatures.${feature.key}.description`)}</p>
              {#if $_(`landing.tuiFeatures.${feature.key}.code`)}
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
              <img
                src={getMobileScreenshot(item)}
                alt={$_(`landing.mobileFeatures.${item.key}.alt`)}
                class="mobile-image"
              />
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
            <div class="security-icon">🔐</div>
            <h3 class="security-item-title">{$_('landing.security.encryption.title')}</h3>
            <p class="security-item-description">{$_('landing.security.encryption.description')}</p>
          </div>
          <div class="security-item">
            <div class="security-icon">🏠</div>
            <h3 class="security-item-title">{$_('landing.security.selfHosted.title')}</h3>
            <p class="security-item-description">{$_('landing.security.selfHosted.description')}</p>
          </div>
          <div class="security-item">
            <div class="security-icon">🚫</div>
            <h3 class="security-item-title">{$_('landing.security.noTracking.title')}</h3>
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
</div>

<style>
  .landing-page {
    background: linear-gradient(to bottom, #f8fafc 0%, #ffffff 100%);
    min-height: 100vh;
  }

  :global(.dark) .landing-page {
    background: linear-gradient(to bottom, #0f172a 0%, #1e293b 100%);
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
    color: #1e293b;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  @media (min-width: 768px) {
    .hero-title {
      font-size: 4rem;
    }
  }

  :global(.dark) .hero-title {
    color: #f1f5f9;
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
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    border: none;
    border-radius: 0.75rem;
    cursor: pointer;
    transition: transform 0.2s, box-shadow 0.2s;
    box-shadow: 0 10px 15px -3px rgba(102, 126, 234, 0.4), 0 4px 6px -2px rgba(102, 126, 234, 0.2);
  }

  .cta-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 20px 25px -5px rgba(102, 126, 234, 0.4), 0 10px 10px -5px rgba(102, 126, 234, 0.2);
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
    background: white;
  }

  :global(.dark) .features-grid {
    background: #1e293b;
  }

  .section-title {
    font-size: 2rem;
    font-weight: 700;
    text-align: center;
    margin-bottom: 1rem;
    color: #1e293b;
  }

  @media (min-width: 768px) {
    .section-title {
      font-size: 3rem;
    }
  }

  :global(.dark) .section-title {
    color: #f1f5f9;
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
    padding: 2rem;
    background: #f8fafc;
    border-radius: 1rem;
    transition: transform 0.2s, box-shadow 0.2s;
  }

  .feature-card:hover {
    transform: translateY(-4px);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04);
  }

  :global(.dark) .feature-card {
    background: #0f172a;
  }

  .feature-icon {
    font-size: 3rem;
    margin-bottom: 1rem;
  }

  .feature-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.5rem;
    color: #1e293b;
  }

  :global(.dark) .feature-title {
    color: #f1f5f9;
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
    font-size: 3rem;
    margin-bottom: 1rem;
  }

  .showcase-title {
    font-size: 1.875rem;
    font-weight: 700;
    margin-bottom: 1rem;
    color: #1e293b;
  }

  :global(.dark) .showcase-title {
    color: #f1f5f9;
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
    color: #667eea;
    font-weight: 700;
  }

  :global(.dark) .showcase-list li {
    color: #cbd5e1;
  }

  /* TUI Features */
  .tui-features {
    padding: 4rem 0;
    background: white;
  }

  :global(.dark) .tui-features {
    background: #1e293b;
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
    background: #f8fafc;
    border-radius: 1rem;
    overflow: hidden;
    box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06);
  }

  :global(.dark) .tui-card {
    background: #0f172a;
  }

  .tui-screenshot {
    width: 100%;
  }

  .tui-content {
    padding: 2rem;
  }

  .tui-icon {
    font-size: 2.5rem;
    margin-bottom: 1rem;
  }

  .tui-title {
    font-size: 1.5rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #1e293b;
  }

  :global(.dark) .tui-title {
    color: #f1f5f9;
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
    padding: 2rem;
    background: linear-gradient(135deg, #667eea10 0%, #764ba210 100%);
    border-radius: 1rem;
    border: 2px solid #e2e8f0;
  }

  :global(.dark) .security-item {
    background: linear-gradient(135deg, #667eea20 0%, #764ba220 100%);
    border-color: #334155;
  }

  .security-icon {
    font-size: 3rem;
    margin-bottom: 1rem;
  }

  .security-item-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #1e293b;
  }

  :global(.dark) .security-item-title {
    color: #f1f5f9;
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
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    text-align: center;
    color: white;
  }

  .cta-title {
    font-size: 2.5rem;
    font-weight: 700;
    margin-bottom: 1rem;
    color: white;
  }

  @media (min-width: 768px) {
    .cta-title {
      font-size: 3rem;
    }
  }

  .cta-subtitle {
    font-size: 1.25rem;
    line-height: 1.75;
    margin-bottom: 2rem;
    opacity: 0.9;
  }

  .final-cta .cta-button {
    background: white;
    color: #764ba2;
    box-shadow: 0 10px 15px -3px rgba(0, 0, 0, 0.3), 0 4px 6px -2px rgba(0, 0, 0, 0.2);
  }

  .final-cta .cta-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.3), 0 10px 10px -5px rgba(0, 0, 0, 0.2);
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
</style>
