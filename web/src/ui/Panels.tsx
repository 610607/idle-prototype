import { useState } from 'react';
import { Icon } from '../icons';
import type { OpView, PendingView, SquadView, TideView } from '../types';
import { Bar, fmtAgo, fmtClock, fmtIn, OUTCOME_NAMES, Rarity, Section, SQUAD_HEX, TagIO, useCtx } from './common';

const PENDING_ICON: Record<string, string> = {
  map: 'map',
  alert: 'alert',
  claim: 'check',
  candidate: 'candidate',
  policy: 'policy',
  route: 'route',
  defend: 'garrison',
};

// ---------------------------------------------------------------------------
// 地图 tab

export function MapPanel() {
  return (
    <>
      <PendingList />
      <TideTable limit={8} />
      <SquadsPanel />
      <EventLog />
    </>
  );
}

export function PendingList() {
  const { view, open, setFocusTile, setRouteEdit } = useCtx();
  const items = view.pending;
  const act = (p: PendingView) => {
    if (p.tile !== undefined) setFocusTile(p.tile);
    if (p.kind === 'candidate' && p.candidate !== undefined) open({ kind: 'candidate', candidate: p.candidate });
    else if (p.kind === 'policy' && p.district !== undefined) open({ kind: 'district', district: p.district });
    else if (p.kind === 'route') {
      const s = view.squads.find((s) => p.text.startsWith(s.name));
      if (s) setRouteEdit({ squad: s.id, route: s.route.filter((i) => !view.tiles[i]!.owned) });
    } else if (p.tile !== undefined) open({ kind: 'tile', tile: p.tile });
  };
  return (
    <Section title="待你决定" icon="event" hint={items.length ? `${items.length} 件` : '没有'}>
      {items.length === 0 ? (
        <div className="empty">都安排好了。挂着就行，回来再看。</div>
      ) : (
        <div className="stack" style={{ gap: 6 }}>
          {items.map((p, k) => (
            <button key={k} className={'pending-item ' + p.kind} onClick={() => act(p)}>
              <Icon id={PENDING_ICON[p.kind] ?? 'event'} size={18} />
              <span className="grow small">{p.text}</span>
              <Icon id="chevron-right" size={14} />
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
          <th>时间</th>
          <th>格子</th>
          <th>结果</th>
          <th>镇守度</th>
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
            <td>{e.skipped ? '告急中，跳过' : e.held ? '守住' : `第 ${e.lost_wave}/${e.total_waves} 波失守`}</td>
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
  const { view, open } = useCtx();
  const rows = limit ? view.tide.slice(0, limit) : view.tide;
  const lost = view.tide.filter((e) => !e.held && !e.skipped).length;
  return (
    <Section
      title="潮汐表"
      icon="tide"
      hint={
        <span>
          12 小时 {view.tide.length} 次{lost ? <b className="bad"> · 失守 {lost}</b> : null}
        </span>
      }
    >
      {rows.length === 0 ? <div className="empty">暂时没有来潮。</div> : <TideRows rows={rows} />}
      {limit && view.tide.length > limit ? (
        <button className="btn small ghost" onClick={() => open({ kind: 'tide' })}>
          全部 {view.tide.length} 次 →
        </button>
      ) : null}
    </Section>
  );
}

export function SquadsPanel() {
  const { view } = useCtx();
  return (
    <Section title="小队" icon="squad" hint={`关注度 ${view.attention[0]}（最深第 ${view.attention[1]} 圈）`}>
      <div className="stack">
        {view.squads.map((s, i) => (
          <SquadCard key={s.id} s={s} color={SQUAD_HEX[i % SQUAD_HEX.length]!} />
        ))}
      </div>
      {view.attention[0] > 0 ? (
        <div className="small warn" style={{ marginTop: 6 }}>
          路线越深，异界越注意你：来潮强度 ×{(view.attention[2] / 10000).toFixed(1)}。
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
    ? '没有队员'
    : trip
      ? `路线中 · ${fmtIn(trip.end - now)}后回来`
      : s.rest_until > now
        ? `休整 · ${fmtIn(s.rest_until - now)}`
        : s.route.length
          ? '待命'
          : '没有路线';
  return (
    <div className="card flat" style={{ borderLeft: `6px solid ${color}` }}>
      <div className="row">
        <b>{s.name}</b>
        <span className="small muted grow">{status}</span>
        <button className="btn small" onClick={() => setRouteEdit({ squad: s.id, route: s.route })} disabled={!members.length}>
          <Icon id="route" size={14} /> 路线
        </button>
        <button className="btn small ghost" onClick={() => open({ kind: 'squad', squad: s.id })}>
          详情
        </button>
      </div>
      <div className="row wrap small" style={{ gap: 8, marginTop: 4 }}>
        {members.map((m) => (
          <span key={m.id} className="row" style={{ gap: 3 }}>
            <Icon id={m.icon} size={14} />
            {m.name}
          </span>
        ))}
      </div>
      {s.route.length ? (
        <div className="small faint" style={{ marginTop: 2 }}>
          路线：{s.route.map((i) => view.tiles[i]!.label).join(' → ')}
        </div>
      ) : null}
      {trip ? <Bar value={now - trip.start} max={trip.end - trip.start} /> : null}
      {s.last ? (
        <div className={'small ' + (s.last.outcome === 'victory' ? 'good' : 'bad')} style={{ marginTop: 2 }}>
          上一趟：{OUTCOME_NAMES[s.last.outcome]}
          {s.last.results.length ? `（${s.last.results.map((r) => r.label).join('、')}）` : '（只巡逻）'}
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
    <Section title="动态" icon="event">
      <div className="list">
        {events.map((e) => (
          <div key={e.id} className="list-item small" style={{ alignItems: 'flex-start' }}>
            <Icon id={EVENT_ICON[e.kind] ?? 'event'} size={16} className={e.kind === 'alert' || e.kind === 'surge_lost' ? 'bad' : ''} />
            <span className="grow">{e.text}</span>
            {e.report !== undefined ? (
              <button className="btn small ghost" onClick={() => open({ kind: 'report', title: e.text, load: () => game.report(e.report!) })}>
                战报
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
          更多
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
      <div className={'op-avatar' + (o.side === 'otherworld' || o.side === '异界' ? ' otherworld' : '')}>
        <Icon id={o.icon} size={22} />
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
  const { view, open } = useCtx();
  return (
    <>
      <Section title="候选" icon="candidate" hint={view.queue ? `排队 ${view.queue}` : `${view.candidates.length} 人`}>
        {view.candidates.length === 0 ? (
          <div className="empty">街区信任和人才积累会带来候选人；镇压的裂隙也会。</div>
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
      </Section>
      <Section title="队员" icon="roster" hint={`${view.operatives.length}/${view.roster_cap}`}>
        <div className="list">
          {view.operatives.map((o) => (
            <OpRow key={o.id} o={o} onClick={() => open({ kind: 'op', op: o.id })} right={o.locked ? <Icon id="lock" size={14} /> : null} />
          ))}
        </div>
      </Section>
      <Section title="仓库" icon="equip" hint={`${view.inventory.length} 件`}>
        {view.inventory.length === 0 ? (
          <div className="empty">路线上偶尔掉装备；物资点也有。</div>
        ) : (
          <div className="list">
            {view.inventory.map((it, k) => (
              <div key={k} className="list-item">
                <Icon id={it.icon} size={20} />
                <div className="grow">
                  <b>{it.name}</b>
                  <div className="small muted">{it.desc}</div>
                  <TagIO reads={it.reads} writes={it.writes} />
                </div>
              </div>
            ))}
          </div>
        )}
      </Section>
    </>
  );
}

// ---------------------------------------------------------------------------
// 总部 tab

export function HQPanel() {
  const { view, run, game, toast } = useCtx();
  const org = view.org;
  const [saveText, setSaveText] = useState('');
  const [confirmCollapse, setConfirmCollapse] = useState(false);
  return (
    <>
      <Section title={`组织 Lv.${org.level}`} icon="org" hint={org.next ? `${org.exp}/${org.next}` : `${org.exp}`}>
        {org.next ? <Bar value={org.exp} max={org.next} /> : null}
        <div className="stack" style={{ gap: 2, marginTop: 8 }}>
          {org.perks.map((p) => (
            <div key={p.level} className={'small ' + (p.unlocked ? '' : 'faint')}>
              {p.unlocked ? '✓' : '·'} Lv.{p.level} {p.text}
            </div>
          ))}
        </div>
      </Section>

      <Section title="规章" icon="rules" hint="自动替你做的事">
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

      <Section title="特殊人物" icon="star" hint="满足条件后进入候选">
        <div className="stack" style={{ gap: 8 }}>
          {org.specials.map((s) => (
            <div key={s.id} className="card flat">
              <div className="row">
                <Icon id={s.icon} size={20} />
                <b>{s.name}</b>
                <span className="faint tiny">{s.side}</span>
                <span className="grow" />
                {s.recruited ? <span className="chip good">已加入</span> : s.met ? <span className="chip">已出现</span> : null}
              </div>
              <div className="small">{s.rule}</div>
              <div className="small muted">
                条件：{s.hint}（{Math.min(s.have, s.need)}/{s.need}）
              </div>
              {!s.met ? <Bar value={s.have} max={s.need} /> : null}
            </div>
          ))}
        </div>
      </Section>

      {view.phase === 'active' ? (
        <Section title={`第 ${view.cycle} 轮`} icon="collapse" hint={view.modifier?.[1]}>
          {view.modifier ? <div className="small muted">{view.modifier[2]}</div> : null}
          <div className="stack" style={{ gap: 2, marginTop: 6 }}>
            {view.results.lines.map(([k, v]) => (
              <div key={k} className="row small">
                <span className="grow">{k}</span>
                <span className="num">{v}</span>
              </div>
            ))}
            <div className="row">
              <b className="grow">结算经验</b>
              <b className="num">{view.results.total}</b>
            </div>
          </div>
          <div style={{ marginTop: 8 }}>
            {!confirmCollapse ? (
              <button className="btn danger small" onClick={() => setConfirmCollapse(true)}>
                收束这一轮…
              </button>
            ) : (
              <div className="card flat" style={{ borderColor: 'var(--bad)' }}>
                <p className="small" style={{ marginTop: 0 }}>
                  地图、街区、普通队员都会清空；组织经验、规章、特殊人物和封存的装备留下。
                </p>
                <div className="row">
                  <button className="btn danger" onClick={() => run({ cmd: 'collapse' }, '这一轮收束了') && setConfirmCollapse(false)}>
                    确定收束
                  </button>
                  <button className="btn ghost" onClick={() => setConfirmCollapse(false)}>
                    取消
                  </button>
                </div>
              </div>
            )}
          </div>
        </Section>
      ) : null}

      <Section title="封存" icon="seal" hint={`${view.sealed.length}/${org.sealed_slots}`}>
        {view.sealed.length === 0 ? <div className="empty">封存的装备会带进下一轮。在队员详情里封存。</div> : null}
        {view.sealed.map((it, k) => (
          <div key={k} className="row small">
            <Icon id={it.icon} size={16} />
            {it.name}
            <button className="btn small ghost" onClick={() => run({ cmd: 'seal', item: it.id, on: false })}>
              取消封存
            </button>
          </div>
        ))}
      </Section>

      {org.history.length ? (
        <Section title="往轮" icon="report">
          {org.history.map((h) => (
            <div key={h.number} className="row small">
              <span className="grow">
                第 {h.number} 轮 · {h.modifier}
              </span>
              <span className="num">+{h.exp}</span>
            </div>
          ))}
        </Section>
      ) : null}

      <Section title="存档与调试" icon="clock">
        <div className="row wrap">
          {[
            [3600, '+1 时'],
            [4 * 3600, '+4 时'],
            [12 * 3600, '+12 时'],
            [24 * 3600, '+1 天'],
          ].map(([s, l]) => (
            <button
              key={l as string}
              className="btn small"
              onClick={() => {
                const t0 = performance.now();
                const n = game.skip(s as number);
                toast(`快进 ${l}：${n} 条新动态（${Math.round(performance.now() - t0)} ms）`);
              }}
            >
              {l}
            </button>
          ))}
        </div>
        <div className="small faint" style={{ marginTop: 4 }}>
          快进走的是和离线回来一样的离散事件模拟。当前时钟偏移 {Math.round(game.offset / 3600)} 小时。
        </div>
        <div className="row wrap" style={{ marginTop: 8 }}>
          <button className="btn small" onClick={() => setSaveText(game.exportSave())}>
            导出存档
          </button>
          <button
            className="btn small"
            onClick={() => {
              const err = game.importSave(saveText);
              toast(err ?? '已读档', !!err);
            }}
            disabled={!saveText.trim()}
          >
            从文本读档
          </button>
          <button
            className="btn small danger"
            onClick={() => {
              if (window.confirm('清空存档，重新开始？')) {
                game.reset();
                toast('新游戏');
              }
            }}
          >
            重新开始
          </button>
        </div>
        <textarea className="save" value={saveText} onChange={(e) => setSaveText(e.target.value)} placeholder="导出的存档会出现在这里；也可以粘贴存档再读档。" />
      </Section>
    </>
  );
}
