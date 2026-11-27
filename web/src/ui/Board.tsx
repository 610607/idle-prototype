import { useMemo } from 'react';
import { t } from '../i18n';
import { BoardIcon } from '../icons';
import type { TileView, View } from '../types';
import { SQUAD_HEX, useCtx } from './common';

const N = 15;
const C = 32; // cell size: one 16px pixel icon sits at 1:1 in the middle
const M = 14; // margin for coordinates

/** District paint: muted, dark enough that enamel-white icons read on top. */
export const WASH = ['#6a6340', '#4b6646', '#47636d', '#6d4c43', '#5b5175', '#6b6745', '#466a5c', '#705a42', '#505a76', '#665a50'];

const COL = {
  sunk: '#161913',
  ground: '#23261f',
  ink: '#e8e0c8',
  ink2: '#b6af98',
  ink3: '#858270',
  hazard: '#e0b43c',
  bad: '#db5e42',
  good: '#98b96f',
  rift: '#a99bd0',
};

function xy(i: number): [number, number] {
  return [i % N, Math.floor(i / N)];
}
function center(i: number): [number, number] {
  const [x, y] = xy(i);
  return [x * C + C / 2, y * C + C / 2];
}

/** Hover text of a tile: its label, then whether it is ours and whether it is on alert. */
function tileTitle(tile: TileView): string {
  return `${tile.label}${tile.owned ? t('board.tile.owned') : ''}${tile.alert ? t('board.tile.alert') : ''}`;
}

function tileIcon(t: TileView, view: View): string | null {
  if (!t.revealed && t.terrain !== 'barrier') return null;
  switch (t.terrain) {
    case 'hq':
      return 'tile-hq';
    case 'rift':
      return t.rift ? `rift-${t.rift.kind}` : 'tile-rift';
    case 'special':
      return t.special ? `sp-${t.special}` : 'unknown';
    case 'barrier':
      return null;
    case 'city':
      if (t.owned) {
        if (t.usage === 'housing') return 'housing';
        if (t.usage === 'none') return null;
        return view.facilities.find((f) => f.id === t.usage)?.icon ?? 'unknown';
      }
      return t.enemies[0]?.[0]?.icon ?? null;
    default:
      return null;
  }
}

/** Four L-shaped corner marks, like a viewfinder. */
function Brackets({ x, y, color, className }: { x: number; y: number; color: string; className?: string }) {
  const a = 7;
  const w = 2;
  const s = C - 1;
  return (
    <g className={className} fill={color} pointerEvents="none">
      <rect x={x - 1} y={y - 1} width={a} height={w} />
      <rect x={x - 1} y={y - 1} width={w} height={a} />
      <rect x={x + s - a + 1} y={y - 1} width={a} height={w} />
      <rect x={x + s - 1} y={y - 1} width={w} height={a} />
      <rect x={x - 1} y={y + s - 1} width={a} height={w} />
      <rect x={x - 1} y={y + s - a + 1} width={w} height={a} />
      <rect x={x + s - a + 1} y={y + s - 1} width={a} height={w} />
      <rect x={x + s - 1} y={y + s - a + 1} width={w} height={a} />
    </g>
  );
}

