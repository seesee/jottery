# PWA Icons

The PWA requires the following icon files in the `public/` directory:

- `pwa-192x192.png` - 192x192 pixel PNG icon
- `pwa-512x512.png` - 512x512 pixel PNG icon
- `favicon.ico` - Standard favicon
- `apple-touch-icon.png` - 180x180 pixel PNG for iOS
- `mask-icon.svg` - SVG icon for Safari pinned tabs

## Quick Icon Generation

You can use one of these methods to generate icons:

### Method 1: Online Tools
- Use [Favicon Generator](https://realfavicongenerator.net/)
- Upload the `public/favicon.svg` file
- Download and extract all generated icons to `public/`

### Method 2: Using ImageMagick (if installed)
```bash
# From the project root:
convert public/favicon.svg -resize 192x192 public/pwa-192x192.png
convert public/favicon.svg -resize 512x512 public/pwa-512x512.png
convert public/favicon.svg -resize 180x180 public/apple-touch-icon.png
convert public/favicon.svg -resize 32x32 public/favicon.ico
cp public/favicon.svg public/mask-icon.svg
```

### Method 3: Manual Creation
Create PNG versions of the logo at the required sizes using any image editor (Figma, Photoshop, GIMP, etc.).

## Current Status
✅ `favicon.svg` - Base SVG icon created
⏳ PNG icons - Need to be generated

The app will build without these icons, but they're recommended for a complete PWA experience.
