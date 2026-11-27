// 像素图标小工具：读调色板、读/写网格、几何图元、按左上光源自动上明暗、自动描边。
// 没有依赖，node 直接跑。草稿走 parse→shade→outline（见 build.mjs）。
import { readFileSync } from 'node:fs';

export function loadPalette(path) {
  const colors = {};
  const names = {};
  const materials = {};
  for (const line of readFileSync(path, 'utf8').split('\n')) {
    const s = line.trim();
    if (!s || s.startsWith('#')) continue;
    const parts = s.split(/\s+/);
    if (s.startsWith('@')) {
      const [m, dark, base, light, ...name] = parts;
      materials[m.slice(1)] = { dark, base, light, name: name.join(' ') };
    } else {
      const [ch, hex, ...name] = parts;
      colors[ch] = hex;
      names[ch] = name.join(' ');
    }
  }
  return { colors, names, materials };
}

export class Grid {
  constructor(n) {
    this.n = n;
    this.c = Array.from({ length: n }, () => Array(n).fill('.'));
  }
  static parse(txt) {
    const rows = txt.replace(/\r/g, '').split('\n').filter((r, i, a) => !(i === a.length - 1 && r === ''));
    const n = rows.length;
    const g = new Grid(n);
    rows.forEach((row, y) => {
      if (row.length !== n) throw new Error(`row ${y} has ${row.length} chars, expected ${n}`);
      for (let x = 0; x < n; x++) g.c[y][x] = row[x];
    });
    return g;
  }
  clone() {
    const g = new Grid(this.n);
    g.c = this.c.map((r) => r.slice());
    return g;
  }
  in(x, y) {
    return x >= 0 && y >= 0 && x < this.n && y < this.n;
  }
  get(x, y) {
    return this.in(x, y) ? this.c[y][x] : '.';
  }
  set(x, y, ch) {
    if (this.in(x, y)) this.c[y][x] = ch;
  }
  each(fn) {
    for (let y = 0; y < this.n; y++) for (let x = 0; x < this.n; x++) fn(x, y);
  }
  /** Fill every cell the mask covers. */
  fill(mask, ch) {
    this.each((x, y) => {
      if (mask(x, y)) this.c[y][x] = ch;
    });
    return this;
  }
  /** Fill, after drawing a 1px dark seam (k) around the shape over whatever is already there. */
  stamp(mask, ch, { seam = true } = {}) {
    if (seam) {
      this.each((x, y) => {
        if (mask(x, y)) return;
        if (mask(x - 1, y) || mask(x + 1, y) || mask(x, y - 1) || mask(x, y + 1)) {
          if (this.c[y][x] !== '.') this.c[y][x] = 'k';
        }
      });
    }
    return this.fill(mask, ch);
  }
  /** Explicit pixels from a list of [x,y] or a small sprite string. */
  pix(list, ch) {
    for (const [x, y] of list) this.set(x, y, ch);
    return this;
  }
  sprite(x0, y0, txt) {
    txt
      .replace(/^\n/, '')
      .split('\n')
      .forEach((row, dy) => {
        for (let dx = 0; dx < row.length; dx++) if (row[dx] !== ' ') this.set(x0 + dx, y0 + dy, row[dx] === '_' ? '.' : row[dx]);
      });
    return this;
  }
  /** Material letters → palette letters. Light from the top-left: an edge facing up/left (next to
   *  transparent or k) gets the light tone, an edge facing down/right the dark tone. */
  shade(materials) {
    const out = this.clone();
    const open = (x, y) => {
      const v = this.get(x, y);
      return v === '.' || v === 'k';
    };
    this.each((x, y) => {
      const m = materials[this.c[y][x]];
      if (!m) return;
      const lit = open(x, y - 1) || open(x - 1, y);
      const dark = open(x, y + 1) || open(x + 1, y);
      out.c[y][x] = lit && !dark ? m.light : dark && !lit ? m.dark : m.base;
    });
    return out;
  }
  /** Transparent cells that touch a painted cell (4-neighbourhood) become the outline colour. */
  outline(ch = 'k') {
    const out = this.clone();
    this.each((x, y) => {
      if (this.c[y][x] !== '.') return;
      const nb = [this.get(x - 1, y), this.get(x + 1, y), this.get(x, y - 1), this.get(x, y + 1)];
      if (nb.some((v) => v !== '.')) out.c[y][x] = ch;
    });
    return out;
  }
  text() {
    return this.c.map((r) => r.join('')).join('\n') + '\n';
  }
}

// ---- masks: (x, y) => boolean, sampled at pixel centres ----
const C = (v) => v + 0.5;
export const rect = (x0, y0, x1, y1) => (x, y) => x >= x0 && x <= x1 && y >= y0 && y <= y1;
export const circle = (cx, cy, r) => (x, y) => (C(x) - cx) ** 2 + (C(y) - cy) ** 2 <= r * r;
export const ellipse = (cx, cy, rx, ry) => (x, y) => ((C(x) - cx) / rx) ** 2 + ((C(y) - cy) / ry) ** 2 <= 1;
export const ring = (cx, cy, r0, r1) => (x, y) => {
  const d = Math.hypot(C(x) - cx, C(y) - cy);
  return d >= r0 && d <= r1;
};
export const poly = (pts) => (x, y) => {
  const px = C(x);
  const py = C(y);
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
};
export const seg = (x0, y0, x1, y1, w) => (x, y) => {
  const px = C(x);
  const py = C(y);
  const dx = x1 - x0;
  const dy = y1 - y0;
  const t = Math.max(0, Math.min(1, ((px - x0) * dx + (py - y0) * dy) / (dx * dx + dy * dy || 1)));
  return Math.hypot(px - (x0 + t * dx), py - (y0 + t * dy)) <= w / 2;
};
export const gear = (cx, cy, rRoot, rTip, teeth, duty = 0.5, phase = 0) => (x, y) => {
  const dx = C(x) - cx;
  const dy = C(y) - cy;
  const d = Math.hypot(dx, dy);
  if (d <= rRoot) return true;
  if (d > rTip) return false;
  const a = (Math.atan2(dy, dx) / (2 * Math.PI)) * teeth + phase;
  return a - Math.floor(a) < duty;
};
export const or = (...ms) => (x, y) => ms.some((m) => m(x, y));
export const and = (...ms) => (x, y) => ms.every((m) => m(x, y));
export const not = (m) => (x, y) => !m(x, y);
export const minus = (a, b) => (x, y) => a(x, y) && !b(x, y);

/** Horizontal runs per colour → SVG path data (same trick as web/src/icons.tsx). */
export function toPaths(grid) {
  const d = {};
  for (let y = 0; y < grid.n; y++) {
    let x = 0;
    while (x < grid.n) {
      const ch = grid.c[y][x];
      let w = 1;
      while (x + w < grid.n && grid.c[y][x + w] === ch) w++;
      if (ch !== '.') d[ch] = (d[ch] ?? '') + `M${x} ${y}h${w}v1h-${w}z`;
      x += w;
    }
  }
  return d;
}

export function check(grid, colors, id) {
  const bad = new Set();
  grid.each((x, y) => {
    const v = grid.c[y][x];
    if (v !== '.' && !(v in colors)) bad.add(v);
  });
  if (bad.size) throw new Error(`${id}: unknown palette chars ${[...bad].join('')}`);
}