export function Board() {
  const { view, now, open, pages, routeEdit, setRouteEdit, focusTile, selectedTile, game, toast } = useCtx();
  const tiles = view.tiles;
  const top = pages[pages.length - 1];
  const district = top?.kind === 'district' ? top.district : null;

  // First predicted outcome per border tile from the 12h tide table.
  const nextTide = useMemo(() => {
    const m = new Map<number, { held: boolean; t: number }>();
    for (const e of view.tide) if (!m.has(e.tile) && !e.skipped) m.set(e.tile, { held: e.held, t: e.t });
    return m;
  }, [view.tide]);

  const edges = useMemo(() => {
    const district: string[] = [];
    const territory: string[] = [];
    for (const t of tiles) {
      const [x, y] = xy(t.i);
      const right = x + 1 < N ? tiles[t.i + 1] : undefined;
      const down = y + 1 < N ? tiles[t.i + N] : undefined;
      for (const [o, dir] of [
        [right, 'r'],
        [down, 'd'],
      ] as const) {
        if (!o) continue;
        const seg = dir === 'r' ? `M${(x + 1) * C - 0.5} ${y * C}V${(y + 1) * C}` : `M${x * C} ${(y + 1) * C - 0.5}H${(x + 1) * C}`;
        if (t.owned !== o.owned) territory.push(seg);
        else if ((t.revealed || o.revealed) && t.district !== o.district) district.push(seg);
      }
    }
    return { district: district.join(''), territory: territory.join('') };
  }, [tiles]);

  const onTile = (tile: TileView) => {
    if (routeEdit) {
      const r = routeEdit.route;
      if (r[r.length - 1] === tile.i) {
        setRouteEdit({ ...routeEdit, route: r.slice(0, -1) });
      } else if (r.includes(tile.i)) {
        setRouteEdit({ ...routeEdit, route: r.slice(0, r.indexOf(tile.i) + 1) });
      } else {
        const next = [...r, tile.i];
        const p = game.previewRoute(routeEdit.squad, next);
        // An empty squad may still draw its route; the engine's text for that is err.route_empty.
        if (!p.ok && p.error && p.error !== t('err.route_empty')) {
          toast(p.error, true);
          return;
        }
        setRouteEdit({ ...routeEdit, route: next });
      }
      return;
    }
    open({ kind: 'tile', tile: tile.i }, true);
  };

  const routes = view.squads
    .map((s, si) => ({ s, si, route: routeEdit?.squad === s.id ? routeEdit.route : s.route }))
    .filter((r) => r.route.length > 0);

  const size = N * C;
  return (
    <div className="board-frame">
      <svg className="board" viewBox={`${-M} ${-M} ${size + M + 2} ${size + M + 2}`} role="img" aria-label={t('board.map_label')}>
        <defs>
          {/* fog: an ordered dither, darker than anything known */}
          <pattern id="fog" width="4" height="4" patternUnits="userSpaceOnUse">
            <rect width="4" height="4" fill="#1d201a" />
            <rect width="1" height="1" fill="#2c3027" />
            <rect x="2" y="2" width="1" height="1" fill="#2c3027" />
          </pattern>
          {/* rubble: chunky blocks */}
          <pattern id="rubble" width="8" height="8" patternUnits="userSpaceOnUse">
            <rect width="8" height="8" fill="#34312b" />
            <rect x="0" y="0" width="3" height="2" fill="#4a463d" />
            <rect x="4" y="1" width="3" height="3" fill="#423e36" />
            <rect x="1" y="4" width="2" height="3" fill="#4a463d" />
            <rect x="5" y="5" width="3" height="2" fill="#2a2823" />
            <rect x="3" y="3" width="1" height="1" fill="#57524a" />
          </pattern>
          {/* concrete grain over owned ground */}
          <pattern id="grain" width="6" height="6" patternUnits="userSpaceOnUse">
            <rect x="1" y="1" width="1" height="1" fill="#000" opacity="0.18" />
            <rect x="4" y="3" width="1" height="1" fill="#fff" opacity="0.07" />
            <rect x="2" y="5" width="1" height="1" fill="#000" opacity="0.12" />
          </pattern>
          {/* rift: violet cracks */}
          <pattern id="crack" width="8" height="8" patternUnits="userSpaceOnUse">
            <rect x="1" y="2" width="2" height="1" fill="#7a68a8" opacity="0.6" />
            <rect x="3" y="3" width="1" height="2" fill="#7a68a8" opacity="0.6" />
            <rect x="5" y="6" width="2" height="1" fill="#7a68a8" opacity="0.45" />
          </pattern>
          <pattern id="alert" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="3" height="8" fill={COL.bad} opacity="0.4" />
          </pattern>
          <pattern id="hq" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="3" height="6" fill={COL.hazard} />
            <rect x="3" width="3" height="6" fill={COL.sunk} />
          </pattern>
        </defs>

        {/* coordinates */}
        {Array.from({ length: N }, (_, k) => (
          <g key={'c' + k}>
            <text x={k * C + C / 2} y={-4} textAnchor="middle" className="label">
              {String.fromCharCode(65 + k)}
            </text>
            <text x={-4} y={k * C + C / 2 + 3.5} textAnchor="end" className="label">
              {k + 1}
            </text>
          </g>
        ))}

        <rect x={-1} y={-1} width={size + 1} height={size + 1} fill={COL.sunk} />

        {/* tiles */}
        {tiles.map((t) => {
          const [x, y] = xy(t.i);
          const px = x * C;
          const py = y * C;
          const w = C - 1;
          const hidden = !t.revealed && t.terrain !== 'barrier';
          const wash = t.district !== undefined ? WASH[t.district % WASH.length]! : '#4a4a3e';
          const icon = tileIcon(t, view);
          const tide = nextTide.get(t.i);
          const soon = t.next_surge !== undefined && t.next_surge - now < 1800;
          let ground: React.ReactNode;
          if (hidden) ground = <rect x={px} y={py} width={w} height={w} fill="url(#fog)" />;
          else if (t.terrain === 'barrier') ground = <rect x={px} y={py} width={w} height={w} fill="url(#rubble)" />;
          else if (t.terrain === 'rift')
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={t.owned ? '#4d4268' : '#352d48'} />
                <rect x={px} y={py} width={w} height={w} fill="url(#crack)" />
              </>
            );
          else if (t.terrain === 'hq')
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill="url(#hq)" />
                <rect x={px + 3} y={py + 3} width={w - 6} height={w - 6} fill="#5d5f4c" />
              </>
            );
          else if (t.owned)
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={wash} />
                <rect x={px} y={py} width={w} height={w} fill="url(#grain)" />
                <rect x={px} y={py} width={w} height={1} fill="#fff" opacity={0.08} />
              </>
            );
          else
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={COL.ground} />
                <rect x={px} y={py} width={w} height={w} fill={wash} opacity={0.32} />
              </>
            );
          const iconColor = t.owned || t.terrain === 'hq' ? COL.ink : t.terrain === 'city' || t.terrain === 'rift' ? COL.rift : COL.ink2;
          return (
            <g key={t.i} className="tile" onClick={() => onTile(t)}>
              {ground}
              {t.alert ? <rect x={px} y={py} width={w} height={w} fill="url(#alert)" /> : null}
              {icon ? (
                <g opacity={t.owned || t.terrain !== 'city' ? 1 : 0.8}>
                  <BoardIcon id={icon} x={px + C / 2} y={py + C / 2 - 2} size={16} color={iconColor} />
                </g>
              ) : null}
              {/* wardenship: progress toward claim on unowned tiles, stability on owned ones */}
              {t.revealed && t.terrain !== 'barrier' && t.terrain !== 'hq' && (t.owned || t.warden > 0) ? (
                <g pointerEvents="none">
                  <rect x={px + 4} y={py + C - 6} width={C - 9} height={3} fill={COL.sunk} />
                  <rect
                    x={px + 4}
                    y={py + C - 6}
                    width={Math.round(((C - 9) * Math.max(0, Math.min(100, t.warden))) / 100)}
                    height={3}
                    fill={t.owned ? (t.warden <= 25 ? COL.bad : COL.ink2) : COL.good}
                  />
                </g>
              ) : null}
              {/* next surge: a dog-ear in the top-left corner, red when it is predicted lost */}
              {tide && !t.alert && (!tide.held || soon) ? (
                <path d={`M${px} ${py}h9l-9 9z`} fill={tide.held ? COL.ink3 : COL.bad} pointerEvents="none" />
              ) : null}
              {t.garrison !== undefined ? (
                <g pointerEvents="none">
                  <rect x={px + C - 9} y={py + 3} width={1} height={8} fill={COL.ink} />
                  <rect x={px + C - 8} y={py + 3} width={5} height={4} fill={COL.hazard} />
                </g>
              ) : null}
              {t.claimable ? (
                <rect x={px + 2} y={py + 2} width={w - 4} height={w - 4} fill="none" stroke={COL.good} strokeWidth={2} strokeDasharray="4 3" className="pulse" pointerEvents="none" />
              ) : null}
              {t.alert ? (
                <g className="pulse" pointerEvents="none">
                  <rect x={px + C - 11} y={py + 1} width={9} height={11} fill={COL.bad} />
                  <rect x={px + C - 7} y={py + 3} width={2} height={5} fill={COL.ink} />
                  <rect x={px + C - 7} y={py + 9} width={2} height={2} fill={COL.ink} />
                </g>
              ) : null}
              <rect className="hover" x={px + 0.5} y={py + 0.5} width={w - 1} height={w - 1} fill="none" stroke={COL.ink} strokeWidth={1} pointerEvents="none" />
              <title>{tileTitle(t)}</title>
            </g>
          );
        })}

        {/* district and territory lines */}
        <path d={edges.district} stroke={COL.ink3} strokeOpacity={0.7} strokeWidth={1} fill="none" strokeDasharray="2 2" pointerEvents="none" />
        <path d={edges.territory} stroke={COL.hazard} strokeWidth={2} fill="none" strokeLinecap="square" pointerEvents="none" />

        {/* transit-map routes */}
        {routes.map(({ s, si, route }) => {
          const off = (si === 0 ? -1 : 1) * (routes.length > 1 ? 3 : 0);
          const pts = route.map((i) => center(i)).map(([x, y]) => [x + off, y + off] as const);
          const color = SQUAD_HEX[si % SQUAD_HEX.length];
          const editing = routeEdit?.squad === s.id;
          const d = pts.map(([x, y], k) => `${k ? 'L' : 'M'}${x} ${y}`).join('');
          return (
            <g key={'r' + s.id} pointerEvents="none">
              {pts.length > 1 ? <path d={d} stroke={COL.sunk} strokeWidth={7} fill="none" strokeLinecap="square" strokeLinejoin="miter" /> : null}
              {pts.length > 1 ? (
                <path d={d} stroke={color} strokeWidth={3} fill="none" strokeLinecap="square" strokeLinejoin="miter" strokeDasharray={editing ? '6 3' : undefined} />
              ) : null}
              {pts.map(([x, y], k) => {
                const last = k === pts.length - 1;
                const r = last ? 5 : 4;
                return (
                  <g key={k}>
                    <rect x={x - r} y={y - r} width={r * 2} height={r * 2} fill={last ? color : COL.sunk} stroke={color} strokeWidth={2} />
                    {editing ? (
                      <text x={x} y={y + 4} textAnchor="middle" fontSize="11" fontFamily="VT323, monospace" fill={last ? COL.sunk : color}>
                        {k + 1}
                      </text>
                    ) : null}
                  </g>
                );
              })}
            </g>
          );
        })}

        {/* squads on the move: out along the walked path and back */}
        {view.squads.map((s, si) => {
          const trip = s.running;
          if (!trip || trip.walked.length === 0) return null;
          const span = Math.max(1, trip.end - trip.start);
          const p = Math.max(0, Math.min(1, (now - trip.start) / span));
          const path = [...trip.walked, ...trip.walked.slice(0, -1).reverse()];
          const f = p * (path.length - 1);
          const k = Math.floor(f);
          const a = center(path[k]!);
          const b = center(path[Math.min(path.length - 1, k + 1)]!);
          const x = Math.round(a[0] + (b[0] - a[0]) * (f - k));
          const y = Math.round(a[1] + (b[1] - a[1]) * (f - k));
          const color = SQUAD_HEX[si % SQUAD_HEX.length];
          return (
            <g key={'m' + s.id} pointerEvents="none">
              <rect x={x - 10} y={y - 10} width={20} height={20} fill={color} stroke={COL.sunk} strokeWidth={2} />
              <BoardIcon id="squad" x={x} y={y} size={16} color={COL.sunk} accent={COL.sunk} />
            </g>
          );
        })}

        {district !== null
          ? tiles
              .filter((t) => t.district === district)
              .map((t) => (
                <rect key={'d' + t.i} x={xy(t.i)[0] * C + 1} y={xy(t.i)[1] * C + 1} width={C - 3} height={C - 3} fill={COL.hazard} fillOpacity={0.12} stroke={COL.hazard} strokeWidth={2} pointerEvents="none" />
              ))
          : null}
        {selectedTile !== null && !routeEdit ? <Brackets x={xy(selectedTile)[0] * C} y={xy(selectedTile)[1] * C} color={COL.hazard} /> : null}
        {focusTile !== null && focusTile !== selectedTile ? <Brackets x={xy(focusTile)[0] * C} y={xy(focusTile)[1] * C} color={COL.ink} className="pulse" /> : null}
      </svg>
      <Legend />
    </div>
  );
}

