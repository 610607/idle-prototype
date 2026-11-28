import { useEffect, useRef, useState } from 'react';
import { t } from '../i18n';
import { Icon } from '../icons';
import type { ArmoryView, HintView, OpView, PendingView, SquadView, TideView } from '../types';
import { WASH } from './Board';
import { Bar, Cost, fmtAgo, fmtClock, fmtIn, OUTCOME_NAMES, RES_KEYS, RES_NAMES, Rarity, Section, shows, SQUAD_HEX, TagIO, useCtx } from './common';

const PENDING_ICON: Record<string, string> = {
  map: 'map',
  alert: 'alert',
  claim: 'check',
  candidate: 'candidate',
  policy: 'policy',
  route: 'route',
  defend: 'garrison',
  research: 'trait-book',
  alert_more: 'alert',
  defend_more: 'garrison',
};

// ---------------------------------------------------------------------------
// 指挥 tab: what to do (tasks, squads) on the left; what is coming and what happened on the right.

export function CommandPanel() {
  const { view } = useCtx();
  return (
    <div className="split">
      <div className="col">
        <Apprentice />
        <SquadsPanel />
      </div>
      <div className="col">
        {view.threat ? <TideTable limit={10} /> : null}
        <EventLog />
      </div>
    </div>
  );
}

/** The forecast window (balance.tide_horizon) as one strip: every surge is a mark, lost ones stand out. */
export function TideStrip() {
  const { game, view, now, open, setFocusTile } = useCtx();
  const H = game.balance.tide_horizon;
  const last = Math.round(H / 3600);
  const W = 480;
  const lost = view.tide.filter((e) => !e.held && !e.skipped).length;
  const x = (t: number) => 4 + ((t - now) / H) * (W - 8);
  const hours = Array.from({ length: last + 1 }, (_, k) => k);
  // Stack marks that land on the same few minutes.
  const lanes: number[] = [];
  const marks = view.tide
    .filter((e) => e.t - now <= H)
    .map((e) => {
      const px = x(e.t);
      let lane = 0;
      while (lanes[lane] !== undefined && px - lanes[lane]! < 7) lane++;
      lanes[lane] = px;
      return { e, px, lane: Math.min(lane, 2) };
    });
  return (
    <div className="tide-strip">
      <div className="head">
        <span className="section-badge">
          <Icon id="tide" />
        </span>
        <h4>{t('panels.tide_strip.title', { h: last })}</h4>
        <span className="grow">
          {t('panels.tide_strip.count', { n: view.tide.length })}
          {lost ? <b className="bad">{t('panels.tide_strip.lost', { n: lost })}</b> : <span className="good">{t('panels.tide_strip.all_held')}</span>}
        </span>
        <button className="btn ghost small" onClick={() => open({ kind: 'tide' }, true)}>
          {t('panels.tide_strip.full_table')}
        </button>
      </div>
      <svg viewBox={`0 0 ${W} 40`} preserveAspectRatio="none" role="img" aria-label={t('panels.tide_strip.aria', { h: last })}>
        <rect x={0} y={4} width={W} height={22} fill="var(--sunk)" />
        {hours.map((h) => (
          <g key={h}>
            <rect x={x(now + h * 3600) - 0.5} y={h % 4 === 0 ? 4 : 20} width={1} height={h % 4 === 0 ? 22 : 6} fill="var(--line)" />
            {h % 3 === 0 ? (
              <text x={x(now + h * 3600)} y={38} textAnchor={h === 0 ? 'start' : h === last ? 'end' : 'middle'}>
                {h === 0 ? t('panels.tide_strip.now') : fmtClock(now + h * 3600)}
              </text>
            ) : null}
          </g>
        ))}
        {marks.map(({ e, px, lane }, k) => (
          <g
            key={k}
            className="tick"
            onClick={() => {
              setFocusTile(e.tile);
              open({ kind: 'tile', tile: e.tile }, true);
            }}
          >
            <rect
              x={px - 2.5}
              y={6 + lane * 6.5}
              width={5}
              height={5.5}
              fill={e.skipped ? 'var(--ink-3)' : e.held ? 'var(--ink-2)' : 'var(--bad)'}
              opacity={e.skipped ? 0.5 : 1}
            />
            <title>
              {t('panels.tide_strip.mark', {
                time: fmtClock(e.t),
                tile: e.label,
                result: e.skipped ? t('panels.tide.skipped') : e.held ? t('panels.tide.held') : t('panels.tide_strip.lost_wave', { wave: String(e.lost_wave) }),
              })}
            </title>
          </g>
        ))}
      </svg>
    </div>
  );
}

/** 见习任务: the next few "do it once" steps; hidden once every task is claimed. */
function Apprentice() {
  const { view, run } = useCtx();
  const tasks = view.tasks;
  if (!tasks || !tasks.open.length) return null;
  return (
    <Section title={t('panels.tasks.title')} icon="check" tone="hazard" hint={`${tasks.claimed}/${tasks.total}`}>
      <div className="stack" style={{ gap: 6 }}>
        {tasks.open.map((x) => {
          const reward = RES_KEYS.filter((k) => (x.reward[k] ?? 0) > 0).map((k) => `${RES_NAMES[k]} +${x.reward[k]}`);
          return (
            <div key={x.id} className={'task-item' + (x.done ? ' done' : '')}>
              <div className="row">
                <span className="chip">{x.group}</span>
                <b>{x.name}</b>
                <span className="grow" />
                {reward.length ? <span className="tiny muted">{reward.join(' · ')}</span> : null}
                {x.done ? (
                  <button className="btn small primary" onClick={() => run({ cmd: 'claim_task', task: x.id }, t('panels.tasks.claimed', { name: x.name }))}>
                    {t('panels.tasks.claim')}
                  </button>
                ) : null}
              </div>
              {!x.done ? <div className="small muted">{x.desc}</div> : null}
            </div>
          );
        })}
      </div>
    </Section>
  );
}

