// Dumps the text the web client shows, so moving text into locales/ can be checked to change
// nothing (docs/i18n.md). Run it before and after and compare the two files:
//
//   node scripts/text_dump.mjs <out-file>
//
// It writes deterministic saves with the engine example `web_fixtures`, serves the app with vite,
// freezes the clock, and in a desktop and a phone viewport opens every tab and every sheet (each
// tile, operative, candidate, district, squad, battle report and the tide table). For each
// screen it records the visible text and the title / aria-label / placeholder / alt attributes.
// Opening sheets goes through `window.__idle`, which App sets in dev builds only.
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const { chromium } = createRequire(import.meta.url)('playwright');
const out = process.argv[2];
if (!out) {
  console.error('usage: node scripts/text_dump.mjs <out-file>');
  process.exit(2);
}
const web = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const fx = mkdtempSync(join(tmpdir(), 'web-text-'));
execFileSync('cargo', ['run', '--release', '--quiet', '--example', 'web_fixtures', '--', fx], {
  cwd: join(web, '../engine'),
  stdio: 'inherit',
});

const server = await createServer({ root: web, server: { port: 0, strictPort: false }, logLevel: 'error' });
await server.listen();
const url = server.resolvedUrls.local[0];
const browser = await chromium.launch();
const lines = [];
const VIEWPORTS = [
  ['desktop', { width: 1280, height: 900 }],
  ['phone', { width: 390, height: 844 }],
];

for (const name of ['early', 'mid', 'between']) {
  const save = readFileSync(join(fx, `${name}.json`), 'utf8').trim();
  const at = JSON.parse(save).saved_at * 1000;
  for (const [vp, viewport] of VIEWPORTS) {
    const ctx = await browser.newContext({ viewport, deviceScaleFactor: 1 });
    await ctx.addInitScript((s) => {
      localStorage.setItem('idle-proto/v1/save', s);
      localStorage.setItem('idle-proto/v1/time-offset', '0');
    }, save);
    const page = await ctx.newPage();
    page.on('pageerror', (e) => lines.push(`!! ${name} ${vp} page error: ${e.message}`));
    await page.clock.setFixedTime(at);
    await page.goto(url);
    await page.waitForFunction(() => window.__idle);
    const settle = () => page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    const dump = async (label) => {
      await settle();
      const text = await page.evaluate(() => {
        const names = ['title', 'aria-label', 'placeholder', 'alt'];
        const attrs = [...document.querySelectorAll(names.map((n) => `[${n}]`).join(','))]
          .map((e) => names.map((n) => e.getAttribute(n)).filter(Boolean).join(' | '))
          .join('\n');
        return `${document.title}\n${document.body.innerText}\n-- attributes\n${attrs}`;
      });
      lines.push(`## ${name} ${vp} ${label}\n${text}`);
    };
    await dump('start');
    const tabs = await page.locator('[role=tab]').count();
    for (let i = 0; i < tabs; i++) {
      const tab = page.locator('[role=tab]').nth(i);
      if (!(await tab.isVisible())) continue;
      await tab.click();
      await dump(`tab ${i}`);
    }
    const ids = await page.evaluate(() => {
      const v = window.__idle.view();
      return {
        tile: v.tiles.map((t) => t.i),
        op: v.operatives.map((o) => o.id),
        candidate: v.candidates.map((c) => c.id),
        district: v.districts.map((d) => d.id),
        squad: v.squads.map((s) => s.id),
        report: v.events.filter((e) => e.report != null).map((e) => e.report),
      };
    });
    for (const [kind, list] of Object.entries(ids)) {
      for (const id of list) {
        await page.evaluate(
          ([kind, id]) => {
            const h = window.__idle;
            const sheet = kind === 'report' ? { kind, title: 'report', load: () => h.game.report(id) } : { kind, [kind]: id };
            h.open(sheet, true);
          },
          [kind, id],
        );
        await dump(`${kind} ${id}`);
      }
    }
    await page.evaluate(() => window.__idle.open({ kind: 'tide' }, true));
    await dump('tide');
    await ctx.close();
  }
}

await browser.close();
await server.close();
rmSync(fx, { recursive: true, force: true });
writeFileSync(out, lines.join('\n') + '\n');
console.log(`${lines.length} screens → ${out}`);