function Legend() {
  return (
    <div className="board-legend">
      <span>
        <svg width="18" height="6">
          <rect y="1" width="18" height="4" fill={COL.hazard} />
        </svg>
        {t('board.legend.border')}
      </span>
      <span>
        <svg width="16" height="6">
          <rect y="1" width="16" height="3" fill={SQUAD_HEX[0]} />
        </svg>
        {t('board.legend.squad_a')}
      </span>
      <span>
        <svg width="16" height="6">
          <rect y="1" width="16" height="3" fill={SQUAD_HEX[1]} />
        </svg>
        {t('board.legend.squad_b')}
      </span>
      <span>
        <svg width="12" height="12">
          <rect x="1" y="1" width="10" height="10" fill="none" stroke={COL.good} strokeWidth="2" strokeDasharray="3 2" />
        </svg>
        {t('board.legend.claimable')}
      </span>
      <span>
        <svg width="10" height="12">
          <rect width="9" height="11" fill={COL.bad} />
          <rect x="4" y="2" width="2" height="5" fill={COL.ink} />
          <rect x="4" y="8" width="2" height="2" fill={COL.ink} />
        </svg>
        {t('board.legend.alert')}
      </span>
      <span>
        <svg width="10" height="10">
          <path d="M0 0h9l-9 9z" fill={COL.bad} />
        </svg>
        {t('board.legend.lost')}
      </span>
      <span>
        <svg width="8" height="10">
          <rect x="0" y="0" width="1" height="9" fill={COL.ink} />
          <rect x="1" y="0" width="5" height="4" fill={COL.hazard} />
        </svg>
        {t('board.legend.garrison')}
      </span>
      <span>
        <svg width="20" height="6">
          <rect y="1" width="20" height="3" fill={COL.sunk} />
          <rect y="1" width="12" height="3" fill={COL.good} />
        </svg>
        {t('board.legend.warden')}
      </span>
    </div>
  );
}
