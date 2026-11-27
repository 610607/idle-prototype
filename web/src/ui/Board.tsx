import { useMemo, useState } from 'react';
import { t } from '../i18n';
import { BoardIcon } from '../icons';
import type { TileView, View } from '../types';
import { SQUAD_HEX, useCtx } from './common';

const N = 15;
const C = 32; // cell size: one 16px pixel icon sits at 1:1 in the middle
const M = 14; // margin for coordinates

/** District paint; the values live in style.css (--wash-0 … --wash-9). */
export const WASH = Array.from({ length: 10 }, (_, k) => `var(--wash-${k})`);

/** Board colours, all CSS variables from style.css. */
const COL = {
  sunk: 'var(--map-bg)',
  ink: 'var(--ink)',
  ink2: 'var(--ink-2)',
  ink3: 'var(--ink-3)',
  hazard: 'var(--hazard)',
  bad: 'var(--bad)',
  good: 'var(--good)',
  rift: 'var(--rift-lt)',
  riftDk: 'var(--rift-dk)',
  token: 'var(--map-token)',
  ground: 'var(--map-ground)',
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
          {/* fog: one continuous sheet with a faint diagonal hatch, darker than anything known */}
          <pattern id="fog" width="6" height="6" patternUnits="userSpaceOnUse">
            <rect width="6" height="6" fill="var(--map-fog)" />
            <rect x="0" y="5" width="1" height="1" fill="var(--map-fog-2)" />
            <rect x="1" y="4" width="1" height="1" fill="var(--map-fog-2)" />
            <rect x="2" y="3" width="1" height="1" fill="var(--map-fog-2)" />
            <rect x="3" y="2" width="1" height="1" fill="var(--map-fog-2)" />
            <rect x="4" y="1" width="1" height="1" fill="var(--map-fog-2)" />
            <rect x="5" y="0" width="1" height="1" fill="var(--map-fog-2)" />
          </pattern>
          {/* rubble: chunky broken concrete inside a raised block */}
          <pattern id="rubble" width="9" height="9" patternUnits="userSpaceOnUse">
            <rect width="9" height="9" fill="var(--map-barrier)" />
            <rect x="0" y="0" width="4" height="3" fill="var(--map-chunk)" />
            <rect x="0" y="3" width="4" height="1" fill="var(--map-chunk-dk)" />
            <rect x="5" y="1" width="3" height="3" fill="var(--map-chunk-dk)" />
            <rect x="1" y="5" width="3" height="3" fill="var(--map-chunk)" />
            <rect x="1" y="8" width="3" height="1" fill="var(--map-chunk-dk)" />
            <rect x="6" y="6" width="3" height="2" fill="var(--map-chunk-dk)" />
            <rect x="5" y="5" width="1" height="1" fill="var(--map-barrier-hi)" />
          </pattern>
          {/* concrete grain over owned ground */}
          <pattern id="grain" width="6" height="6" patternUnits="userSpaceOnUse">
            <rect x="1" y="1" width="1" height="1" fill="#000" opacity="0.18" />
            <rect x="4" y="3" width="1" height="1" fill="#fff" opacity="0.07" />
            <rect x="2" y="5" width="1" height="1" fill="#000" opacity="0.12" />
          </pattern>
          {/* rift: violet cracks */}
          <pattern id="crack" width="8" height="8" patternUnits="userSpaceOnUse">
            <rect x="1" y="2" width="2" height="1" fill="var(--map-crack)" opacity="0.7" />
            <rect x="3" y="3" width="1" height="2" fill="var(--map-crack)" opacity="0.7" />
            <rect x="5" y="6" width="2" height="1" fill="var(--map-crack)" opacity="0.5" />
          </pattern>
          <pattern id="alert" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="3" height="8" fill={COL.bad} opacity="0.4" />
          </pattern>
          <pattern id="hq" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="3" height="6" fill={COL.hazard} />
            <rect x="3" width="3" height="6" fill="var(--hazard-ink)" />
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
          const wash = t.district !== undefined ? WASH[t.district % WASH.length]! : 'var(--steel)';
          const icon = tileIcon(t, view);
          const tide = nextTide.get(t.i);
          const soon = t.next_surge !== undefined && t.next_surge - now < 1800;
          let ground: React.ReactNode;
          // Fog fills the whole cell so the unknown reads as one sheet; known tiles keep a 1px gutter.
          if (hidden) ground = <rect x={px} y={py} width={C} height={C} fill="url(#fog)" />;
          else if (t.terrain === 'barrier')
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill="url(#rubble)" />
                <rect x={px} y={py} width={w} height={2} fill="var(--map-barrier-hi)" />
                <rect x={px} y={py} width={2} height={w} fill="var(--map-barrier-hi)" />
                <rect x={px} y={py + w - 3} width={w} height={3} fill="var(--map-barrier-lo)" />
                <rect x={px + w - 2} y={py} width={2} height={w} fill="var(--map-barrier-lo)" />
              </>
            );
          else if (t.terrain === 'rift')
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={t.owned ? 'var(--map-rift-own)' : 'var(--map-rift)'} />
                <rect x={px} y={py} width={w} height={w} fill="url(#crack)" />
                <rect x={px + 0.5} y={py + 0.5} width={w - 1} height={w - 1} fill="none" stroke="var(--map-crack)" strokeOpacity={0.45} />
              </>
            );
          else if (t.terrain === 'hq')
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill="url(#hq)" />
                <rect x={px + 3} y={py + 3} width={w - 6} height={w - 6} fill="var(--map-hq)" />
                <rect x={px + 3} y={py + 3} width={w - 6} height={1} fill="#fff" opacity={0.18} />
              </>
            );
          else if (t.owned)
            // our land: the district's paint at full strength, lit from the top-left like a plate
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={wash} />
                <rect x={px} y={py} width={w} height={w} fill="url(#grain)" />
                <rect x={px} y={py} width={w} height={1} fill="#fff" opacity={0.16} />
                <rect x={px} y={py} width={1} height={w} fill="#fff" opacity={0.08} />
                <rect x={px} y={py + w - 1} width={w} height={1} fill="#000" opacity={0.3} />
              </>
            );
          else
            // known ground, not ours: flat concrete with a hint of the district's paint
            ground = (
              <>
                <rect x={px} y={py} width={w} height={w} fill={COL.ground} />
                <rect x={px} y={py} width={w} height={w} fill={wash} opacity={0.3} />
                <rect x={px} y={py} width={w} height={1} fill="#fff" opacity={0.05} />
              </>
            );
          // icon colour by what the thing is: enemies violet, points of interest hazard, our buildings enamel
          const enemy = !t.owned && t.terrain === 'city' && !!icon;
          const iconColor =
            t.terrain === 'hq'
              ? 'var(--hazard-ink)'
              : t.terrain === 'special'
                ? COL.hazard
                : t.terrain === 'rift'
                  ? COL.rift
                  : enemy
                    ? COL.rift
                    : t.usage === 'housing'
                      ? COL.ink2
                      : COL.ink;
          return (
            <g key={t.i} className="tile" onClick={() => onTile(t)}>
              {ground}
              {t.alert ? <rect x={px} y={py} width={w} height={w} fill="url(#alert)" /> : null}
              {icon ? (
                <g>
                  {/* enemies sit on a dark token so the otherworld reads as pieces on the board */}
                  {enemy ? <rect x={px + 6.5} y={py + 3.5} width={19} height={19} fill={COL.token} stroke={COL.riftDk} strokeWidth={1} /> : null}
                  <BoardIcon id={icon} x={px + C / 2} y={py + C / 2 - 3} size={16} color={iconColor} accent={t.terrain === 'hq' ? 'var(--hazard-ink)' : undefined} />
                </g>
              ) : null}
              {/* wardenship: progress toward claim on unowned tiles, stability on owned ones */}
              {t.revealed && t.terrain !== 'barrier' && t.terrain !== 'hq' && (t.owned || t.warden > 0) ? (
                <g pointerEvents="none">
                  <rect x={px + 4} y={py + C - 7} width={C - 9} height={4} fill={COL.sunk} />
                  <rect
                    x={px + 5}
                    y={py + C - 6}
                    width={Math.round(((C - 11) * Math.max(0, Math.min(100, t.warden))) / 100)}
                    height={2}
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
        <path d={edges.district} stroke={COL.ink3} strokeOpacity={0.55} strokeWidth={1} fill="none" strokeDasharray="2 2" pointerEvents="none" />
        <path d={edges.territory} stroke="var(--hazard-ink)" strokeWidth={4} fill="none" strokeLinecap="square" pointerEvents="none" />
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

/** A little tile for the legend, painted with the board's own patterns. */
function Swatch({ children }: { children: React.ReactNode }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" shapeRendering="crispEdges" aria-hidden>
      {children}
    </svg>
  );
}

function Legend() {
  const row = (swatch: React.ReactNode, text: string) => (
    <span>
      {swatch}
      {text}
    </span>
  );
  // Open on wide screens; phones start with it folded to keep the map and the tide strip in view.
  const [open] = useState(() => !window.matchMedia('(max-width: 900px)').matches);
  return (
    <details className="legend" open={open}>
      <summary>{t('board.legend.title')}</summary>
      <div className="board-legend">
      {row(
        <Swatch>
          <rect width="16" height="16" fill="url(#fog)" />
        </Swatch>,
        t('terrain.unknown'),
      )}
      {row(
        <Swatch>
          <rect width="16" height="16" fill={COL.ground} />
          <rect width="16" height="16" fill={WASH[1]} opacity={0.3} />
        </Swatch>,
        t('terrain.city'),
      )}
      {row(
        <Swatch>
          <rect width="16" height="16" fill={WASH[1]} />
          <rect width="16" height="1" fill="#fff" opacity={0.16} />
          <path d="M1 16V1H16" stroke={COL.hazard} strokeWidth="2" fill="none" />
        </Swatch>,
        t('board.legend.owned'),
      )}
      {row(
        <Swatch>
          <rect width="16" height="16" fill="url(#rubble)" />
          <rect width="16" height="2" fill="var(--map-barrier-hi)" />
          <rect width="2" height="16" fill="var(--map-barrier-hi)" />
          <rect y="13" width="16" height="3" fill="var(--map-barrier-lo)" />
        </Swatch>,
        t('terrain.barrier'),
      )}
      {row(
        <Swatch>
          <rect width="16" height="16" fill="var(--map-rift)" />
          <rect width="16" height="16" fill="url(#crack)" />
          <rect x="0.5" y="0.5" width="15" height="15" fill="none" stroke="var(--map-crack)" strokeOpacity={0.5} />
        </Swatch>,
        t('terrain.rift'),
      )}
      {row(
        <Swatch>
          <rect x="0.5" y="0.5" width="15" height="15" fill={COL.token} stroke={COL.riftDk} />
          <rect x="5" y="4" width="6" height="8" fill={COL.rift} />
          <rect x="6" y="6" width="1" height="2" fill={COL.token} />
          <rect x="9" y="6" width="1" height="2" fill={COL.token} />
        </Swatch>,
        t('board.legend.enemy'),
      )}
      <i className="sep" />
      {row(
        <Swatch>
          <rect x="2" y="2" width="12" height="12" fill="none" stroke={COL.good} strokeWidth="2" strokeDasharray="3 2" />
        </Swatch>,
        t('board.legend.claimable'),
      )}
      {row(
        <Swatch>
          <rect x="3" y="2" width="9" height="12" fill={COL.bad} />
          <rect x="7" y="4" width="2" height="5" fill={COL.ink} />
          <rect x="7" y="10" width="2" height="2" fill={COL.ink} />
        </Swatch>,
        t('board.legend.alert'),
      )}
      {row(
        <Swatch>
          <rect width="16" height="16" fill={COL.ground} />
          <path d="M0 0h9l-9 9z" fill={COL.bad} />
        </Swatch>,
        t('board.legend.lost'),
      )}
      {row(
        <Swatch>
          <rect x="4" y="2" width="1" height="12" fill={COL.ink} />
          <rect x="5" y="2" width="7" height="5" fill={COL.hazard} />
        </Swatch>,
        t('board.legend.garrison'),
      )}
      {row(
        <Swatch>
          <rect x="0" y="6" width="16" height="4" fill={COL.sunk} />
          <rect x="1" y="7" width="9" height="2" fill={COL.good} />
        </Swatch>,
        t('board.legend.warden'),
      )}
      {row(
        <Swatch>
          <rect x="0" y="6" width="16" height="4" fill={SQUAD_HEX[0]} />
        </Swatch>,
        t('board.legend.squad_a'),
      )}
      {row(
        <Swatch>
          <rect x="0" y="6" width="16" height="4" fill={SQUAD_HEX[1]} />
        </Swatch>,
        t('board.legend.squad_b'),
      )}
      </div>
    </details>
  );
}
