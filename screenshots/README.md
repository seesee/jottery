# Landing Page Screenshots

This directory contains screenshots used in the landing page placeholders.

## Generating Screenshots

Screenshots are automatically generated using Playwright E2E tests. To generate new screenshots:

```bash
# Install dependencies if needed
npm install

# Run the screenshot test suite (chromium only for consistency)
npx playwright test screenshots --project=chromium

# Or run individual screenshot tests
npx playwright test screenshots.spec.ts:01 --project=chromium
```

## Screenshot Inventory

The following screenshots are generated:

| Filename | Dimensions | Description | Used In |
|----------|------------|-------------|---------|
| `01-main-interface.png` | 1920x1080 | Main interface showing note list and editor | Hero section |
| `02-rich-editor.png` | 1920x1080 | Editor with Python syntax highlighting | Web Features - Rich Editor |
| `03-multi-select.png` | 1920x1080 | Multi-select mode with bulk operations toolbar | Web Features - Multi-Select |
| `04-search-results.png` | 1920x1080 | Search results with tag filter | Web Features (optional) |
| `05-calculator.png` | 1920x1080 | REPL calculator with example calculations | Web Features - Calculator |
| `06-categories-settings.png` | 1920x1080 | Settings panel showing color categories | Web Features (optional) |
| `07-mobile-view.png` | 375x667 | Mobile responsive view (iPhone SE) | Mobile showcase (optional) |
| `08-dark-mode.png` | 1920x1080 | Dark mode theme | Theme showcase (optional) |

## Using Screenshots in Landing Page

### Option 1: Replace ScreenshotPlaceholder Components

Edit `src/lib/components/LandingPage.svelte` to replace the `<ScreenshotPlaceholder>` components with actual images:

```svelte
<!-- Before -->
<ScreenshotPlaceholder
  width={1200}
  height={800}
  description={$_('landing.screenshots.mainInterface')}
  icon="🗒️"
/>

<!-- After -->
<img
  src="/screenshots/01-main-interface.png"
  alt={$_('landing.screenshots.mainInterface')}
  class="rounded-lg shadow-xl w-full h-auto"
/>
```

### Option 2: Copy Screenshots to Public Directory

```bash
# Copy screenshots to public directory
cp screenshots/*.png public/screenshots/

# Or create symlink
ln -s ../screenshots public/screenshots
```

### Option 3: Optimize Screenshots

For production, optimize the images:

```bash
# Install imagemagick or use online tools
brew install imagemagick  # macOS

# Resize and optimize
for file in screenshots/*.png; do
  convert "$file" -resize 1200x800 -quality 85 "screenshots/optimized-$(basename $file)"
done
```

## Regenerating Screenshots

Screenshots should be regenerated when:
- UI design changes significantly
- New features are added
- Color scheme or theme changes
- You want to update the landing page visuals

Simply run the test suite again:

```bash
npx playwright test screenshots --project=chromium
```

Old screenshots will be overwritten with fresh ones.

## Customization

To customize screenshot generation:

1. Edit `e2e/screenshots.spec.ts`
2. Modify viewport sizes, wait times, or interactions
3. Add new test cases for additional screenshots
4. Adjust screenshot dimensions in the `page.screenshot()` calls

## CI/CD Integration

To generate screenshots in CI/CD:

```yaml
# Example GitHub Actions workflow
- name: Generate Screenshots
  run: |
    npm ci
    npx playwright install chromium
    npx playwright test screenshots --project=chromium

- name: Upload Screenshots
  uses: actions/upload-artifact@v3
  with:
    name: landing-page-screenshots
    path: screenshots/
```

## Notes

- Screenshots are taken in Chromium for consistency
- Default viewport is 1920x1080 (desktop)
- Mobile screenshot uses 375x667 (iPhone SE)
- All screenshots use the test password: `screenshot-test-password`
- The test suite imports demo notes from `demo-generation/jottery-demo-notes.json`
