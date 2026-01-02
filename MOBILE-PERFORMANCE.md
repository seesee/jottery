# Mobile Performance Testing & Optimization

## Current Baseline

**Build Stats (2026-01-02):**
- Bundle size: 1,938.62 KB (uncompressed)
- Gzipped: 555.84 KB
- Build tool: Vite 7.3.0 with vite-plugin-singlefile

**Major Dependencies:**
- CodeMirror 6 + 8 language packs
- highlight.js (markdown preview syntax highlighting)
- pdfjs-dist (PDF preview)
- marked (markdown parsing)
- flexsearch (full-text search)
- lucide-svelte (icons)

## Performance Testing Checklist

### 1. Bundle Size Analysis

- [x] Current baseline recorded: 555.84 KB gzipped
- [x] Analyze bundle composition with rollup-plugin-visualizer
- [ ] Identify largest dependencies
- [ ] Check for duplicate dependencies
- [ ] Measure impact of each major library

### 2. Mobile Device Testing

#### iOS Safari
- [ ] iPhone 15 Pro (latest iOS)
- [ ] iPhone 12 (iOS 16)
- [ ] iPad (latest)
- [ ] Measure:
  - [ ] Initial load time
  - [ ] Time to interactive
  - [ ] Note list scroll performance
  - [ ] Editor typing lag
  - [ ] Search responsiveness
  - [ ] Attachment preview performance

#### Android Chrome
- [ ] Pixel 8 (Android 14)
- [ ] Samsung Galaxy S21 (Android 13)
- [ ] Mid-range device (e.g., Pixel 4a)
- [ ] Measure same metrics as iOS

#### Network Conditions
- [ ] WiFi (fast)
- [ ] 4G (moderate)
- [ ] 3G (slow)
- [ ] Offline mode

### 3. Performance Metrics

#### Core Web Vitals
- [ ] Largest Contentful Paint (LCP) - Target: <2.5s
- [ ] First Input Delay (FID) - Target: <100ms
- [ ] Cumulative Layout Shift (CLS) - Target: <0.1
- [ ] Time to Interactive (TTI) - Target: <3.5s

#### App-Specific Metrics
- [ ] Note list render time with 100 notes
- [ ] Note list render time with 1000 notes
- [ ] Note list render time with 5000 notes
- [ ] Search query response time
- [ ] Editor load time
- [ ] Markdown preview generation time
- [ ] Attachment upload/preview time
- [ ] Virtual scroll smoothness (60fps target)

### 4. Optimization Opportunities

#### Bundle Size Reduction
- [ ] **Lazy load PDF.js** - Only load when PDF is previewed (~1MB saving)
- [ ] **Lazy load highlight.js** - Only load when markdown preview is active
- [ ] **Code split language packs** - Load CodeMirror languages on demand
- [ ] **Tree-shake lucide icons** - Only bundle used icons
- [ ] **Remove unused i18n translations** - Only bundle active languages

#### Code Splitting Strategy
```javascript
// Example: Lazy load PdfViewer
const PdfViewer = lazy(() => import('./PdfViewer.svelte'));

// Example: Lazy load highlight.js
const loadHighlightJs = () => import('highlight.js');
```

#### Runtime Optimizations
- [ ] Debounce expensive operations (search, auto-save)
- [ ] Throttle scroll handlers
- [ ] Use passive event listeners where possible
- [ ] Optimize virtual scrolling calculations
- [ ] Reduce re-renders with proper memoization
- [ ] Optimize IndexedDB batch operations

#### Animation Performance
- [ ] Use CSS transforms instead of position changes
- [ ] Use will-change for animated elements
- [ ] Limit concurrent animations
- [ ] Test all transitions for 60fps on mobile
- [ ] Reduce animation duration on low-end devices

### 5. Large Dataset Testing

