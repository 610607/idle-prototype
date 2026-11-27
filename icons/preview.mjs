// Writes icons/preview.svg: every pixel icon at 1× and 2× on the UI's dark panel colour.
// Usage: node icons/preview.mjs   (no dependencies)
// Colour icons use icons/palette.txt; monochrome control glyphs (# + *) are drawn in the UI's ink.
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Grid, loadPalette, toPaths } from './tools/px.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const dir = join(here, 'pixel');
const pal = loadPalette(join(here, 'palette.txt'));
const ids = readdirSync(dir)
  .filter((f) => f.endsWith('.txt'))
  .map((f) => f.slice(0, -4))
  .sort();

const INK = '#ede4ca';
const ACCENT = '#e0b43c';
const BG = '#282c24';

function paths(txt) {
  return Object.entries(toPaths(Grid.parse(txt)))
    .map(([ch, d]) => {
      if (ch === '#') return `<path d="${d}" fill="${INK}"/>`;
      if (ch === '+') return `<path d="${d}" fill="${INK}" opacity=".45"/>`;
      if (ch === '*') return `<path d="${d}" fill="${ACCENT}"/>`;
      return `<path d="${d}" fill="${pal.colors[ch]}"/>`;
    })
    .join('');
}

const COLS = 8;
const CW = 150;
const CH = 64;
const rows = Math.ceil(ids.length / COLS);
const W = COLS * CW + 16;
const H = rows * CH + 16;
const cells = ids.map((id, k) => {
  const x = 8 + (k % COLS) * CW;
  const y = 8 + Math.floor(k / COLS) * CH;
  const g = paths(readFileSync(join(dir, id + '.txt'), 'utf8'));
  return (
    `<g transform="translate(${x} ${y})">` +
    `<g transform="translate(6 8)">${g}</g>` +
    `<g transform="translate(30 0) scale(2)">${g}</g>` +
    `<text x="0" y="50" font-family="ui-monospace,Menlo,monospace" font-size="10" fill="#b6af98">${id}</text>` +
    `</g>`
  );
});

const svg =
  `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" shape-rendering="crispEdges">` +
  `<rect width="${W}" height="${H}" fill="${BG}"/>` +
  cells.join('') +
  `</svg>\n`;
writeFileSync(join(here, 'preview.svg'), svg);
console.log(`preview.svg: ${ids.length} icons`);
