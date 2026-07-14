import { test, expect } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const iosDir = path.resolve(__dirname, '..');
const repoRoot = path.resolve(iosDir, '../..');
const config = JSON.parse(fs.readFileSync(path.join(iosDir, 'screens.json'), 'utf8'));
const outRoot = path.join(repoRoot, 'demo-generation/screenshots/appstore');

for (const device of config.devices) {
  for (const screen of config.screens) {
    if (!screen.devices.includes(device.key)) continue;

    test(`${device.key}/${screen.name}`, async ({ page }) => {
      const rawPath = path.join(iosDir, 'raw', device.key, `${screen.name}.png`);
      expect(fs.existsSync(rawPath), `missing raw capture: ${rawPath}`).toBe(true);

      // Render at half size with deviceScaleFactor 2 → exact device pixels.
      await page.setViewportSize({ width: device.width / 2, height: device.height / 2 });

      const url = new URL(`file://${path.join(iosDir, 'compose/template.html')}`);
      url.searchParams.set('headline', screen.headline);
      url.searchParams.set('img', `file://${rawPath}`);
      url.searchParams.set('device', device.key);
      await page.goto(url.toString());
      await page.waitForLoadState('networkidle');

      const outDir = path.join(outRoot, device.key);
      fs.mkdirSync(outDir, { recursive: true });
      const outPath = path.join(outDir, `${screen.name}.png`);
      await page.screenshot({ path: outPath });

      // Validate exact pixel dimensions from the PNG header (IHDR).
      const buf = fs.readFileSync(outPath);
      expect(buf.readUInt32BE(16)).toBe(device.width);
      expect(buf.readUInt32BE(20)).toBe(device.height);
    });
  }
}