#### Test Scenarios
- [ ] 100 notes: Baseline performance
- [ ] 1,000 notes: Typical heavy user
- [ ] 5,000 notes: Stress test (user's current count: 3,069)
- [ ] 10,000 notes: Maximum scale test

#### Metrics per Dataset
- [ ] Initial load time
- [ ] Search index build time
- [ ] Search query time
- [ ] Virtual scroll performance
- [ ] Memory usage
- [ ] Battery impact (measure over 30 min)

### 6. Offline Functionality

- [ ] App loads without network
- [ ] Notes are accessible offline
- [ ] Create/edit/delete works offline
- [ ] Search works offline
- [ ] Attachments viewable offline
- [ ] Sync queues changes when offline
- [ ] Reconnection handling

### 7. Screen Sizes & Orientations

#### Screen Sizes (min-width)
- [ ] 320px - iPhone SE
- [ ] 375px - iPhone 12/13 Mini
- [ ] 390px - iPhone 14/15
- [ ] 414px - iPhone 14/15 Plus
- [ ] 768px - iPad Portrait
- [ ] 820px - iPad Air
- [ ] 1024px - iPad Landscape

#### Orientations
- [ ] Portrait mode: All features accessible
- [ ] Landscape mode: Optimal layout
- [ ] Rotation handling: No data loss, smooth transition

### 8. Touch Interactions

- [ ] Tap targets: Minimum 44x44px
- [ ] Swipe gestures: Smooth, no lag
- [ ] Long press: Consistent feedback
- [ ] Pinch zoom: Works on images/PDFs
- [ ] Pull-to-refresh: Disabled to avoid conflicts
- [ ] Touch scrolling: Momentum preserved

## Optimization Plan Priority

### Phase 1: Quick Wins (This Session)
1. ✅ Add bundle analyzer to see composition
2. ✅ Lazy load PDF.js (only load when preview is opened)
3. ✅ Lazy load highlight.js (only load for markdown preview)
4. Test on simulator/emulator with DevTools

**Findings:**
- Lazy loading implemented for PDF.js and highlight.js
- Single-file build (vite-plugin-singlefile) prevents download size reduction
- Benefits: Parse time, execution time, memory usage (not download size)
- Current bundle: 555.78 KB gzipped (unchanged from 555.84 KB)

**Single-File Build Trade-offs:**
- ✅ Pros: Easy deployment, works offline, no CDN dependencies
- ❌ Cons: Cannot reduce initial download via code splitting
- Alternative: Multi-file build with code splitting would reduce initial load by ~200-300KB

### Phase 2: Code Splitting (Next Session)
1. Split CodeMirror language packs
2. Lazy load settings modal
3. Lazy load recycle bin modal
4. Lazy load version history modal

### Phase 3: Runtime Optimization (Following Session)
1. Profile with Chrome DevTools Performance tab
2. Optimize virtual scrolling calculations
3. Reduce unnecessary re-renders
4. Optimize search indexing

### Phase 4: Real Device Testing (When Ready)
1. Test on actual iOS/Android devices
2. Measure metrics with Lighthouse Mobile
3. Record videos for visual performance analysis
4. Gather user feedback on performance

## Tools & Resources

### Analysis Tools
- Chrome DevTools (Performance, Network, Coverage tabs)
- Lighthouse Mobile audit
- rollup-plugin-visualizer for bundle analysis
- Safari Web Inspector (iOS testing)
- Chrome Remote Debugging (Android testing)

### Performance Testing
- WebPageTest.org (mobile simulation)
- Chrome DevTools Device Emulation
- BrowserStack (real device testing - if available)

### Monitoring
- Built-in browser Performance API
- Custom timing markers for key operations
- IndexedDB performance monitoring

## Success Criteria

### Must Have (P1)
- [ ] Bundle size under 600 KB gzipped
- [ ] Time to Interactive under 3.5s on 4G
- [ ] 60fps scroll performance with 1000 notes
- [ ] Search responds in <200ms for typical query
- [ ] All features work offline

### Should Have (P2)
- [ ] Bundle size under 500 KB gzipped
- [ ] LCP under 2.5s on 4G
- [ ] Smooth animations on mid-range Android
- [ ] Memory usage under 100MB for 1000 notes

### Nice to Have (P3)
- [ ] Bundle size under 400 KB gzipped
- [ ] Works smoothly on 3G
- [ ] Supports 10,000 notes without degradation

## Session Log

### 2026-01-02 - Initial Analysis & Lazy Loading

**Completed:**
1. ✅ Installed rollup-plugin-visualizer for bundle analysis
2. ✅ Generated bundle visualization (dist/stats.html)
3. ✅ Implemented lazy loading for PDF.js in PdfViewer.svelte
4. ✅ Implemented lazy loading for highlight.js in EditorPane.svelte
5. ✅ Documented single-file build trade-offs

**Key Findings:**
- Bundle size: 555.78 KB gzipped (1,939.88 KB uncompressed)
- Single-file build prevents download size reduction via code splitting
- Lazy loading still provides runtime performance benefits:
  - Reduced parse time (code parsed on-demand)
  - Reduced memory usage (modules not initialized until needed)
  - Faster initial app startup

**Technical Details:**
- PDF.js: Dynamic import when PdfViewer component mounts
- highlight.js: Dynamic import when markdown preview first shown
- Both show loading indicators during module fetch

**Next Decisions:**
1. Keep single-file build for simplicity, accept ~556KB download
2. OR: Switch to multi-file build for ~200-300KB smaller initial load
3. Continue with Phase 2: Code splitting CodeMirror language packs
4. Test on actual mobile devices to measure real-world performance

## Next Steps

1. ~~Install bundle analyzer~~ ✅
2. ~~Generate bundle visualization~~ ✅
3. ~~Implement lazy loading for heavy dependencies~~ ✅
4. Decision: Single-file vs multi-file build strategy
5. Test on real devices (or best available simulators)
6. Measure Core Web Vitals on mobile
7. Document findings and iterate
