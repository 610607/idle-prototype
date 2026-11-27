// Build icons: icons/src/<id>.txt (material drafts) → icons/pixel/<id>.txt (palette characters only),
// then check every icons/pixel file.
//   node icons/tools/build.mjs            build every draft, check everything
//   node icons/tools/build.mjs id id …    build only these drafts (others are only checked)
// No dependencies.
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Grid, loadPalette } from './px.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const pal = loadPalette(join(root, 'palette.txt'));
const src = join(root, 'src');
const out = join(root, 'pixel');
// Monochrome glyphs (controls that follow the text colour): ink, shade, accent.
const MONO = new Set(['#', '+', '*']);

const only = process.argv.slice(2);
const drafts = readdirSync(src)
  .filter((f) => f.endsWith('.txt'))
  .map((f) => f.slice(0, -4))
  .filter((id) => !only.length || only.includes(id));
for (const id of only) if (!existsSync(join(src, id + '.txt'))) throw new Error(`${id}: no draft icons/src/${id}.txt`);

for (const id of drafts) {
  const g = Grid.parse(readFileSync(join(src, id + '.txt'), 'utf8'));
  if (g.n !== 16) throw new Error(`${id}: draft has ${g.n} rows, expected 16`);
  writeFileSync(join(out, id + '.txt'), g.shade(pal.materials).outline().text());
}

let errors = 0;
for (const f of readdirSync(out).filter((f) => f.endsWith('.txt'))) {
  const id = f.slice(0, -4);
  let g;
  try {
    g = Grid.parse(readFileSync(join(out, f), 'utf8'));
  } catch (e) {
    console.error(`${id}: ${e.message}`);
    errors++;
    continue;
  }
  if (g.n !== 16) {
    console.error(`${id}: ${g.n} rows, expected 16`);
    errors++;
    continue;
  }
  const chars = new Set();
  g.each((x, y) => chars.add(g.c[y][x]));
  chars.delete('.');
  const mono = [...chars].filter((c) => MONO.has(c));
  const colour = [...chars].filter((c) => !MONO.has(c));
  const unknown = colour.filter((c) => !(c in pal.colors));
  if (unknown.length) {
    console.error(`${id}: characters not in palette.txt: ${unknown.join('')}`);
    errors++;
  }
  if (mono.length && colour.length) {
    console.error(`${id}: mixes monochrome (#+*) and palette characters`);
    errors++;
  }
}
if (errors) process.exit(1);
console.log(`built ${drafts.length}, checked ${readdirSync(out).length} icons`);
