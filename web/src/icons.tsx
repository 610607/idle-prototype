// Shared line icons from /icons (24 grid, 1.75 stroke, currentColor). Content stores icon ids only.
const raw = import.meta.glob('../../icons/*.svg', { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

const inner: Record<string, string> = {};
for (const [path, svg] of Object.entries(raw)) {
  const id = path.split('/').pop()!.replace(/\.svg$/, '');
  const m = svg.match(/<svg[^>]*>([\s\S]*)<\/svg>/);
  inner[id] = m ? m[1]!.trim() : '';
}

export function hasIcon(id: string): boolean {
  return id in inner;
}

export function Icon({ id, size = 18, title, className }: { id: string; size?: number; title?: string; className?: string }) {
  const body = inner[id] ?? inner['unknown'] ?? '';
  return (
    <svg
      className={'icon ' + (className ?? '')}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden={title ? undefined : true}
      role={title ? 'img' : undefined}
      dangerouslySetInnerHTML={{ __html: (title ? `<title>${title}</title>` : '') + body }}
    />
  );
}

/** Same icon as an SVG group, for drawing inside the board's own <svg>. */
export function BoardIcon({ id, x, y, size, color }: { id: string; x: number; y: number; size: number; color?: string }) {
  const body = inner[id] ?? '';
  const s = size / 24;
  return (
    <g
      transform={`translate(${x - size / 2} ${y - size / 2}) scale(${s})`}
      fill="none"
      stroke={color ?? 'currentColor'}
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      dangerouslySetInnerHTML={{ __html: body }}
    />
  );
}
