import { useEffect, useRef, useState } from 'react';
import { t } from '../i18n';
import { Icon } from '../icons';
import type { ArmoryView, OpView, PendingView, SquadView, TideView } from '../types';
import { WASH } from './Board';
import { Bar, Cost, fmtAgo, fmtClock, fmtIn, OUTCOME_NAMES, RES_KEYS, RES_NAMES, Rarity, Section, SQUAD_HEX, TagIO, useCtx } from './common';

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
  return (
    <div className="split">
      <div className="col">
        <Apprentice />
        <SquadsPanel />
      </div>
      <div className="col">
        <TideTable limit={10} />
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
  if (!tasks.open.length) return null;
  return (
    <Section title={t('panels.tasks.title')} icon="check" tone="hazard" hint={`${tasks.claimed}/${tasks.total}`}>
      <div className="stack" style={{ gap: 6 }}>
        {tasks.open.map((x) => {
          const reward = RES_KEYS.filter((k) => x.reward[k] > 0).map((k) => `${RES_NAMES[k]} +${x.reward[k]}`);
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
    <Section title={t('panels.squads.title')} icon="squad" tone="elec" hint={t('panels.squads.attention', { n: view.attention[0], ring: view.attention[1] })}>
      <div className="stack">
        {view.squads.map((s, i) => (
          <SquadCard key={s.id} s={s} color={SQUAD_HEX[i % SQUAD_HEX.length]!} />
        ))}
      </div>
      {view.attention[0] > 0 ? (
        <div className="small warn" style={{ marginTop: 6 }}>
          {t('panels.squads.attention_note', { x: (view.attention[2] / 10000).toFixed(1) })}
        </div>
      ) : null}
    </Section>
  );
}

function SquadCard({ s, color }: { s: SquadView; color: string }) {
  const { view, now, open, setRouteEdit } = useCtx();
  const members = s.members.map((id) => view.operatives.find((o) => o.id === id)).filter(Boolean) as OpView[];
  const trip = s.running;
  const status = !members.length
    ? t('panels.squad.no_members')
    : trip
      ? t('panels.squad.on_route', { left: fmtIn(trip.end - now) })
      : s.rest_until > now
        ? t('panels.squad.resting', { left: fmtIn(s.rest_until - now) })
        : s.route.length
          ? t('panels.squad.ready')
          : t('panels.squad.no_route');
  return (
    <div className="card" style={{ boxShadow: `inset 4px 0 0 ${color}`, paddingLeft: 14 }}>
      <div className="row">
        <h3 style={{ color, fontSize: 17 }}>{s.name}</h3>
        <span className="small muted grow">{status}</span>
        <button className="btn small" onClick={() => setRouteEdit({ squad: s.id, route: s.route })} disabled={!members.length}>
          <Icon id="route" size={14} /> {t('panels.squad.route')}
        </button>
        <button className="btn small ghost" onClick={() => open({ kind: 'squad', squad: s.id })}>
          {t('panels.squad.details')}
        </button>
      </div>
      <div className="row wrap small" style={{ gap: 8, marginTop: 4 }}>
        {members.map((m) => (
          <span key={m.id} className="row" style={{ gap: 3 }}>
            <Icon id={m.icon} />
            {m.name}
          </span>
        ))}
      </div>
      {s.route.length ? (
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
        <div className="tiny muted">{o.status}</div>
      </div>
      {right}
    </button>
  );
}

export function RosterPanel() {
  const { view, open, run } = useCtx();
  const roster = (
    <Section title={t('panels.roster.title')} icon="roster" hint={`${view.operatives.length}/${view.roster_cap}`}>
      <div className="list">
        {view.operatives.map((o) => (
          <OpRow key={o.id} o={o} onClick={() => open({ kind: 'op', op: o.id })} right={o.locked ? <Icon id="lock" className="faint" /> : null} />
        ))}
      </div>
    </Section>
  );
  return (
    <div className="split">
      <div className="col">{roster}</div>
      <div className="col">
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
          {view.districts.some((d) => d.customize.unlocked) ? (
            <div className="tiny faint" style={{ marginTop: 6 }}>
              {t('panels.candidates.custom_open')}
            </div>
          ) : null}
        </Section>
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
                  {view.sealed.some((s) => s.id === it.id) ? (
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
              <span>
                <span className="faint">{t('panels.districts.militia')}</span> <span className="num">{d.militia}</span>
              </span>
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
  const { view, run, game, toast, confirm } = useCtx();
  const org = view.org;
  const [saveText, setSaveText] = useState('');
  return (
    <>
    {view.phase === 'active' && view.armory.length ? <ArmoryPanel /> : null}
    <div className="split">
      <div className="col">
        <Section title={t('panels.org.title', { level: org.level })} icon="org" tone="hazard" hint={org.next ? `${org.exp}/${org.next}` : `${org.exp}`}>
          {org.next ? <Bar value={org.exp} max={org.next} /> : null}
          <div className="stack" style={{ gap: 2, marginTop: 8 }}>
            {org.perks.map((p) => (
              <div key={p.level + p.text} className={'small ' + (p.unlocked ? '' : 'faint')}>
                {p.unlocked ? '✓' : '·'} Lv.{p.level} {p.text}
              </div>
            ))}
          </div>
        </Section>

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

        {view.phase === 'active' ? <CycleBlock /> : null}
      </div>
      <div className="col">
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

        {org.history.length ? (
          <Section title={t('panels.history.title')} icon="report">
            {org.history.map((h) => (
              <div key={h.number} className="row small">
                <span className="grow">
                  {t('panels.history.cycle', { n: h.number, modifier: h.modifier })}
                </span>
                <span className="num">+{h.exp}</span>
              </div>
            ))}
          </Section>
        ) : null}

        <Section title={t('panels.debug.title')} icon="clock">
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
            <button
              className="btn small danger"
              onClick={() =>
                confirm({
                  title: t('panels.debug.reset_title'),
                  body: t('panels.debug.reset_body'),
                  ok: t('panels.debug.reset_ok'),
                  danger: true,
                  onOk: () => {
                    game.reset();
                    toast(t('panels.debug.reset_done'));
                  },
                })
              }
            >
              {t('panels.debug.reset')}
            </button>
          </div>
          <textarea className="save" value={saveText} onChange={(e) => setSaveText(e.target.value)} placeholder={t('panels.debug.save_placeholder')} />
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
  const tracks = view.armory;
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
        {view.results.lines.map(([k, v]) => (
          <div key={k} className="row small">
            <span className="grow">{k}</span>
            <span className="num">{v}</span>
          </div>
        ))}
        <div className="row">
          <b className="grow">{t('panels.cycle.exp')}</b>
          <b className="num">{view.results.total}</b>
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
                        exp: view.results.total,
                        sealed: view.sealed.length,
                        doctrines: view.research.archive.map((d) => d.name).join(t('panels.sep.list')) || t('panels.cycle.none_picked'),
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
