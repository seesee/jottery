/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{svelte,js,ts}'],
  theme: {
    extend: {
      screens: {
        // Mobile-first breakpoints
        // mobile: < 768px (default, no prefix needed)
        // tablet: 768px - 1023px (use md:)
        // desktop: >= 1024px (use lg:)
        'tablet': '768px',
        'desktop': '1024px',
      },
    },
  },
  plugins: [],
  darkMode: 'class',
};
