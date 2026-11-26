import { createContext, useContext, type ReactNode } from 'react';
import type { Game } from '../engine/client';
import { Icon } from '../icons';
import type { Command, Delta, DryRun, Resources, Tag, View } from '../types';

// ---------------------------------------------------------------------------
// App context: the engine, the latest view, and the few UI actions every panel needs.

export type Sheet =
  | { kind: 'tile'; tile: number }
  | { kind: 'op'; op: number }
  | { kind: 'candidate'; candidate: number }
  | { kind: 'district'; district: number }
  | { kind: 'report'; title: string; load: () => import('../types').BattleReport | null }
  | { kind: 'squad'; squad: number }
  | { kind: 'tide' };

export interface Ctx {
  game: Game;
  view: View;
  now: number;
  /** Push a page onto the workspace; `reset` starts a fresh trail; null goes back to the tab. */
  open: (s: Sheet | null, reset?: boolean) => void;
  /** The workspace trail, oldest first. */
  pages: Sheet[];
  /** Pop back to trail length n (0 = the tab itself). */
  popTo: (n: number) => void;
  /** Tile shown in the workspace, outlined on the board. */
  selectedTile: number | null;
  /** Ask before something hard to undo. */
  confirm: (c: ConfirmAsk) => void;
  /** Run a command; shows the error as a toast. Returns success. */
  run: (cmd: Command, okText?: string) => boolean;
  toast: (text: string, error?: boolean) => void;
  routeEdit: RouteEdit | null;
  setRouteEdit: (r: RouteEdit | null) => void;
  focusTile: number | null;
  setFocusTile: (t: number | null) => void;
  /** Name of the workspace tab the trail started from. */
  tabName: string;
}

export interface ConfirmAsk {
  title: string;
  body: ReactNode;
  ok: string;
  danger?: boolean;
  onOk: () => void;
}

export interface RouteEdit {
  squad: number;
  route: number[];
}

export const GameCtx = createContext<Ctx | null>(null);
export function useCtx(): Ctx {
  const c = useContext(GameCtx);
  if (!c) throw new Error('no game context');
  return c;
}

// ---------------------------------------------------------------------------
// Formatting

export const RES_KEYS: (keyof Resources)[] = ['scrap', 'parts', 'intel', 'crystal'];
export const RES_NAMES: Record<keyof Resources, string> = { scrap: '废材', parts: '零件', intel: '情报', crystal: '晶体' };

export function fmtIn(secs: number): string {
  if (secs <= 0) return '现在';
  if (secs < 60) return `${secs} 秒`;
  const m = Math.floor(secs / 60);
  if (m < 60) return `${m} 分`;
  const h = Math.floor(m / 60);
  const mm = m % 60;
  if (h < 48) return mm ? `${h} 时 ${mm} 分` : `${h} 时`;
  return `${Math.floor(h / 24)} 天 ${h % 24} 时`;
}

export function fmtAgo(secs: number): string {
  if (secs < 60) return '刚刚';
  return fmtIn(secs) + '前';
}