export function PendingList() {
  const { view, open, setFocusTile, setRouteEdit, goTab } = useCtx();
  const items = view.pending;
  const act = (p: PendingView) => {
    if (p.tile !== undefined) setFocusTile(p.tile);
    if (p.kind === 'candidate' && p.candidate !== undefined) open({ kind: 'candidate', candidate: p.candidate }, true);
    else if (p.kind === 'policy' && p.district !== undefined) open({ kind: 'district', district: p.district }, true);
    else if (p.kind === 'research') goTab('research');
    else if (p.kind === 'alert_more' || p.kind === 'defend_more') open({ kind: 'tide' }, true);
    else if (p.kind === 'route') {
      const s = view.squads.find((s) => p.text.startsWith(s.name));
      if (s) setRouteEdit({ squad: s.id, route: s.route.filter((i) => !view.tiles[i]!.owned) });
    } else if (p.tile !== undefined) open({ kind: 'tile', tile: p.tile }, true);
  };
  return (
    <Section title={t('panels.pending.title')} icon="event" tone={view.pending.some((p) => p.kind === 'alert' || p.kind === 'defend') ? 'bad' : 'hazard'} hint={items.length ? t('panels.pending.count', { n: items.length }) : t('panels.pending.none')}>
      {items.length === 0 ? (
        <div className="empty">{t('panels.pending.empty')}</div>
      ) : (
        <div className="stack" style={{ gap: 6 }}>
          {items.map((p, k) => (
            <button key={k} className={'pending-item ' + p.kind} onClick={() => act(p)}>
              <Icon id={PENDING_ICON[p.kind] ?? 'event'} />
              <span className="grow small">{p.text}</span>
              <Icon id="chevron-right" className="faint" />
            </button>
          ))}
        </div>
      )}
    </Section>
  );
}

/** Departments: founded, ready to found (a button), or how far along their condition is. */
export function Departments() {
  const { view, run } = useCtx();
  if (!view.departments?.length) return null;
  return (
    <Section title={t('panels.dept.title')} icon="org" tone="hazard">
      <div className="next-list">
        {view.departments.map((d) => (
          <div key={d.id} className={'next-item' + (d.can_found ? ' ready' : '')}>
            <div className="row wrap small" style={{ gap: 6 }}>
              <b>{d.name}</b>
              {d.founded ? (
                <span className="chip good">{t('panels.dept.founded')}</span>
              ) : (
                <>
                  <span className="grow muted">{d.ready ? t('panels.dept.ready') : d.cond}</span>
                  {d.ready ? null : (
                    <span className="num">
                      {d.have}/{d.need}
                    </span>
                  )}
                </>
              )}
            </div>
            {d.founded || d.ready ? null : <Bar value={d.have} max={d.need} />}
            <div className="tiny faint">{d.desc}</div>
            {d.founded ? null : (
              <div className="row wrap small" style={{ gap: 8, marginTop: 4 }}>
                <span className="muted">{t('panels.dept.cost')}</span>
                <Cost cost={d.cost} have={view.resources} />
                {d.ready ? (
                  <button
                    className={'btn small ' + (d.can_found ? 'primary pulse-ring' : '')}
                    disabled={!d.can_found}
                    onClick={() => run({ cmd: 'found_department', department: d.id }, t('panels.dept.done', { name: d.name }))}
                  >
                    {t('panels.dept.found')}
                  </button>
                ) : null}
              </div>
            )}
          </div>
        ))}
      </div>
    </Section>
  );
}

/** How long a new hint stays marked as new, in seconds of game time. */
const HINT_FRESH_SECS = 12;

/** A fixed map's hint: the latest one stays until another replaces it, and a new one flashes. It
 *  never blocks anything. Once the chapter is over this is the way out instead. `compact`: the copy
 *  phones show above the workspace. */
export function HintLine({ compact }: { compact?: boolean }) {
  const { view, now } = useCtx();
  const [last, setLast] = useState<HintView | null>(view.hint ?? null);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (view.hint) setLast(view.hint);
  }, [view.hint?.t, view.hint?.text]); // eslint-disable-line react-hooks/exhaustive-deps
  if (view.chapter_end) return <ChapterEndCard compact={compact} />;
  const h = view.hint ?? last;
  if (!h) return null;
  const fresh = now - h.t < HINT_FRESH_SECS;
  return (
    // Keyed by the hint, so a new one remounts and its flash plays again.
    // On phones an older hint folds to two lines above the workspace; a tap opens it.
    <div
      key={`${h.t}:${h.text}`}
      className={'hint-line' + (fresh ? ' fresh' : '') + (compact ? ' compact' : '') + (compact && !fresh && !open ? ' folded' : '')}
      role="status"
      onClick={compact ? () => setOpen((o) => !o) : undefined}
    >
      <span className="hint-tag">{fresh ? t('panels.hint.new') : t('panels.hint.tag')}</span>
      <span className="hint-text">{h.text}</span>
    </div>
  );
}

/** The chapter is over: the ending, the 密室 and the way on to a formal run. */
function ChapterEndCard({ compact }: { compact?: boolean }) {
  const { view, run } = useCtx();
  const end = view.chapter_end!;
  const leave = (
    <button className="btn primary" onClick={() => run({ cmd: 'leave_map' }, t('panels.chapter_end.left'))}>
      {t('panels.chapter_end.leave')}
    </button>
  );
  if (compact)
    return (
      <div className="hint-line ending compact">
        <span className="hint-tag">{t('panels.chapter_end.title')}</span>
        <span className="hint-text">{leave}</span>
      </div>
    );
  return (
    <div className="ending-card">
      <h3>{t('panels.chapter_end.title')}</h3>
      <p>{end.text}</p>
      <RecordList />
      <div className="row wrap" style={{ gap: 8 }}>
        {leave}
        <span className="tiny faint">{t('panels.chapter_end.stay')}</span>
      </div>
    </div>
  );
}

