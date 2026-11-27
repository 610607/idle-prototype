// Pixel icons from /icons/pixel/<id>.txt: 16 rows × 16 chars.
//   .  transparent
//   #  ink   (currentColor)
//   +  shade (currentColor at 45%)
//   *  accent (hazard yellow, --px-accent)
// Each grid becomes three SVG paths built from horizontal runs, drawn with crispEdges.
// Content stores icon ids only, so swapping the art never touches the engine.
const raw = import.meta.glob('../../icons/pixel/*.txt', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

interface Px {
  ink: string;
  shade: string;
  accent: string;
}

function parse(txt: string): Px {
  const rows = txt.replace(/\r/g, '').split('\n').slice(0, 16);
  const d: Record<string, string> = { '#': '', '+': '', '*': '' };
  rows.forEach((row, y) => {
    let x = 0;
    while (x < 16) {
      const c = row[x] ?? '.';
      let w = 1;
      while (x + w < 16 && (row[x + w] ?? '.') === c) w++;
      if (c in d) d[c] += `M${x} ${y}h${w}v1h-${w}z`;
      x += w;
    }
  });
  return { ink: d['#']!, shade: d['+']!, accent: d['*']! };
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

export function Icon({ id, size = 16, title, className }: { id: string; size?: number; title?: string; className?: string }) {
  const p = get(id);
  const s = snap(size);
  return (
    <svg
      className={'icon px ' + (className ?? '')}
      // The id lets style.css tint icons by category (res-*, enemy-*, tag-* …) until the art carries colour.
      data-icon={id}
      width={s}
      height={s}
      viewBox="0 0 16 16"
      shapeRendering="crispEdges"
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
    >
      {title ? <title>{title}</title> : null}
      {p ? (
        <>
          <path d={p.shade} fill="currentColor" opacity={0.45} />
          <path d={p.ink} fill="currentColor" />
          <path d={p.accent} fill="var(--px-accent, #e0b43c)" />
        </>
      ) : null}
    </svg>
  );
}

/** Same icon as an SVG group, for drawing inside the board's own <svg>. */
export function BoardIcon({ id, x, y, size, color, accent }: { id: string; x: number; y: number; size: number; color?: string; accent?: string }) {
  const p = get(id);
  if (!p) return null;
  const s = size / 16;
  return (
    <g transform={`translate(${x - size / 2} ${y - size / 2}) scale(${s})`} shapeRendering="crispEdges" pointerEvents="none">
      <path d={p.shade} fill={color ?? 'currentColor'} opacity={0.45} />
      <path d={p.ink} fill={color ?? 'currentColor'} />
      <path d={p.accent} fill={accent ?? '#e0b43c'} />
    </g>
  );
}