export function fmtClock(t: number): string {
  const d = new Date(t * 1000);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

export function fmtRate(v: number): string {
  // income arrives x100 per hour
  const x = v / 100;
  return (x >= 0 ? '+' : '') + (Number.isInteger(x) ? x.toFixed(0) : x.toFixed(1));
}

export const OUTCOME_NAMES: Record<string, string> = {
  victory: '全胜',
  planned: '按计划收队',
  emergency: '紧急撤退',
  wipe: '全员倒下',
};

export const DIAL_NAMES: Record<string, string> = { mine: '开采', balance: '平衡', seal: '封印' };

export const SQUAD_HEX = ['#5ea3d6', '#e58cab'];

export function squadColor(view: View, squadId: number): string {
  const i = view.squads.findIndex((s) => s.id === squadId);
  return SQUAD_HEX[i % SQUAD_HEX.length] ?? '#b6af98';
}

export const TAG_HEX: Record<Tag, string> = {
  shock: '#4f8fd6',
  mark: '#d0503c',
  overload: '#e08a2c',
  erosion: '#8aa040',
  shield: '#7a9cc0',
  fortify: '#a07a50',
};

/** Short name of a workspace page, for breadcrumbs. */
export function pageLabel(view: View, s: Sheet): string {
  switch (s.kind) {
    case 'tile':
      return view.tiles[s.tile]?.label ?? '格子';
    case 'op':
      return view.operatives.find((o) => o.id === s.op)?.name ?? '队员';
    case 'candidate':
      return view.candidates.find((c) => c.id === s.candidate)?.op.name ?? '候选';
    case 'district':
      return view.districts.find((d) => d.id === s.district)?.name ?? '街区';
    case 'report':
      return '战报';
    case 'squad':
      return view.squads.find((q) => q.id === s.squad)?.name ?? '小队';
    case 'tide':
      return '潮汐表';
  }
}

// ---------------------------------------------------------------------------
// Small components

export function TagChip({ tag, n, mode }: { tag: Tag; n?: number; mode?: 'read' | 'write' | 'solid' }) {
  const { game } = useCtx();
  const def = game.content.tags.find((t) => t.id === tag);
  const cls = 'chip' + (mode === 'read' ? ' read' : '') + (mode === 'solid' ? ' solid' : '');
  const style = mode === 'solid' ? { background: TAG_HEX[tag] } : { color: TAG_HEX[tag] };
  const title = (def ? `${def.name}：${def.desc}` : tag) + (mode === 'read' ? '（读取）' : mode === 'write' ? '（施加）' : '');
  return (
    <span className={cls} style={style} title={title}>
      <Icon id={`tag-${tag}`} size={16} />
      {def?.name ?? tag}
      {n !== undefined && n > 1 ? <span className="num">×{n}</span> : null}
    </span>
  );
}

/** "Reads" and "writes" tag rows: how a trait, skill or enemy plugs into the tag web. */
export function TagIO({ reads, writes }: { reads: Tag[]; writes: Tag[] }) {
  if (!reads.length && !writes.length) return null;
  return (
    <span className="row wrap" style={{ gap: 4 }}>
      {writes.map((t) => (
        <TagChip key={'w' + t} tag={t} mode="write" />
      ))}
      {reads.length ? <span className="faint tiny">读</span> : null}
      {reads.map((t) => (
        <TagChip key={'r' + t} tag={t} mode="read" />
      ))}
    </span>
  );
}

export function Cost({ cost, have }: { cost: Resources; have?: Resources }) {
  const parts = RES_KEYS.filter((k) => cost[k] > 0);
  if (!parts.length) return <span className="faint">免费</span>;
  return (
    <span className="row" style={{ gap: 8 }}>
      {parts.map((k) => (
        <span key={k} className={'res-item ' + (have && have[k] < cost[k] ? 'bad' : '')}>
          <Icon id={`res-${k}`} size={16} />
          <span className="num">{cost[k]}</span>
        </span>
      ))}
    </span>
  );
}

export function Bar({ value, max, tone }: { value: number; max: number; tone?: 'good' | 'bad' | 'warn' }) {
  const pct = Math.max(0, Math.min(100, (value / Math.max(1, max)) * 100));
  return (
    <div className={'bar ' + (tone ?? '')}>
      <i style={{ width: pct + '%' }} />
    </div>
  );
}

export function Rarity({ r, name }: { r: number; name: string }) {
  return <span className={`rarity r${r}`}>{name}</span>;
}

export function DeltaList({ deltas }: { deltas: Delta[] }) {
  if (!deltas.length) return <div className="delta-empty">没有可见的变化。</div>;
  return (
    <div className="deltas">
      {deltas.map((d, i) => (
        <DeltaRow key={i} d={d} />
      ))}
    </div>
  );
}

function DeltaRow({ d }: { d: Delta }) {
  return (
    <>
      <span>{d.label}</span>
      <span className="num muted">{d.before}</span>
      <span className="arrow">→</span>
      <span className={'num after ' + (d.better ? 'good' : 'bad')}>{d.after}</span>
    </>
  );
}

/** A dry-run block: what the command would change, decided by the engine. */
export function DryRunView({ dry }: { dry: DryRun | null }) {
  if (!dry) return null;
  if (!dry.ok) return <div className="bad small">{dry.error}</div>;
  return (
    <div className="stack" style={{ gap: 6 }}>
      <DeltaList deltas={dry.deltas} />
      {dry.irreversible ? <div className="warn small">这一步改变地图形状，做了就不能撤回。</div> : null}
    </div>
  );
}

/** A page in the workspace. `aside` becomes a second column once the workspace is wide. */
export function Page({
  title,
  icon,
  onClose,
  children,
  aside,
  extra,
  wideRight,
}: {
  title: ReactNode;
  icon?: string;
  onClose: () => void;
  children: ReactNode;
  aside?: ReactNode;
  extra?: ReactNode;
  wideRight?: boolean;
}) {
  const { pages, popTo, view, tabName } = useCtx();
  return (
    <div className="page">
      <div className="page-head">
        <button className="btn ghost small" onClick={onClose} aria-label="返回" title="返回（Esc）">
          <Icon id="chevron-right" className="flip" /> 返回
        </button>
        {icon ? <Icon id={icon} size={32} /> : null}
        <div className="grow" style={{ minWidth: 0 }}>
          <div className="crumbs ellipsis">
            <button onClick={() => popTo(0)}>{tabName}</button>
            {pages.slice(0, -1).map((p, k) => (
              <span key={k}>
                {' › '}
                <button onClick={() => popTo(k + 1)}>{pageLabel(view, p)}</button>
              </span>
            ))}
          </div>
          <h2 className="ellipsis">{title}</h2>
        </div>
        {extra}
      </div>
      <div className="pane">
        {aside ? (
          <div className={'split' + (wideRight ? ' wide-right' : '')}>
            <div className="col">{children}</div>
            <div className="col">{aside}</div>
          </div>
        ) : (
          children
        )}
      </div>
    </div>
  );
}

export function ConfirmDialog({ ask, onDone }: { ask: ConfirmAsk; onDone: () => void }) {
  return (
    <div className="scrim" onClick={onDone}>
      <div className="dialog stack" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2 style={{ fontSize: 20 }}>{ask.title}</h2>
        <div className="small muted">{ask.body}</div>
        <div className="row" style={{ justifyContent: 'flex-end', marginTop: 6 }}>
          <button className="btn ghost" onClick={onDone}>
            算了
          </button>
          <button
            className={'btn ' + (ask.danger ? 'danger' : 'primary')}
            onClick={() => {
              ask.onOk();
              onDone();
            }}
          >
            {ask.ok}
          </button>
        </div>
      </div>
    </div>
  );
}

export function Section({ title, icon, hint, children }: { title: ReactNode; icon?: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <section>
      <div className="section-title">
        {icon ? <Icon id={icon} size={16} /> : null}
        <h3>{title}</h3>
        {hint ? <span className="hint">{hint}</span> : null}
      </div>
      {children}
    </section>
  );
}
