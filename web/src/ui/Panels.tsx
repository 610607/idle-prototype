import { useState } from 'react';
import { Icon } from '../icons';
import type { OpView, PendingView, SquadView, TideView } from '../types';
import { WASH } from './Board';
import { Bar, fmtAgo, fmtClock, fmtIn, OUTCOME_NAMES, RES_KEYS, RES_NAMES, Rarity, Section, SQUAD_HEX, TagIO, useCtx } from './common';

const PENDING_ICON: Record<string, string> = {
  map: 'map',
  alert: 'alert',
  claim: 'check',
  candidate: 'candidate',
  policy: 'policy',
  route: 'route',
  defend: 'garrison',
  research: 'trait-book',
};

// ---------------------------------------------------------------------------
// 指挥 tab: squads and what happened on the left, the full tide table on the right.

export function CommandPanel() {
  return (
    <div className="split">
      <div className="col">
        <Apprentice />
        <SquadsPanel />
        <EventLog />
      </div>
      <div className="col">
        <TideTable />
      </div>
    </div>
  );
}

/** The next 12 hours as one strip: every surge is a mark, lost ones stand out. */
export function TideStrip() {
  const { view, now, open, setFocusTile } = useCtx();
  const H = 12 * 3600;
  const W = 480;
  const lost = view.tide.filter((e) => !e.held && !e.skipped).length;
  const x = (t: number) => 4 + ((t - now) / H) * (W - 8);
  const hours = Array.from({ length: 13 }, (_, k) => k);
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
        <Icon id="tide" />
        <h4>潮汐 · 12 小时</h4>
        <span className="grow">
          {view.tide.length} 次{lost ? <b className="bad"> · 预计失守 {lost}</b> : <span className="good"> · 全部守住</span>}
        </span>
        <button className="btn ghost small" onClick={() => open({ kind: 'tide' }, true)}>
          全表 →
        </button>
      </div>
      <svg viewBox={`0 0 ${W} 40`} preserveAspectRatio="none" role="img" aria-label="未来 12 小时来潮">
        <rect x={0} y={4} width={W} height={22} fill="var(--sunk)" />
        {hours.map((h) => (
          <g key={h}>
            <rect x={x(now + h * 3600) - 0.5} y={h % 4 === 0 ? 4 : 20} width={1} height={h % 4 === 0 ? 22 : 6} fill="var(--line)" />
            {h % 3 === 0 ? (
              <text x={x(now + h * 3600)} y={38} textAnchor={h === 0 ? 'start' : h === 12 ? 'end' : 'middle'}>
                {h === 0 ? '现在' : fmtClock(now + h * 3600)}
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
            <title>{`${fmtClock(e.t)} ${e.label}：${e.skipped ? '告急中，跳过' : e.held ? '守住' : `第 ${e.lost_wave} 波失守`}`}</title>
          </g>
        ))}
      </svg>
    </div>
  );
}

/** 见习任务: the next few "do it once" steps; hidden once every task is claimed. */
function Apprentice() {
  const { view, run } = useCtx();
  const t = view.tasks;
  if (!t.open.length) return null;
  return (
    <Section title="见习任务" icon="check" hint={`${t.claimed}/${t.total}`}>
      <div className="stack" style={{ gap: 6 }}>
        {t.open.map((x) => {
          const reward = RES_KEYS.filter((k) => x.reward[k] > 0).map((k) => `${RES_NAMES[k]} +${x.reward[k]}`);
          return (
            <div key={x.id} className={'task-item' + (x.done ? ' done' : '')}>
              <div className="row">
                <span className="chip">{x.group}</span>
                <b>{x.name}</b>
                <span className="grow" />
                {reward.length ? <span className="tiny muted">{reward.join(' · ')}</span> : null}
                {x.done ? (
                  <button className="btn small primary" onClick={() => run({ cmd: 'claim_task', task: x.id }, `见习任务完成：${x.name}`)}>
                    领取
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
    else if (p.kind === 'route') {
      const s = view.squads.find((s) => p.text.startsWith(s.name));
      if (s) setRouteEdit({ squad: s.id, route: s.route.filter((i) => !view.tiles[i]!.owned) });
    } else if (p.tile !== undefined) open({ kind: 'tile', tile: p.tile }, true);
  };
  return (
    <Section title="待你决定" icon="event" hint={items.length ? `${items.length} 件` : '没有'}>
      {items.length === 0 ? (
        <div className="empty">都安排好了。挂着就行，回来再看。</div>
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
  const lostOnly = view.tide.filter((e) => !e.held && !e.skipped);
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
      {lostOnly.length ? (
        <div className="card striped bad" style={{ marginBottom: 10 }}>
          <div className="small">
            <b className="bad">预计失守：</b>
            {[...new Set(lostOnly.map((e) => e.label))].join('、')}。点行进格子，挑一个守得住的人派驻。
          </div>
        </div>
      ) : null}
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
    <div className="card" style={{ boxShadow: `inset 4px 0 0 ${color}`, paddingLeft: 14 }}>
      <div className="row">
        <h3 style={{ color, fontSize: 17 }}>{s.name}</h3>
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
            <Icon id={m.icon} />
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
            <Icon id={EVENT_ICON[e.kind] ?? 'event'} className={e.kind === 'alert' || e.kind === 'surge_lost' ? 'bad' : e.kind === 'claimed' || e.kind === 'claimable' ? 'good' : 'faint'} />
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
    <Section title="队员" icon="roster" hint={`${view.operatives.length}/${view.roster_cap}`}>
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
          {view.districts.some((d) => d.customize.unlocked) ? (
            <div className="tiny faint" style={{ marginTop: 6 }}>
              定制招募已开放：在街区页用居民换一名指定职业、词条或稀有度的候选。
            </div>
          ) : null}
        </Section>
        <Section title="仓库" icon="equip" hint={`${view.inventory.length} 件`}>
          {view.inventory.length === 0 ? (
            <div className="empty">路线上偶尔掉装备；物资点也有。</div>
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
                  <button className="btn small ghost" title="封存的装备带进下一轮" onClick={() => run({ cmd: 'seal', item: it.id, on: true }, `已封存${it.name}`)}>
                    <Icon id="seal" size={14} /> 封存
                  </button>
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
    <Section title="街区" icon="trust" hint="占到格子的街区才有居民和信任">
      <div className="district-grid">
        {ds.map((d) => (
          <button key={d.id} className="card district-card" onClick={() => open({ kind: 'district', district: d.id })} style={d.owned ? undefined : { opacity: 0.6 }}>
            <div className="row">
              <span className="swatch" style={{ background: WASH[d.id % WASH.length] }} />
              <h3 className="grow ellipsis" style={{ fontSize: 17 }}>
                {d.name}
              </h3>
              {d.policy ? <span className="chip good">政策</span> : d.policy_unlocked ? <span className="chip warn">可定政策</span> : null}
            </div>
            <div className="row small" style={{ gap: 10 }}>
              <span>
                <span className="faint">格</span> <span className="num">{d.owned}/{d.total}</span>
              </span>
              <span>
                <span className="faint">人口</span> <span className="num">{d.population}</span>
              </span>
              <span>
                <span className="faint">民兵</span> <span className="num">{d.militia}</span>
              </span>
              {d.contiguous ? <span className="chip good">连片</span> : null}
              {d.unhoused ? <span className="chip bad">无处安置 {d.unhoused}</span> : null}
            </div>
            {d.owned ? (
              <div className="row small">
                <span className="faint">信任</span>
                <div className="grow">
                  <Bar value={d.trust} max={100} tone={d.trust >= 60 ? 'good' : d.trust < 35 ? 'bad' : undefined} />
                </div>
                <span className="num">{d.trust}</span>
                <span className={'tiny ' + (d.trust_target > d.trust ? 'good' : d.trust_target < d.trust ? 'bad' : 'faint')}>→{d.trust_target}</span>
              </div>
            ) : (
              <div className="tiny faint">还没有领地</div>
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
    <div className="split">
      <div className="col">
        <Section title={`组织 Lv.${org.level}`} icon="org" hint={org.next ? `${org.exp}/${org.next}` : `${org.exp}`}>
          {org.next ? <Bar value={org.exp} max={org.next} /> : null}
          <div className="stack" style={{ gap: 2, marginTop: 8 }}>
            {org.perks.map((p) => (
              <div key={p.level + p.text} className={'small ' + (p.unlocked ? '' : 'faint')}>
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

        {view.phase === 'active' ? <CycleBlock /> : null}
      </div>
      <div className="col">
        <Section title="特殊人物" icon="star" hint="满足条件后进入候选">
          <div className="stack" style={{ gap: 8 }}>
            {org.specials.map((s) => (
              <div key={s.id} className="card flat">
                <div className="row">
                  <Icon id={s.icon} size={20} />
                  <b>{s.name}</b>
                  <span className="faint tiny">{s.side}</span>
                  <span className="grow" />
                  {s.recruited ? <span className="chip good">已加入</span> : s.met ? <span className="chip warn">已出现</span> : null}
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

        <Section title="封存" icon="seal" hint={`${view.sealed.length}/${org.sealed_slots}`}>
          {view.sealed.length === 0 ? <div className="empty">封存的装备会带进下一轮。在「队员 → 仓库」里封存。</div> : null}
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
              onClick={() =>
                confirm({
                  title: '重新开始？',
                  body: '清空这个浏览器里的存档。先导出一份更稳妥。',
                  ok: '清空重来',
                  danger: true,
                  onOk: () => {
                    game.reset();
                    toast('新游戏');
                  },
                })
              }
            >
              重新开始
            </button>
          </div>
          <textarea className="save" value={saveText} onChange={(e) => setSaveText(e.target.value)} placeholder="导出的存档会出现在这里；也可以粘贴存档再读档。" />
        </Section>
      </div>
    </div>
  );
}

function CycleBlock() {
  const { view, run, confirm } = useCtx();
  return (
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
        <button
          className="btn danger small"
          onClick={() =>
            confirm({
              title: '收束这一轮？',
              body: '地图、街区、普通队员都会清空；组织经验、规章、特殊人物和封存的装备留下。',
              ok: '确定收束',
              danger: true,
              onOk: () => run({ cmd: 'collapse' }, '这一轮收束了'),
            })
          }
        >
          收束这一轮…
        </button>
      </div>
    </Section>
  );
}
