import { createContext, useContext, type ReactNode } from 'react';
import type { Game } from '../engine/client';
import { t } from '../i18n';
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
  /** Switch the workspace to a tab (closing any open pages). */
  goTab: (t: 'command' | 'roster' | 'districts' | 'research' | 'hq') => void;
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

export const RES_KEYS: (keyof Resources)[] = ['scrap', 'parts', 'intel', 'crystal', 'medicine'];
export const RES_NAMES: Record<keyof Resources, string> = {
  scrap: t('common.res.scrap'),
  parts: t('common.res.parts'),
  intel: t('common.res.intel'),
  crystal: t('common.res.crystal'),
  medicine: t('common.res.medicine'),
};

export function fmtIn(secs: number): string {
  if (secs <= 0) return t('common.time.now');
  if (secs < 60) return t('common.time.s', { s: secs });
  const m = Math.floor(secs / 60);
  if (m < 60) return t('common.time.m', { m });
  const h = Math.floor(m / 60);
  const mm = m % 60;
  if (h < 48) return mm ? t('common.time.hm', { h, mm }) : t('common.time.h', { h });
  return t('common.time.dh', { d: Math.floor(h / 24), h: h % 24 });
}

export function fmtAgo(secs: number): string {
  if (secs < 60) return t('common.time.just_now');
  return t('common.time.ago', { t: fmtIn(secs) });
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
  victory: t('outcome.victory'),
  planned: t('outcome.planned'),
  emergency: t('outcome.emergency'),
  wipe: t('outcome.wipe'),
};

export const DIAL_NAMES: Record<string, string> = { mine: t('dial.mine'), balance: t('dial.balance'), seal: t('dial.seal') };

/** Squad colours; the values live in style.css (--squad-0, --squad-1). */
export const SQUAD_HEX = ['var(--squad-0)', 'var(--squad-1)'];

export function squadColor(view: View, squadId: number): string {
  const i = view.squads.findIndex((s) => s.id === squadId);
  return SQUAD_HEX[i % SQUAD_HEX.length] ?? 'var(--ink-2)';
}

/** Tag colours; the values live in style.css (--tag-*). */
export const TAG_HEX: Record<Tag, string> = {
  shock: 'var(--tag-shock)',
  mark: 'var(--tag-mark)',
  overload: 'var(--tag-overload)',
  erosion: 'var(--tag-erosion)',
  shield: 'var(--tag-shield)',
  fortify: 'var(--tag-fortify)',
};

/** Short name of a workspace page, for breadcrumbs. */
export function pageLabel(view: View, s: Sheet): string {
  switch (s.kind) {
    case 'tile':
      return view.tiles[s.tile]?.label ?? t('common.crumb.tile');
    case 'op':
      return view.operatives.find((o) => o.id === s.op)?.name ?? t('common.crumb.op');
    case 'candidate':
      return view.candidates?.find((c) => c.id === s.candidate)?.op.name ?? t('common.crumb.candidate');
    case 'district':
      return view.districts.find((d) => d.id === s.district)?.name ?? t('common.crumb.district');
    case 'report':
      return t('common.crumb.report');
    case 'squad':
      return view.squads.find((q) => q.id === s.squad)?.name ?? t('common.crumb.squad');
    case 'tide':
      return t('common.crumb.tide');
  }
}

// ---------------------------------------------------------------------------
// Small components

export function TagChip({ tag, n, mode }: { tag: Tag; n?: number; mode?: 'read' | 'write' | 'solid' }) {
  const { game } = useCtx();
  const def = game.content.tags.find((d) => d.id === tag);
  const cls = 'chip' + (mode === 'read' ? ' read' : '') + (mode === 'solid' ? ' solid' : '');
  const style = mode === 'solid' ? { background: TAG_HEX[tag] } : { color: TAG_HEX[tag] };
  const title =
    (def ? t('common.tag.title', { name: def.name, desc: def.desc }) : tag) +
    (mode === 'read' ? t('common.tag.read') : mode === 'write' ? t('common.tag.write') : '');
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
      {writes.map((w) => (
        <TagChip key={'w' + w} tag={w} mode="write" />
      ))}
      {reads.length ? <span className="faint tiny">{t('common.tag.reads')}</span> : null}
      {reads.map((r) => (
        <TagChip key={'r' + r} tag={r} mode="read" />
      ))}
    </span>
  );
}

export function Cost({ cost, have }: { cost: Resources; have?: Resources }) {
  const parts = RES_KEYS.filter((k) => (cost[k] ?? 0) > 0);
  if (!parts.length) return <span className="faint">{t('common.cost.free')}</span>;
  return (
    <span className="row" style={{ gap: 8 }}>
      {parts.map((k) => (
        <span key={k} className={'res-item ' + (have && (have[k] ?? 0) < (cost[k] ?? 0) ? 'bad' : '')}>
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
  if (!deltas.length) return <div className="delta-empty">{t('common.delta.empty')}</div>;
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
      {dry.irreversible ? <div className="warn small">{t('common.dry.irreversible')}</div> : null}
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
        <button className="btn ghost small" onClick={onClose} aria-label={t('common.page.back')} title={t('common.page.back_title')}>
          <Icon id="chevron-right" className="flip" /> {t('common.page.back')}
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
            {t('common.confirm.cancel')}
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

/** Enamel colour of a section's badge: what kind of thing the plate holds. */
export type Tone = 'hazard' | 'bad' | 'good' | 'rift' | 'elec' | 'rust' | 'steel';

/** A steel plate with a stencilled header. `tone` paints the header badge; `flush` drops the plate
 *  (for sections that sit inside a card of their own). */
export function Section({
  title,
  icon,
  hint,
  tone,
  flush,
  id,
  children,
}: {
  title: ReactNode;
  icon?: string;
  hint?: ReactNode;
  tone?: Tone;
  flush?: boolean;
  id?: string;
  children: ReactNode;
}) {
  return (
    <section id={id} className={'section tone-' + (tone ?? 'steel') + (flush ? ' flush' : '')}>
      <div className="section-title">
        {icon ? (
          <span className="section-badge">
            <Icon id={icon} size={16} />
          </span>
        ) : null}
        <h3>{title}</h3>
        {hint ? <span className="hint">{hint}</span> : null}
      </div>
      <div className="section-body">{children}</div>
    </section>
  );
}