/** The ending once, as a dialog over the page; closed, it stays above the map (ChapterEndCard). */
export function ChapterEndDialog() {
  const { view, run } = useCtx();
  const [closed, setClosed] = useState<number | null>(null);
  const end = view.chapter_end;
  if (!end || closed === end.t || view.phase !== 'active') return null;
  return (
    <div className="scrim" onClick={() => setClosed(end.t)}>
      <div className="dialog stack ending-dialog" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <h2 style={{ fontSize: 22 }}>{t('panels.chapter_end.title')}</h2>
        <p style={{ margin: 0 }}>{end.text}</p>
        <RecordList />
        <div className="row wrap" style={{ justifyContent: 'flex-end', gap: 8 }}>
          <button className="btn ghost" onClick={() => setClosed(end.t)}>
            {t('panels.chapter_end.later')}
          </button>
          <button className="btn primary" onClick={() => run({ cmd: 'leave_map' }, t('panels.chapter_end.left'))}>
            {t('panels.chapter_end.leave')}
          </button>
        </div>
      </div>
    </div>
  );
}

/** The 密室 lines: what was done, and what the room below holds because of it. */
function RecordList() {
  const { view } = useCtx();
  if (!view.records?.length) return null;
  return (
    <div className="records">
      <div className="tiny muted">{t('panels.records.lead')}</div>
      {view.records.map((r) => (
        <div key={r.id} className={'record' + (r.done ? ' done' : '')}>
          <span className="mark">{r.done ? '✓' : '·'}</span>
          <span>
            {r.done ? null : <span className="faint">{t('panels.records.not_yet')}</span>}
            <b>{r.did}</b> → {r.got}
          </span>
        </div>
      ))}
    </div>
  );
}

/** 密室: the room under HQ, as repaired so far, and the next one or two repairs. */
export function RecordsPanel() {
  const { view } = useCtx();
  if (!view.records?.length) return null;
  return (
    <Section title={t('panels.records.title')} icon="org" tone="hazard">
      <RecordList />
    </Section>
  );
}

/** The next one or two unlocks and how far along they are; absent in games with everything open. */
export function NextUnlocks() {
  const { view } = useCtx();
  if (!view.next?.length) return null;
  return (
    <Section title={t('panels.next.title')} icon="unlock" tone="hazard">
      <div className="next-list">
        {view.next.map((n) => (
          <div key={n.id} className="next-item" title={n.hint}>
            <div className="row small" style={{ gap: 6 }}>
              <b>{n.name}</b>
              <span className="grow muted ellipsis">{n.cond}</span>
              <span className="num">
                {n.have}/{n.need}
              </span>
            </div>
            <Bar value={n.have} max={n.need} />
            <div className="tiny faint">{n.hint}</div>
          </div>
        ))}
      </div>
    </Section>
  );
}

