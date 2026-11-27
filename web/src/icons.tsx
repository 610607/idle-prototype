// Pixel icons from /icons/pixel/<id>.txt: 16 rows × 16 chars (icons/README.md).
// Colour icons use the characters of /icons/palette.txt, one colour each; '.' is transparent.
// Monochrome glyphs (controls that follow the text colour) use the older three characters:
//   #  ink   (currentColor)
//   +  shade (currentColor at 45%)
//   *  accent (hazard yellow, --px-accent)
// Each grid becomes one SVG path per character, built from horizontal runs, drawn with crispEdges.
// Content stores icon ids only, so swapping the art never touches the engine.
import paletteTxt from '../../icons/palette.txt?raw';

const raw = import.meta.glob('../../icons/pixel/*.txt', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const palette: Record<string, string> = {};
for (const line of paletteTxt.split('\n')) {
  const s = line.trim();
  if (!s || s.startsWith('#') || s.startsWith('@')) continue;
  const [ch, hex] = s.split(/\s+/);
  if (ch && hex) palette[ch] = hex;
}

/** Path data per character. */
type Px = Record<string, string>;

function parse(txt: string): Px {
  const rows = txt.replace(/\r/g, '').split('\n').slice(0, 16);
  const d: Px = {};
  rows.forEach((row, y) => {
    let x = 0;
    while (x < 16) {
      const c = row[x] ?? '.';
      let w = 1;
      while (x + w < 16 && (row[x + w] ?? '.') === c) w++;
      if (c !== '.') d[c] = (d[c] ?? '') + `M${x} ${y}h${w}v1h-${w}z`;
      x += w;
    }
  });
  return d;
}

const icons: Record<string, Px> = {};
for (const [path, txt] of Object.entries(raw)) {
  const id = path.split('/').pop()!.replace(/\.txt$/, '');
  icons[id] = parse(txt);
}

export function hasIcon(id: string): boolean {
  return id in icons;
}

function get(id: string): Px | undefined {
  return icons[id] ?? icons['unknown'];
}

/** Pixel art only looks right at whole multiples of 16. */
function snap(size: number): number {
  return Math.max(16, Math.round(size / 16) * 16);
}

/** The paths of one icon. `ink`/`accent` colour monochrome glyphs; `solid` paints every pixel one
 *  colour (a silhouette). */
function paths(p: Px, ink: string, accent: string, solid?: string) {
  return Object.entries(p).map(([ch, d]) => {
    if (solid) return <path key={ch} d={d} fill={solid} opacity={ch === '+' ? 0.45 : undefined} />;
    if (ch === '#') return <path key={ch} d={d} fill={ink} />;
    if (ch === '+') return <path key={ch} d={d} fill={ink} opacity={0.45} />;
    if (ch === '*') return <path key={ch} d={d} fill={accent} />;
    return <path key={ch} d={d} fill={palette[ch] ?? ink} />;
  });
}

export function Icon({ id, size = 16, title, className }: { id: string; size?: number; title?: string; className?: string }) {
  const p = get(id);
  const s = snap(size);
  return (
    <svg
      className={'icon px ' + (className ?? '')}
      data-icon={id}
      width={s}
      height={s}
      viewBox="0 0 16 16"
      shapeRendering="crispEdges"
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
    >
      {title ? <title>{title}</title> : null}
      {p ? paths(p, 'currentColor', 'var(--px-accent, #e0b43c)') : null}
    </svg>
  );
}

/** Same icon as an SVG group, for drawing inside the board's own <svg>. `color`/`accent` apply to
 *  monochrome glyphs; `solid` draws a one-colour silhouette. */
export function BoardIcon({ id, x, y, size, color, accent, solid }: { id: string; x: number; y: number; size: number; color?: string; accent?: string; solid?: string }) {
  const p = get(id);
  if (!p) return null;
  const s = size / 16;
  return (
    <g transform={`translate(${x - size / 2} ${y - size / 2}) scale(${s})`} shapeRendering="crispEdges" pointerEvents="none">
      {paths(p, color ?? 'currentColor', accent ?? '#e0b43c', solid)}
    </g>
  );
}
