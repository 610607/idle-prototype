// Contact sheet for reviewing icons: 1×, 2× and 3× on the UI's three panel colours, with ids.
//   node icons/tools/sheet.mjs OUT.png [id-or-prefix …]
// A prefix ends with '-' or '*' (skill-, tag*). No ids: every icon. Needs Playwright (preinstalled
// in the cloud container); `--html` writes OUT.html instead of a PNG.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { Grid, loadPalette, toPaths } from './px.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pal = loadPalette(join(root, 'palette.txt'));
const args = process.argv.slice(2);
const htmlOnly = args.includes('--html');
const [outPath, ...want] = args.filter((a) => a !== '--html');
if (!outPath) throw new Error('usage: node icons/tools/sheet.mjs OUT.png [id-or-prefix …]');

const all = readdirSync(join(root, 'pixel'))
  .filter((f) => f.endsWith('.txt'))
  .map((f) => f.slice(0, -4))
  .sort();
const match = (id) =>
  !want.length || want.some((w) => (w.endsWith('*') ? id.startsWith(w.slice(0, -1)) : w.endsWith('-') ? id.startsWith(w) : id === w));
const ids = all.filter(match);

const INK = '#ede4ca';
const ACCENT = '#e0b43c';
function svg(id, s) {
  const g = Grid.parse(readFileSync(join(root, 'pixel', id + '.txt'), 'utf8'));
  const p = toPaths(g);
  const body = Object.entries(p)
    .map(([ch, d]) => {
      if (ch === '#') return `<path d="${d}" fill="${INK}"/>`;
      if (ch === '+') return `<path d="${d}" fill="${INK}" opacity=".45"/>`;
      if (ch === '*') return `<path d="${d}" fill="${ACCENT}"/>`;
      return `<path d="${d}" fill="${pal.colors[ch]}"/>`;
    })
    .join('');
  return `<svg width="${16 * s}" height="${16 * s}" viewBox="0 0 16 16" shape-rendering="crispEdges">${body}</svg>`;
}

let html = `<!doctype html><meta charset="utf-8"><body style="margin:0;background:#1d201a;color:#aba58d;font:11px monospace">`;
for (const bg of ['#1d201a', '#282c24', '#4a5143']) {
  html += `<div style="background:${bg};padding:8px;display:flex;flex-wrap:wrap;gap:10px 14px;align-items:flex-end;width:1360px">`;
  for (const id of ids) {
    html += `<div style="display:flex;flex-direction:column;gap:3px;align-items:flex-start"><div style="display:flex;gap:5px;align-items:flex-end">${svg(id, 1)}${svg(id, 2)}${svg(id, 3)}</div><span>${id}</span></div>`;
  }
  html += `</div>`;
}
html += `</body>`;

if (htmlOnly) {
  writeFileSync(outPath.replace(/\.png$/, '') + '.html', html);
} else {
  const require = createRequire(import.meta.url);
  const { chromium } = require('playwright');
  const b = await chromium.launch();
  const pg = await b.newPage({ viewport: { width: 1380, height: 400 }, deviceScaleFactor: 1 });
  await pg.setContent(html);
  await pg.screenshot({ path: outPath, fullPage: true });
  await b.close();
}
console.log(`${ids.length} icons → ${outPath}`);
