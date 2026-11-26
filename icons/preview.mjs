// Writes icons/preview.svg: every pixel icon at 1× and 2× on the UI's dark panel colour.
// Usage: node icons/preview.mjs   (no dependencies)
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const dir = join(here, 'pixel');
const ids = readdirSync(dir)
  .filter((f) => f.endsWith('.txt'))
  .map((f) => f.slice(0, -4))
  .sort();

const INK = '#e8e0c8';
const ACCENT = '#e0b43c';
const BG = '#2c3028';

function paths(txt) {
  const d = { '#': '', '+': '', '*': '' };
  txt
    .replace(/\r/g, '')
    .split('\n')
    .slice(0, 16)
    .forEach((row, y) => {
      let x = 0;
      while (x < 16) {
        const c = row[x] ?? '.';
        let w = 1;
        while (x + w < 16 && (row[x + w] ?? '.') === c) w++;
        if (c in d) d[c] += `M${x} ${y}h${w}v1h-${w}z`;
        x += w;
      }
    });
  return `<path d="${d['+']}" fill="${INK}" opacity=".45"/><path d="${d['#']}" fill="${INK}"/><path d="${d['*']}" fill="${ACCENT}"/>`;
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
