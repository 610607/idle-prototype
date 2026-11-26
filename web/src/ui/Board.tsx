import { useMemo } from 'react';
import { BoardIcon } from '../icons';
import type { TileView, View } from '../types';
import { SQUAD_HEX, useCtx } from './common';

const N = 15;
const C = 40; // cell size
const M = 16; // margin for coordinates

const WASH = ['#eadbb9', '#d7e3c6', '#cddde4', '#e8d0cc', '#dbd2e7', '#e4e0bf', '#cde2d6', '#ead6c3', '#d4d9ea', '#e6dccf'];

function xy(i: number): [number, number] {
  return [i % N, Math.floor(i / N)];
}
function center(i: number): [number, number] {
  const [x, y] = xy(i);
  return [x * C + C / 2, y * C + C / 2];
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

export function Board() {
  const { view, now, open, routeEdit, setRouteEdit, focusTile, game, toast } = useCtx();
  const tiles = view.tiles;

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
        const seg =
          dir === 'r'
            ? `M${(x + 1) * C} ${y * C}V${(y + 1) * C}`
            : `M${x * C} ${(y + 1) * C}H${(x + 1) * C}`;
        if (t.owned !== o.owned) territory.push(seg);
        else if ((t.revealed || o.revealed) && t.district !== o.district) district.push(seg);
      }
    }
    // Outer edge of territory touching the map border.
    return { district: district.join(''), territory: territory.join('') };
  }, [tiles]);

  const onTile = (t: TileView) => {
    if (routeEdit) {
      const r = routeEdit.route;
      if (r[r.length - 1] === t.i) {
        setRouteEdit({ ...routeEdit, route: r.slice(0, -1) });
      } else if (r.includes(t.i)) {
        setRouteEdit({ ...routeEdit, route: r.slice(0, r.indexOf(t.i) + 1) });
      } else {
        const next = [...r, t.i];
        const p = game.previewRoute(routeEdit.squad, next);
        if (!p.ok && p.error && !p.error.includes('小队没人')) {
          toast(p.error, true);
          return;
        }
        setRouteEdit({ ...routeEdit, route: next });
      }
      return;
    }
    open({ kind: 'tile', tile: t.i });
  };

  const routes = view.squads
    .map((s, si) => ({ s, si, route: routeEdit?.squad === s.id ? routeEdit.route : s.route }))
    .filter((r) => r.route.length > 0);

  return (
    <div className="board-wrap">
      <svg className="board" viewBox={`${-M} ${-M} ${N * C + M + 4} ${N * C + M + 4}`} role="img" aria-label="城市地图">
        <defs>
          <pattern id="fog" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <rect width="6" height="6" fill="#e2dccd" />
            <line x1="0" y1="0" x2="0" y2="6" stroke="#cdc5b2" strokeWidth="2" />
          </pattern>
          <pattern id="barrier" width="5" height="5" patternUnits="userSpaceOnUse">
            <rect width="5" height="5" fill="#bfb6a0" />
            <path d="M0 0L5 5M5 0L0 5" stroke="#4a4538" strokeWidth="0.8" />
          </pattern>
          <pattern id="alert" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
            <line x1="0" y1="0" x2="0" y2="8" stroke="#b4372a" strokeWidth="2" opacity="0.35" />
          </pattern>
        </defs>

        {/* coordinates */}
        {Array.from({ length: N }, (_, k) => (
          <g key={'c' + k}>
            <text x={k * C + C / 2} y={-5} textAnchor="middle" className="label">
              {String.fromCharCode(65 + k)}
            </text>
            <text x={-5} y={k * C + C / 2 + 2.5} textAnchor="end" className="label">
              {k + 1}
            </text>
          </g>
        ))}

        {/* tiles */}
        {tiles.map((t) => {
          const [x, y] = xy(t.i);
          const px = x * C;
          const py = y * C;
          const hidden = !t.revealed && t.terrain !== 'barrier';
          const wash = t.district !== undefined ? WASH[t.district % WASH.length] : '#efe8d8';
          let fill = wash;
          let opacity = 1;
          if (hidden) fill = 'url(#fog)';
          else if (t.terrain === 'barrier') fill = 'url(#barrier)';
          else if (t.terrain === 'rift') fill = t.owned ? '#d9cfe6' : '#cbbfdc';
          else if (t.terrain === 'hq') fill = '#efe3c4';
          else if (!t.owned) opacity = 0.5;
          const icon = tileIcon(t, view);
          const tide = nextTide.get(t.i);
          const soon = t.next_surge !== undefined && t.next_surge - now < 1800;
          return (
            <g key={t.i} className="tile" onClick={() => onTile(t)}>
              <rect x={px} y={py} width={C} height={C} fill="#f3eee2" />
              <rect x={px} y={py} width={C} height={C} fill={fill} opacity={opacity} />
              {t.alert ? <rect x={px} y={py} width={C} height={C} fill="url(#alert)" /> : null}
              {!t.owned && t.revealed && t.terrain === 'city' ? (
                <rect x={px + 0.5} y={py + 0.5} width={C - 1} height={C - 1} fill="none" stroke="#26241f" strokeOpacity={0.12} strokeDasharray="1 2" />
              ) : null}
              {icon ? <BoardIcon id={icon} x={px + C / 2} y={py + C / 2 - 2} size={t.owned || t.terrain !== 'city' ? 20 : 15} color={t.owned || t.terrain !== 'city' ? '#26241f' : '#8c8574'} /> : null}
              {/* wardenship: progress toward claim on unowned tiles, stability on owned ones */}
              {t.revealed && t.terrain !== 'barrier' && t.terrain !== 'hq' && (t.owned || t.warden > 0) ? (
                <g>
                  <rect x={px + 6} y={py + C - 7} width={C - 12} height={3.5} rx={1} fill="#f3eee2" stroke="#26241f" strokeWidth={0.6} />
                  <rect
                    x={px + 6}
                    y={py + C - 7}
                    width={((C - 12) * Math.max(0, Math.min(100, t.warden))) / 100}
                    height={3.5}
                    rx={1}
                    fill={t.owned ? (t.warden <= 25 ? '#b4372a' : '#26241f') : '#2e7d4f'}
                  />
                </g>
              ) : null}
              {t.claimable ? (
                <rect x={px + 2.5} y={py + 2.5} width={C - 5} height={C - 5} fill="none" stroke="#2e7d4f" strokeWidth={2} strokeDasharray="4 2.5" className="pulse" />
              ) : null}
              {t.alert ? (
                <g className="pulse">
                  <circle cx={px + C - 7} cy={py + 7} r={5.5} fill="#b4372a" />
                  <text x={px + C - 7} y={py + 10} textAnchor="middle" fontSize="9" fontWeight="700" fill="#fff">
                    !
                  </text>
                </g>
              ) : null}
              {tide && !t.alert ? (
                <g>
                  <BoardIcon id="wave" x={px + 8} y={py + 8} size={10} color={tide.held ? (soon ? '#26241f' : '#8c8574') : '#b4372a'} />
                  {!tide.held ? <circle cx={px + 8} cy={py + 8} r={6.5} fill="none" stroke="#b4372a" strokeWidth={1} /> : null}
                </g>
              ) : null}
              {t.garrison !== undefined ? <BoardIcon id="garrison" x={px + C - 8} y={py + C - 13} size={10} color="#26241f" /> : null}
              {focusTile === t.i ? <rect x={px + 1} y={py + 1} width={C - 2} height={C - 2} fill="none" stroke="#1f5f8b" strokeWidth={2.5} className="pulse" /> : null}
              <rect className="hover" x={px + 1} y={py + 1} width={C - 2} height={C - 2} fill="none" stroke="#26241f" strokeWidth={1.5} />
              <title>{`${t.label}${t.owned ? ' · 领地' : ''}${t.alert ? ' · 告急' : ''}`}</title>
            </g>
          );
        })}

        {/* district and territory lines */}
        <path d={edges.district} stroke="#26241f" strokeOpacity={0.45} strokeWidth={1} fill="none" strokeDasharray="3 2" pointerEvents="none" />
        <path d={edges.territory} stroke="#26241f" strokeWidth={2.5} fill="none" strokeLinecap="square" pointerEvents="none" />
        <rect x={0} y={0} width={N * C} height={N * C} fill="none" stroke="#26241f" strokeWidth={1.5} pointerEvents="none" />

        {/* transit-map routes */}
        {routes.map(({ s, si, route }) => {
          const off = (si === 0 ? -1 : 1) * (routes.length > 1 ? 3 : 0);
          const pts = route.map((i) => center(i)).map(([x, y]) => [x + off, y + off] as const);
          const color = SQUAD_HEX[si % SQUAD_HEX.length];
          const editing = routeEdit?.squad === s.id;
          const d = pts.map(([x, y], k) => `${k ? 'L' : 'M'}${x} ${y}`).join('');
          return (
            <g key={'r' + s.id} pointerEvents="none">
              {pts.length > 1 ? <path d={d} stroke="#f3eee2" strokeWidth={8} fill="none" strokeLinecap="round" strokeLinejoin="round" /> : null}
              {pts.length > 1 ? (
                <path d={d} stroke={color} strokeWidth={5} fill="none" strokeLinecap="round" strokeLinejoin="round" strokeDasharray={editing ? '7 4' : undefined} />
              ) : null}
              {pts.map(([x, y], k) => (
                <g key={k}>
                  <circle cx={x} cy={y} r={k === pts.length - 1 ? 5.5 : 4} fill="#fff" stroke={color} strokeWidth={2.5} />
                  {editing ? (
                    <text x={x} y={y + 2.6} textAnchor="middle" fontSize="7" fontWeight="700" fill={color}>
                      {k + 1}
                    </text>
                  ) : null}
                </g>
              ))}
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
          const x = a[0] + (b[0] - a[0]) * (f - k);
          const y = a[1] + (b[1] - a[1]) * (f - k);
          const color = SQUAD_HEX[si % SQUAD_HEX.length];
          return (
            <g key={'m' + s.id} pointerEvents="none">
              <circle cx={x} cy={y} r={7} fill={color} stroke="#fff" strokeWidth={2} />
              <BoardIcon id="squad" x={x} y={y} size={9} color="#fff" />
            </g>
          );
        })}
      </svg>
      <div className="board-legend">
        <span>
          <svg width="16" height="8">
            <path d="M1 4H15" stroke={SQUAD_HEX[0]} strokeWidth="4" strokeLinecap="round" />
          </svg>
          甲队路线
        </span>
        <span>
          <svg width="16" height="8">
            <path d="M1 4H15" stroke={SQUAD_HEX[1]} strokeWidth="4" strokeLinecap="round" />
          </svg>
          乙队路线
        </span>
        <span>
          <svg width="14" height="14">
            <rect x="1" y="1" width="12" height="12" fill="none" stroke="#2e7d4f" strokeWidth="2" strokeDasharray="3 2" />
          </svg>
          可占领
        </span>
        <span>
          <svg width="14" height="14">
            <circle cx="7" cy="7" r="5.5" fill="#b4372a" />
          </svg>
          告急
        </span>
        <span>
          <svg width="14" height="14">
            <circle cx="7" cy="7" r="5.5" fill="none" stroke="#b4372a" />
          </svg>
          预计失守
        </span>
        <span>
          <svg width="20" height="8">
            <rect x="1" y="2" width="18" height="4" fill="#f3eee2" stroke="#26241f" strokeWidth="0.8" />
            <rect x="1" y="2" width="11" height="4" fill="#2e7d4f" />
          </svg>
          镇守度
        </span>
      </div>
    </div>
  );
}