export function TideRows({ rows }: { rows: TideView[] }) {
  const { now, open, setFocusTile } = useCtx();
  return (
    <table className="tide">
      <thead>
        <tr>
          <th>{t('panels.tide.col_time')}</th>
          <th>{t('panels.tide.col_tile')}</th>
          <th>{t('panels.tide.col_result')}</th>
          <th>{t('panels.tide.col_warden')}</th>
        </tr>
      </thead>
      <tbody>
        {rows.map((e, k) => (
          <tr
            key={k}
            className={e.skipped ? 'skipped' : e.held ? '' : 'lost'}
            onClick={() => {
              setFocusTile(e.tile);
              open({ kind: 'tile', tile: e.tile });
            }}
          >
            <td className="num">
              {fmtClock(e.t)} <span className="faint tiny">{fmtIn(e.t - now)}</span>
            </td>
            <td>{e.label}</td>
            <td>{e.skipped ? t('panels.tide.skipped') : e.held ? t('panels.tide.held') : t('panels.tide.lost_wave', { wave: String(e.lost_wave), total: e.total_waves })}</td>
            <td className="num">
              {e.warden_before}→{e.warden_after}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function TideTable({ limit }: { limit?: number }) {
  const { game, view, open } = useCtx();
  const rows = limit ? view.tide.slice(0, limit) : view.tide;
  const lostOnly = view.tide.filter((e) => !e.held && !e.skipped);
  const lost = view.tide.filter((e) => !e.held && !e.skipped).length;
  return (
    <Section
      title={t('panels.tide.title')}
      icon="tide"
      tone="rust"
      hint={
        <span>
          {t('panels.tide.hint', { n: view.tide.length, h: game.balance.tide_horizon / 3600 })}
          {lost ? <b className="bad">{t('panels.tide.hint_lost', { n: lost })}</b> : null}
        </span>
      }
    >
      {lostOnly.length ? (
        <div className="card striped bad" style={{ marginBottom: 10 }}>
          <div className="small">
            <b className="bad">{t('panels.tide.lost_head')}</b>
            {t('panels.tide.lost_body', { tiles: [...new Set(lostOnly.map((e) => e.label))].join(t('panels.sep.list')) })}
          </div>
        </div>
      ) : null}
      {rows.length === 0 ? <div className="empty">{t('panels.tide.empty')}</div> : <TideRows rows={rows} />}
      {limit && view.tide.length > limit ? (
        <button className="btn small ghost" onClick={() => open({ kind: 'tide' })}>
          {t('panels.tide.all', { n: view.tide.length })}
        </button>
      ) : null}
    </Section>
  );
}

export function SquadsPanel() {
  const { view } = useCtx();
  return (
    <Section
      title={t('panels.squads.title')}
      icon="squad"
      tone="elec"
      hint={view.threat ? t('panels.squads.threat', { n: view.threat.value, level: (view.threat.level_bp / 10000).toFixed(1) }) : undefined}
    >
      <div className="stack">
        {view.squads.map((s, i) => (
          <SquadCard key={s.id} s={s} color={SQUAD_HEX[i % SQUAD_HEX.length]!} />
        ))}
      </div>
      {view.med_line ? <MedLineRow /> : null}
      {view.threat ? (
        <div className="small muted" style={{ marginTop: 6 }}>
          {t('panels.squads.threat_note', { x: (view.threat.strength_bp / 10000).toFixed(1), next: view.threat.next })}
        </div>
      ) : null}
    </Section>
  );
}

function SquadCard({ s, color }: { s: SquadView; color: string }) {
  const { game, view, now, open, run, setRouteEdit } = useCtx();
  const stop = s.last?.stopped ?? null;
  const members = s.members.map((id) => view.operatives.find((o) => o.id === id)).filter(Boolean) as OpView[];
  const trip = s.running;
  const routes = shows(view, 'routes');
  const status = !members.length
    ? t('panels.squad.no_members')
    : s.force
      ? t('panels.squad.force_next')
      : trip
      ? trip.end - now <= 0
        ? t('panels.squad.arriving')
        : t(s.once ? 'panels.squad.on_dispatch' : 'panels.squad.on_route', { left: fmtIn(trip.end - now) })
      : members.every((m) => m.health.down)
        ? t('panels.squad.all_down')
        : s.route.length
          ? t('panels.squad.ready')
          : routes
            ? t('panels.squad.no_route')
            : t('panels.squad.idle_dispatch');
  return (
    <div className="card" style={{ boxShadow: `inset 4px 0 0 ${color}`, paddingLeft: 14 }}>
      <div className="row wrap">
        <h3 style={{ color, fontSize: 17 }}>{s.name}</h3>
        <span className="small muted grow">{status}</span>
        {routes ? (
          <button className="btn small" onClick={() => setRouteEdit({ squad: s.id, route: s.route })} disabled={!members.length}>
            <Icon id="route" size={14} /> {t('panels.squad.route')}
          </button>
        ) : null}
        <button className="btn small ghost" onClick={() => open({ kind: 'squad', squad: s.id })}>
          {t('panels.squad.details')}
        </button>
      </div>
      <div className="stack" style={{ gap: 4, marginTop: 4 }}>
        {members.map((m) => (
          <div key={m.id} className="member-row">
            <button className="row link" style={{ gap: 3 }} onClick={() => open({ kind: 'op', op: m.id })}>
              <Icon id={m.icon} />
              <span className="small">{m.name}</span>
            </button>
            <HpBar o={m} />
            <HealButton o={m} />
          </div>
        ))}
      </div>
      {s.route.length && !s.once ? (
        <div className="small faint" style={{ marginTop: 2 }}>
          {t('panels.squad.route_list', { tiles: s.route.map((i) => view.tiles[i]!.label).join(' → ') })}
        </div>
      ) : null}
      {trip ? <Bar value={now - trip.start} max={trip.end - trip.start} /> : null}
      {s.last ? (
        <div className={'small ' + (s.last.outcome === 'victory' ? 'good' : 'bad')} style={{ marginTop: 2 }}>
          {s.last.results.length
            ? t('panels.squad.last_results', { outcome: OUTCOME_NAMES[s.last.outcome], results: s.last.results.map((r) => r.label).join(t('panels.sep.list')) })
            : t('panels.squad.last_patrol', { outcome: OUTCOME_NAMES[s.last.outcome] })}
          {s.last.forced ? t('panels.squad.forced') : null}
          {s.last.healed_at?.length ? t('panels.squad.healed_at', { tiles: s.last.healed_at.map((i) => view.tiles[i]!.label).join(t('panels.sep.list')) }) : null}
        </div>
      ) : null}
      {stop ? (
        <div className="row wrap small warn stop-line" style={{ gap: 6, marginTop: 2 }}>
          <span className="grow">{t(stop.low_hp ? 'panels.squad.stopped_hp' : 'panels.squad.stopped', { tile: stop.label, outcome: stop.outcome ? OUTCOME_NAMES[stop.outcome] : '' })}</span>
          {stop.report !== null ? (
            <button className="btn small ghost" onClick={() => open({ kind: 'report', title: `${s.name} · ${stop.label}`, load: () => game.report(stop.report!) })}>
              {t('panels.squad.why')}
            </button>
          ) : null}
          {!s.force && routes && s.route.length ? (
            <button className="btn small" onClick={() => run({ cmd: 'assault', squad: s.id }, t('panels.squad.assault_done', { squad: s.name, tile: stop.label }))}>
              {t('panels.squad.assault')}
            </button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

const EVENT_ICON: Record<string, string> = {
  claimable: 'check',
  claimed: 'garrison',
  alert: 'alert',
  surge_lost: 'wave',
  surge_held: 'wave',
  candidate: 'candidate',
  trait: 'trait',
  trip: 'route',
  contiguous: 'trust',
  core: 'sp-core',
  special: 'star',
  rule: 'rules',
  drop: 'equip',
  info: 'event',
};

export function EventLog() {
  const { view, now, open, game } = useCtx();
  const [all, setAll] = useState(false);
  const events = all ? view.events : view.events.slice(0, 12);
  return (
    <Section title={t('panels.events.title')} icon="event">
      <div className="list">
        {events.map((e) => (
          <div key={e.id} className="list-item small" style={{ alignItems: 'flex-start' }}>
            <Icon id={EVENT_ICON[e.kind] ?? 'event'} className={e.kind === 'alert' || e.kind === 'surge_lost' ? 'bad' : e.kind === 'claimed' || e.kind === 'claimable' ? 'good' : 'faint'} />
            <span className="grow">{e.text}</span>
            {e.report !== undefined ? (
              <button className="btn small ghost" onClick={() => open({ kind: 'report', title: e.text, load: () => game.report(e.report!) })}>
                {t('panels.events.report')}
              </button>
            ) : null}
            <span className="faint tiny" style={{ whiteSpace: 'nowrap' }}>
              {fmtAgo(now - e.t)}
            </span>
          </div>
        ))}
      </div>
      {!all && view.events.length > 12 ? (
        <button className="btn small ghost" onClick={() => setAll(true)}>
          {t('panels.events.more')}
        </button>
      ) : null}
    </Section>
  );
}

// ---------------------------------------------------------------------------
// 队员 tab

/** Current health as a bar: downed, out on a trip (as it will be when back), or at home. */
export function HpBar({ o }: { o: OpView }) {
  const { view, now } = useCtx();
  const h = o.health;
  const pct = h.down ? 0 : Math.round((h.hp * 100) / Math.max(1, h.max));
  const tone = h.down || pct < 30 ? 'bad' : pct < 60 ? 'warn' : 'good';
  // `up_in` is from when the view was built; count it down between redraws.
  const upIn = h.up_in !== undefined ? Math.max(0, h.up_in - (now - view.time)) : undefined;
  const label = h.down
    ? upIn !== undefined
      ? t('panels.hp.down_in', { left: fmtIn(upIn) })
      : t('panels.hp.down_away')
    : h.away
      ? t('panels.hp.back', { hp: h.hp, max: h.max })
      : t('panels.hp.now', { hp: h.hp, max: h.max });
  return (
    <div className={'hp ' + tone + (h.down ? ' down' : '')}>
      <span className="hp-bar">
        <i style={{ width: `${pct}%` }} />
        {h.down && upIn !== undefined && view.pace ? <i className="getup" style={{ width: `${Math.round(100 - (upIn * 100) / Math.max(1, view.pace.get_up_secs))}%` }} /> : null}
      </span>
      <span className="hp-label">{label}</span>
    </div>
  );
}

/** One button that heals an operative: to full when the stock pays, else as far as it goes. The
 *  engine prices both (`health.heal`); nothing shows before healing is open or at full health. */
export function HealButton({ o }: { o: OpView }) {
  const { view, run } = useCtx();
  const h = o.health;
  const heal = h.heal;
  if (!heal || h.away || (!h.down && h.hp >= h.max)) return null;
  const have = view.resources.medicine ?? 0;
  const full = heal.full <= have;
  const some = heal.reach > h.hp || (h.down && heal.reach > 0);
  return (
    <button
      className={'btn small' + (full ? ' primary' : '')}
      disabled={!full && !some}
      title={full ? undefined : t('sheets.heal.short', { reach: heal.reach })}
      onClick={() => run({ cmd: 'heal', op: o.id }, t('sheets.heal.done', { name: o.name }))}
    >
      <Icon id="res-medicine" size={14} />
      {full ? t('panels.heal.full', { cost: heal.full }) : some ? t('panels.heal.part', { reach: heal.reach, cost: have }) : t('panels.heal.none', { cost: heal.full })}
    </button>
  );
}

export function OpRow({ o, onClick, right }: { o: OpView; onClick?: () => void; right?: React.ReactNode }) {
  return (
    <button className="list-item" onClick={onClick}>
      <div className={'op-avatar' + (o.side === 'otherworld' ? ' otherworld' : '')}>
        <Icon id={o.icon} size={32} />
      </div>
      <div className="grow">
        <div className="row" style={{ gap: 6 }}>
          <b className="ellipsis">{o.name}</b>
          <Rarity r={o.rarity} name={o.rarity_name} />
          <span className="faint tiny ellipsis">{o.class_name}</span>
        </div>
        <div className="row wrap" style={{ gap: 4, marginTop: 2 }}>
          <TagIO reads={o.reads} writes={o.writes} />
        </div>
        <HpBar o={o} />
        <div className="tiny muted">{o.status}</div>
      </div>
      {right}
    </button>
  );
}

/** 用药线: on/off and the two lines (heal below a share of max HP; keep a reserve of medicine). */
function MedLineRow() {
  const { view, run } = useCtx();
  const m = view.med_line!;
  const set = (on: boolean, below_pct: number, reserve: number) =>
    run({ cmd: 'set_med_line', on, below_pct, reserve }, t('panels.med_line.saved'));
  return (
    <div className="row wrap small" style={{ gap: 6, marginTop: 8 }}>
      <b>{t('panels.med_line.title')}</b>
      <span className={m.short ? 'bad' : 'muted'}>
        {t(m.on ? 'panels.med_line.on' : 'panels.med_line.off', { pct: m.below_pct, reserve: m.reserve })}
        {m.short ? t('panels.med_line.short') : ''}
      </span>
      <button className="btn small" onClick={() => set(!m.on, m.below_pct, m.reserve)}>
        {t(m.on ? 'panels.med_line.turn_off' : 'panels.med_line.turn_on')}
      </button>
      <button className="btn small ghost" onClick={() => set(m.on, Math.max(10, m.below_pct - 10), m.reserve)}>
        {t('panels.med_line.lower')}
      </button>
      <button className="btn small ghost" onClick={() => set(m.on, Math.min(100, m.below_pct + 10), m.reserve)}>
        {t('panels.med_line.higher')}
      </button>
      <button className="btn small ghost" onClick={() => set(m.on, m.below_pct, Math.max(0, m.reserve - 10))}>
        {t('panels.med_line.less')}
      </button>
      <button className="btn small ghost" onClick={() => set(m.on, m.below_pct, m.reserve + 10)}>
        {t('panels.med_line.more')}
      </button>
    </div>
  );
}

export function RosterPanel() {
  const { view, open, run } = useCtx();
  const roster = (
    <Section title={t('panels.roster.title')} icon="roster" hint={shows(view, 'hire') ? `${view.operatives.length}/${view.roster_cap}` : undefined}>
      <div className="list">
        {view.operatives.map((o) => (
          <OpRow key={o.id} o={o} onClick={() => open({ kind: 'op', op: o.id })} right={o.locked ? <Icon id="lock" className="faint" /> : null} />
        ))}
      </div>
      {view.med_line ? <MedLineRow /> : null}
    </Section>
  );
  return (
    <div className="split">
      <div className="col">{roster}</div>
      <div className="col">
        {view.candidates ? (
        <Section title={t('panels.candidates.title')} icon="candidate" tone="good" hint={view.queue ? t('panels.candidates.queue', { n: view.queue }) : t('panels.candidates.count', { n: view.candidates.length })}>
          {view.candidates.length === 0 ? (
            <div className="empty">{t('panels.candidates.empty')}</div>
          ) : (
            <div className="list">
              {view.candidates.map((c) => (
                <OpRow
                  key={c.id}
                  o={c.op}
                  onClick={() => open({ kind: 'candidate', candidate: c.id })}
                  right={<span className="tiny faint">{c.source}</span>}
                />
              ))}
            </div>
          )}
          {view.districts.some((d) => d.customize?.unlocked) ? (
            <div className="tiny faint" style={{ marginTop: 6 }}>
              {t('panels.candidates.custom_open')}
            </div>
          ) : null}
        </Section>
        ) : null}
        {view.inventory ? (
        <Section title={t('panels.inventory.title')} icon="equip" tone="rust" hint={t('panels.inventory.count', { n: view.inventory.length })}>
          {view.inventory.length === 0 ? (
            <div className="empty">{t('panels.inventory.empty')}</div>
          ) : (
            <div className="list">
              {view.inventory.map((it, k) => (
                <div key={k} className="list-item">
                  <span className="op-avatar">
                    <Icon id={it.icon} size={32} />
                  </span>
                  <div className="grow">
                    <b>{it.name}</b>
                    <div className="small muted">{it.desc}</div>
                    <TagIO reads={it.reads} writes={it.writes} />
                  </div>
                  {view.sealed?.some((s) => s.id === it.id) ? (
                    <span className="small muted">{t('panels.sealed.title')}</span>
                  ) : (
                    <button className="btn small ghost" title={t('panels.inventory.seal_title')} onClick={() => run({ cmd: 'seal', item: it.id, on: true }, t('panels.inventory.sealed', { name: it.name }))}>
                      <Icon id="seal" size={14} /> {t('panels.inventory.seal')}
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </Section>
        ) : null}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// 街区 tab

export function DistrictsPanel() {
  const { view, open } = useCtx();
  const ds = [...view.districts].sort((a, b) => b.owned - a.owned || a.id - b.id);
  return (
    <Section title={t('panels.districts.title')} icon="trust" tone="good" hint={t('panels.districts.hint')}>
      <div className="district-grid">
        {ds.map((d) => (
          <button key={d.id} className="card district-card" onClick={() => open({ kind: 'district', district: d.id })} style={d.owned ? undefined : { opacity: 0.6 }}>
            <div className="row">
              <span className="swatch" style={{ background: WASH[d.id % WASH.length] }} />
              <h3 className="grow ellipsis" style={{ fontSize: 17 }}>
                {d.name}
              </h3>
              {d.policy ? <span className="chip good">{t('panels.districts.policy')}</span> : d.policy_unlocked ? <span className="chip warn">{t('panels.districts.policy_open')}</span> : null}
            </div>
            <div className="row small" style={{ gap: 10 }}>
              <span>
                <span className="faint">{t('panels.districts.tiles')}</span> <span className="num">{d.owned}/{d.total}</span>
              </span>
              <span>
                <span className="faint">{t('panels.districts.population')}</span> <span className="num">{d.population}</span>
              </span>
              {view.threat ? (
                <span>
                  <span className="faint">{t('panels.districts.militia')}</span> <span className="num">{d.militia}</span>
                </span>
              ) : null}
              {d.contiguous ? <span className="chip good">{t('panels.districts.contiguous')}</span> : null}
              {d.unhoused ? <span className="chip bad">{t('panels.districts.unhoused', { n: d.unhoused })}</span> : null}
            </div>
            {d.owned ? (
              <div className="row small">
                <span className="faint">{t('panels.districts.trust')}</span>
                <div className="grow">
                  <Bar value={d.trust} max={100} tone={d.trust >= 60 ? 'good' : d.trust < 35 ? 'bad' : undefined} />
                </div>
                <span className="num">{d.trust}</span>
                <span className={'tiny ' + (d.trust_target > d.trust ? 'good' : d.trust_target < d.trust ? 'bad' : 'faint')}>→{d.trust_target}</span>
              </div>
            ) : (
              <div className="tiny faint">{t('panels.districts.no_land')}</div>
            )}
            <div className="row tiny muted">
              <Icon id={d.flavor.icon} /> {d.flavor.name}
            </div>
          </button>
        ))}
      </div>
    </Section>
  );
}

// ---------------------------------------------------------------------------
// 总部 tab

export function HQPanel() {
  const { view, run, game, toast, restart } = useCtx();
  const org = view.org;
  const [saveText, setSaveText] = useState('');
  return (
    <>
    {view.phase === 'active' && view.armory?.length ? <ArmoryPanel /> : null}
    {/* Between cycles the map choice shows the 密室 at its top already. */}
    {view.phase === 'active' ? <RecordsPanel /> : null}
    <div className="split">
      <div className="col">
        {shows(view, 'org_level') ? (
        <Section title={t('panels.org.title', { level: org.level })} icon="org" tone="hazard" hint={org.next ? `${org.exp}/${org.next}` : `${org.exp}`}>
          {org.next ? <Bar value={org.exp} max={org.next} /> : null}
          <div className="stack" style={{ gap: 2, marginTop: 8 }}>
            {(org.perks ?? []).map((p) => (
              <div key={p.level + p.text} className={'small ' + (p.unlocked ? '' : 'faint')}>
                {p.unlocked ? '✓' : '·'} Lv.{p.level} {p.text}
              </div>
            ))}
          </div>
        </Section>
        ) : null}

        {org.rules ? (
        <Section title={t('panels.rules.title')} icon="rules" hint={t('panels.rules.hint')}>
          <div className="stack" style={{ gap: 6 }}>
            {org.rules.map((r) => (
              <label key={r.id} className="switch" style={{ alignItems: 'flex-start' }}>
                <input type="checkbox" checked={r.on} onChange={(e) => run({ cmd: 'set_rule', rule: r.id, on: e.target.checked })} />
                <span>
                  <b>{r.name}</b>
                  <div className="small muted">{r.desc}</div>
                </span>
              </label>
            ))}
          </div>
        </Section>
        ) : null}

        {view.phase === 'active' && shows(view, 'collapse') ? <CycleBlock /> : null}
      </div>
      <div className="col">
        {/* A fixed map (the tutorial) has no special operatives to meet. */}
        {view.pace && view.phase === 'active' ? null : (
        <Section title={t('panels.specials.title')} icon="star" tone="hazard" hint={t('panels.specials.hint')}>
          <div className="stack" style={{ gap: 8 }}>
            {org.specials.map((s) => (
              <div key={s.id} className="card flat">
                <div className="row">
                  <Icon id={s.icon} size={20} />
                  <b>{s.name}</b>
                  <span className="faint tiny">{s.side}</span>
                  <span className="grow" />
                  {s.recruited ? <span className="chip good">{t('panels.specials.recruited')}</span> : s.met ? <span className="chip warn">{t('panels.specials.met')}</span> : null}
                </div>
                <div className="small">{s.rule}</div>
                <div className="small muted">
                  {t('panels.specials.condition', { hint: s.hint, have: Math.min(s.have, s.need), need: s.need })}
                </div>
                {!s.met ? <Bar value={s.have} max={s.need} /> : null}
              </div>
            ))}
          </div>
        </Section>
        )}

        {view.sealed ? (
        <Section title={t('panels.sealed.title')} icon="seal" tone="rift" hint={`${view.sealed.length}/${org.sealed_slots}`}>
          {view.sealed.length === 0 ? <div className="empty">{t('panels.sealed.empty')}</div> : null}
          {view.sealed.map((it, k) => (
            <div key={k} className="row small">
              <Icon id={it.icon} size={16} />
              {it.name}
              <button className="btn small ghost" onClick={() => run({ cmd: 'seal', item: it.id, on: false })}>
                {t('panels.sealed.unseal')}
              </button>
            </div>
          ))}
        </Section>
        ) : null}

        {org.history.length ? (
          <Section title={t('panels.history.title')} icon="report">
            {org.history.map((h) => (
              <div key={h.number} className="row small">
                <span className="grow">
                  {t('panels.history.cycle', { n: h.number, modifier: h.modifier_name })}
                </span>
                <span className="num">+{h.exp}</span>
              </div>
            ))}
          </Section>
        ) : null}

        <Section title={t('panels.debug.title')} icon="clock">
          <div className="row wrap" style={{ marginBottom: 8 }}>
            <button className="btn danger" onClick={restart}>
              {t('app.restart.button')}
            </button>
            <span className="small faint">{t('app.restart.hint')}</span>
          </div>
          <details>
          <summary className="small muted" style={{ cursor: 'pointer' }}>{t('panels.debug.more')}</summary>
          {/* Skipping hours on a fixed map (whose hour is seconds) only piles up stock. */}
          {view.pace ? null : (
          <>
          <div className="row wrap">
            {[
              [3600, t('panels.debug.hours', { n: 1 })],
              [4 * 3600, t('panels.debug.hours', { n: 4 })],
              [12 * 3600, t('panels.debug.hours', { n: 12 })],
              [24 * 3600, t('panels.debug.days', { n: 1 })],
            ].map(([s, l]) => (
              <button
                key={l as string}
                className="btn small"
                onClick={() => {
                  const t0 = performance.now();
                  const n = game.skip(s as number);
                  toast(t('panels.debug.skipped', { label: l as string, n, ms: Math.round(performance.now() - t0) }));
                }}
              >
                {l}
              </button>
            ))}
          </div>
          <div className="small faint" style={{ marginTop: 4 }}>
            {t('panels.debug.note', { h: Math.round(game.offset / 3600) })}
          </div>
          </>
          )}
          <div className="row wrap" style={{ marginTop: 8 }}>
            <button className="btn small" onClick={() => setSaveText(game.exportSave())}>
              {t('panels.debug.export')}
            </button>
            <button
              className="btn small"
              onClick={() => {
                const err = game.importSave(saveText);
                toast(err ?? t('panels.debug.loaded'), !!err);
              }}
              disabled={!saveText.trim()}
            >
              {t('panels.debug.import')}
            </button>
          </div>
          <textarea className="save" value={saveText} onChange={(e) => setSaveText(e.target.value)} placeholder={t('panels.debug.save_placeholder')} />
          </details>
        </Section>
      </div>
    </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// 军械: three tracks bought with scrap; every few levels a track stops until a breakthrough
// (crystal + parts) multiplies it. Levels belong to the cycle. The engine prices every step and
// says what it does (ArmoryView); this only lays it out.

/** Combined bonus of every track on one target, multiplied the way the engine stacks them. */
function armoryTotal(tracks: ArmoryView[], target: ArmoryView['target']): { hp: number; atk: number } {
  let hp = 1;
  let atk = 1;
  for (const a of tracks.filter((a) => a.target === target)) {
    hp *= 1 + a.hp_pct / 100;
    atk *= 1 + a.atk_pct / 100;
  }
  return { hp: Math.round((hp - 1) * 100), atk: Math.round((atk - 1) * 100) };
}

export function ArmoryPanel() {
  const { view } = useCtx();
  const tracks = view.armory ?? [];
  const targets = (['squad', 'militia'] as const).filter((x) => tracks.some((a) => a.target === x));
  return (
    <Section id="armory" title={t('panels.armory.title')} icon="atk" tone="rust" hint={t('panels.armory.hint')}>
      <div className="armory-sum">
        {targets.map((x) => {
          const sum = armoryTotal(tracks, x);
          return (
            <span key={x}>
              <b>{t(`panels.armory.target.${x}`)}</b>
              <span className="faint">{t('sheets.stat.atk')}</span>
              <span className="num">{t('panels.armory.pct', { v: sum.atk })}</span>
              <span className="faint">{t('sheets.stat.hp')}</span>
              <span className="num">{t('panels.armory.pct', { v: sum.hp })}</span>
            </span>
          );
        })}
      </div>
      <div className="armory-grid">
        {tracks.map((a) => (
          <ArmoryCard key={a.id} a={a} />
        ))}
      </div>
    </Section>
  );
}

function ArmoryCard({ a }: { a: ArmoryView }) {
  const { view, run } = useCtx();
  // Flash the card once when a breakthrough lands.
  const seen = useRef(a.breaks);
  const [flash, setFlash] = useState(false);
  useEffect(() => {
    if (a.breaks > seen.current) {
      setFlash(true);
      const id = window.setTimeout(() => setFlash(false), 1300);
      seen.current = a.breaks;
      return () => window.clearTimeout(id);
    }
    seen.current = a.breaks;
  }, [a.breaks]);
  const gate = a.next === 'breakthrough';
  const maxed = a.next === 'maxed';
  // Levels inside the current stage, and how many breakthroughs the track has in all.
  const stage = a.every > 0 ? a.level - a.breaks * a.every : a.level;
  const cells = a.every > 0 ? a.every : a.max;
  const gates = a.every > 0 ? Math.floor((a.max - 1) / a.every) : 0;
  const rows: [string, number, number][] = [];
  if (a.atk_pct || a.next_atk_pct) rows.push([t('sheets.stat.atk'), a.atk_pct, a.next_atk_pct]);
  if (a.hp_pct || a.next_hp_pct) rows.push([t('sheets.stat.hp'), a.hp_pct, a.next_hp_pct]);
  const kind = a.target === 'militia' ? 't-militia' : a.icon === 'def' ? 't-armor' : 't-arms';
  return (
    <div className={'arm ' + kind + (gate ? ' gate' : '') + (maxed ? ' maxed' : '') + (flash ? ' flash' : '')}>
      <div className="arm-head">
        <span className="arm-ico">
          <Icon id={a.icon} size={32} />
        </span>
        <div className="grow">
          <div className="arm-name ellipsis">{a.name}</div>
          <span className="chip" style={{ color: 'var(--ink-3)' }}>
            {t(`panels.armory.target.${a.target}`)}
          </span>
        </div>
        <div className="arm-lv" title={t('panels.armory.level')}>
          <span className="num">{a.level}</span>
          <span className="of">/{a.max}</span>
        </div>
      </div>
      <div className="arm-track" aria-hidden>
        <div className="arm-cells">
          {Array.from({ length: cells }, (_, k) => (
            <i key={k} className={k < stage ? 'on' : ''} />
          ))}
        </div>
        {gates > 0 && a.breaks < gates ? <span className={'arm-gate' + (gate ? ' ready' : '')} /> : null}
      </div>
      {gates > 0 ? (
        <div className="arm-pips">
          {t('panels.armory.breaks')}
          {Array.from({ length: gates }, (_, k) => (
            <i key={k} className={k < a.breaks ? 'on' : ''} />
          ))}
        </div>
      ) : null}
      <div className="arm-bonus">
        {rows.map(([k, now, next]) => (
          <BonusRow key={k} k={k} now={now} next={next} maxed={maxed} />
        ))}
      </div>
      {gate ? (
        <div className="arm-gate-note">
          <Icon id="warn" />
          <span>{t('panels.armory.gate_note')}</span>
        </div>
      ) : (
        <div className="arm-desc">{a.desc}</div>
      )}
      <div className="arm-act">
        {maxed ? (
          <span className="maxed">{t('panels.armory.maxed')}</span>
        ) : (
          <button
            className={'btn ' + (gate ? 'break' : '')}
            disabled={!a.affordable}
            onClick={() =>
              run(
                { cmd: 'upgrade', track: a.id },
                gate ? t('panels.armory.broke', { name: a.name }) : t('panels.armory.upgraded', { name: a.name, level: a.level + 1 }),
              )
            }
          >
            {gate ? <Icon id="unlock" /> : <Icon id="plus" />}
            {gate ? t('panels.armory.breakthrough') : t('panels.armory.upgrade')}
            <Cost cost={a.cost} have={view.resources} />
          </button>
        )}
      </div>
    </div>
  );
}

function BonusRow({ k, now, next, maxed }: { k: string; now: number; next: number; maxed: boolean }) {
  return (
    <>
      <span className="k">{k}</span>
      <span className="num">{t('panels.armory.pct', { v: now })}</span>
      {maxed || next === now ? (
        <>
          <span />
          <span />
        </>
      ) : (
        <>
          <span className="arrow">→</span>
          <span className="num after">{t('panels.armory.pct', { v: next })}</span>
        </>
      )}
    </>
  );
}

function CycleBlock() {
  const { view, run, confirm } = useCtx();
  return (
    <Section title={t('panels.cycle.title', { n: view.cycle })} icon="collapse" tone="bad" hint={view.modifier?.[1]}>
      {view.modifier ? <div className="small muted">{view.modifier[2]}</div> : null}
      <div className="stack" style={{ gap: 2, marginTop: 6 }}>
        {(view.results?.lines ?? []).map(([k, v]) => (
          <div key={k} className="row small">
            <span className="grow">{k}</span>
            <span className="num">{v}</span>
          </div>
        ))}
        <div className="row">
          <b className="grow">{t('panels.cycle.exp')}</b>
          <b className="num">{view.results?.total ?? 0}</b>
        </div>
      </div>
      <div style={{ marginTop: 8 }}>
        <button
          className="btn danger small"
          onClick={() =>
            confirm({
              title: t('panels.cycle.confirm_title'),
              body: (
                <div className="split" style={{ gap: 12 }}>
                  <div className="col">
                    <b>{t('panels.cycle.cleared')}</b>
                    <div className="small muted">{t('panels.cycle.cleared_list')}</div>
                  </div>
                  <div className="col">
                    <b>{t('panels.cycle.kept')}</b>
                    <div className="small muted">
                      {t('panels.cycle.kept_list', {
                        exp: view.results?.total ?? 0,
                        sealed: view.sealed?.length ?? 0,
                        doctrines: (view.research?.archive ?? []).map((d) => d.name).join(t('panels.sep.list')) || t('panels.cycle.none_picked'),
                      })}
                    </div>
                  </div>
                </div>
              ),
              ok: t('panels.cycle.confirm_ok'),
              danger: true,
              onOk: () => run({ cmd: 'collapse' }, t('panels.cycle.collapsed')),
            })
          }
        >
          {t('panels.cycle.collapse')}
        </button>
      </div>
    </Section>
  );
}
