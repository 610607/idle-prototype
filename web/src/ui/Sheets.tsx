import { useMemo, useState } from 'react';
import { Icon } from '../icons';
import type { Cond, DistrictView, DryRun, Gambit, OpView, Target } from '../types';
import { Bar, DryRunView, fmtIn, OUTCOME_NAMES, Page, Rarity, RES_KEYS, RES_NAMES, Section, SQUAD_HEX, TagIO, useCtx } from './common';
import { OpRow, TideRows } from './Panels';

// ---------------------------------------------------------------------------
// Gambit wording

function condText(c: Cond, tagName: (t: string) => string): string {
  switch (c.t) {
    case 'always':
      return '总是';
    case 'self_hp_below':
      return `自己生命 < ${c.pct}%`;
    case 'self_hp_above':
      return `自己生命 > ${c.pct}%`;
    case 'ally_hp_below':
      return `有队友生命 < ${c.pct}%`;
    case 'enemy_has_tag':
      return `敌人带 [${tagName(c.tag)}]`;
    case 'self_has_tag':
      return `自己带 [${tagName(c.tag)}]`;
    case 'enemy_tag_at_least':
      return `敌人 [${tagName(c.tag)}] ≥ ${c.n} 层`;
    case 'any_tag_at_least':
      return `敌人任意标签 ≥ ${c.n} 层`;
    case 'enemy_count_at_least':
      return `敌人 ≥ ${c.n} 个`;
    case 'wave_at_least':
      return `第 ${c.n} 波起`;
    case 'round_at_least':
      return `第 ${c.n} 回合起`;
  }
}

const TARGET_NAMES: Record<Target, string> = {
  default: '默认目标',
  front_lowest: '前排最残',
  back_first: '优先后排',
  highest_atk: '攻击最高',
  with_tag: '带该标签的',
  lowest_ally: '最残的队友',
  self: '自己',
  most_tags: '标签最多的',
};

// ---------------------------------------------------------------------------

export function OpSheet({ op, onClose }: { op: number; onClose: () => void }) {
  const { view } = useCtx();
  const o = view.operatives.find((x) => x.id === op);
  if (!o)
    return (
      <Page title="队员" onClose={onClose}>
        <p className="muted">这名队员已经不在队里了。</p>
      </Page>
    );
  return (
    <Page
      title={
        <span className="row" style={{ gap: 8 }}>
          {o.name} <Rarity r={o.rarity} name={o.rarity_name} />
          <span className="muted small">{o.class_name}</span>
        </span>
      }
      icon={o.icon}
      onClose={onClose}
      aside={
        <>
          <AssignBlock o={o} />
          <EquipBlock o={o} />
          <GambitEditor o={o} />
          <OpDanger o={o} onClose={onClose} />
        </>
      }
    >
      <OpDetail o={o} />
    </Page>
  );
}

export function OpDetail({ o }: { o: OpView }) {
  const { game } = useCtx();
  const d = o.derived;
  return (
    <>
      <div className="small muted">
        {o.origin} · {o.status}
        {o.special ? ' · 特别人物' : ''}
      </div>
      <div className="stat-tiles">
        {(
          [
            ['hp', '生命', d.hp],
            ['atk', '攻击', d.atk],
            ['def', '防御', d.def],
            ['spd', '速度', d.spd],
            ['ap', '行动', d.ap],
            ['pp', '反应', d.pp],
          ] as const
        ).map(([id, k, v]) => (
          <div key={id} className="stat-tile">
            <span className="k">
              <Icon id={id} /> {k}
            </span>
            <span className="num">{v}</span>
          </div>
        ))}
      </div>
      <details>
        <summary className="small muted" style={{ cursor: 'pointer' }}>
          九项属性
        </summary>
        <div className="statgrid" style={{ marginTop: 4 }}>
          {game.content.stats.map(([id, name], k) => (
            <span key={id} className="small">
              {name} <span className="num">{o.stats[k]}</span>
            </span>
          ))}
        </div>
      </details>
      <Section title="特质" icon="trait" hint="虚线 = 读取的标签，实线 = 施加的标签">
        <div className="stack" style={{ gap: 6 }}>
          {o.traits.map((t) => (
            <div key={t.id} className={'trait' + (t.exp ? ' exp' : '')}>
              <div className="row" style={{ gap: 6 }}>
                <Icon id={t.icon} size={16} />
                <b>{t.name}</b>
                {t.exp ? <span className="chip warn">经历</span> : null}
                <span className="grow" />
                <TagIO reads={t.reads} writes={t.writes} />
              </div>
              <div className="small muted">{t.desc}</div>
            </div>
          ))}
          {o.growing.map((g) => (
            <div key={g.id} className="trait" style={{ borderStyle: 'dashed' }}>
              <div className="row small">
                <Icon id="talent" size={14} />
                <span>正在长出：{g.name}</span>
                <span className="grow" />
                <span className="num faint">
                  {g.have}/{g.need}
                </span>
              </div>
              <div className="tiny faint">{g.hint}</div>
              <Bar value={g.have} max={g.need} tone="warn" />
            </div>
          ))}
        </div>
      </Section>
      <Section title="技能" icon="skill">
        <div className="stack" style={{ gap: 6 }}>
          {o.skills.map((s) => (
            <div key={s.id} className="trait">
              <div className="row" style={{ gap: 6 }}>
                <Icon id={s.icon} size={16} />
                <b>{s.name}</b>
                <span className="chip">{s.passive ? '反应 PP' : '行动 AP'}</span>
                <span className="grow" />
                <TagIO reads={s.reads} writes={s.writes} />
              </div>
              <div className="small muted">{s.desc}</div>
            </div>
          ))}
        </div>
      </Section>
    </>
  );
}

function AssignBlock({ o }: { o: OpView }) {
  const { view, run, open } = useCtx();
  const a = o.assignment;
  return (
    <Section title="安排" icon="squad" hint={o.status}>
      <div className="row wrap">
        {view.squads.map((s, i) => {
          const inIt = s.members.includes(o.id);
          const full = !inIt && s.members.length >= 3;
          return (
            <button
              key={s.id}
              className={'btn small' + (inIt ? ' selected' : '')}
              disabled={full}
              style={{ borderColor: SQUAD_HEX[i] }}
              onClick={() =>
                run(
                  { cmd: 'set_squad', squad: s.id, members: inIt ? s.members.filter((m) => m !== o.id) : [...s.members, o.id] },
                  inIt ? `${o.name} 离开${s.name}` : `${o.name} 加入${s.name}`,
                )
              }
            >
              {inIt ? `离开${s.name}` : `编入${s.name}`}
              {full ? '（满）' : ''}
            </button>
          );
        })}
        {a.t === 'garrison' ? (
          <>
            <button className="btn small" onClick={() => open({ kind: 'tile', tile: a.tile })}>
              去 {view.tiles[a.tile]?.label}
            </button>
            <button className="btn small" onClick={() => run({ cmd: 'garrison', op: o.id, tile: null }, '已撤下驻守')}>
              撤下驻守
            </button>
          </>
        ) : (
          <span className="small faint">驻守：在地图上点边界格挑人。</span>
        )}
      </div>
    </Section>
  );
}

function EquipBlock({ o }: { o: OpView }) {
  const { view, run } = useCtx();
  return (
    <Section title="装备" icon="equip">
      {o.equipment ? (
        <div className="trait">
          <div className="row" style={{ gap: 6 }}>
            <Icon id={o.equipment.icon} size={16} />
            <b>{o.equipment.name}</b>
            <span className="grow" />
            <TagIO reads={o.equipment.reads} writes={o.equipment.writes} />
          </div>
          <div className="small muted">{o.equipment.desc}</div>
          <button className="btn small" style={{ marginTop: 4 }} onClick={() => run({ cmd: 'equip', op: o.id, item: null }, '已卸下')}>
            卸下
          </button>
        </div>
      ) : (
        <div className="empty">没有装备。</div>
      )}
      {view.inventory.length ? (
        <div className="row wrap" style={{ marginTop: 6 }}>
          {view.inventory.map((it, k) => (
            <button key={k} className="btn small" onClick={() => run({ cmd: 'equip', op: o.id, item: it.id }, `装上${it.name}`)} title={it.desc}>
              <Icon id={it.icon} size={14} /> {it.name}
            </button>
          ))}
        </div>
      ) : null}
    </Section>
  );
}

function GambitEditor({ o }: { o: OpView }) {
  const { game, run } = useCtx();
  const [list, setList] = useState<Gambit[]>(o.gambits);
  const dirty = JSON.stringify(list) !== JSON.stringify(o.gambits);
  const tagName = (t: string) => game.content.tags.find((x) => x.id === t)?.name ?? t;
  const skillName = (id: string) => game.content.skills.find((s) => s.id === id)?.name ?? id;
  const on = list.filter((g) => g.on).length;
  const move = (k: number, d: number) => {
    const j = k + d;
    if (j < 0 || j >= list.length) return;
    const next = [...list];
    [next[k], next[j]] = [next[j]!, next[k]!];
    setList(next);
  };
  return (
    <Section title="战术" icon="rules" hint={`启用 ${on}/${o.derived.tactic_slots}（智力越高槽越多）`}>
      <div className="small faint" style={{ marginBottom: 6 }}>
        从上往下，第一条条件成立的就执行。
      </div>
      <div className="stack" style={{ gap: 4 }}>
        {list.map((g, k) => (
          <div key={k} className={'gambit' + (g.on ? '' : ' off')}>
            <input
              type="checkbox"
              checked={g.on}
              onChange={(e) => setList(list.map((x, j) => (j === k ? { ...x, on: e.target.checked } : x)))}
              aria-label="启用"
            />
            <span>
              若 <b>{condText(g.if, tagName)}</b>
              {g.and ? (
                <>
                  {' '}且 <b>{condText(g.and, tagName)}</b>
                </>
              ) : null}
              ，
              {g.do.t === 'skill' ? (
                <>
                  用「{skillName(g.do.skill)}」→ {TARGET_NAMES[g.target]}
                </>
              ) : g.do.t === 'retreat_planned' ? (
                '按计划收队'
              ) : (
                '紧急撤退'
              )}
            </span>
            <span className="row" style={{ gap: 2 }}>
              <button className="btn small ghost" onClick={() => move(k, -1)} aria-label="上移">
                ↑
              </button>
              <button className="btn small ghost" onClick={() => move(k, 1)} aria-label="下移">
                ↓
              </button>
            </span>
          </div>
        ))}
      </div>
      {dirty ? (
        <div className="row" style={{ marginTop: 6 }}>
          <button className="btn small primary" onClick={() => run({ cmd: 'set_gambits', op: o.id, gambits: list }, '战术已保存')}>
            保存战术
          </button>
          <button className="btn small ghost" onClick={() => setList(o.gambits)}>
            还原
          </button>
        </div>
      ) : null}
    </Section>
  );
}

function OpDanger({ o, onClose }: { o: OpView; onClose: () => void }) {
  const { run } = useCtx();
  const [sure, setSure] = useState(false);
  return (
    <div className="row wrap">
      <button className="btn small" onClick={() => run({ cmd: 'lock', op: o.id, locked: !o.locked }, o.locked ? '已解锁' : '已锁定')}>
        <Icon id={o.locked ? 'unlock' : 'lock'} size={14} /> {o.locked ? '解锁' : '锁定'}
      </button>
      {!o.special && !o.locked ? (
        sure ? (
          <button className="btn small danger" onClick={() => run({ cmd: 'retire', op: o.id }, `${o.name} 退役`) && onClose()}>
            确定退役（回出身街区当教官）
          </button>
        ) : (
          <button className="btn small ghost" onClick={() => setSure(true)}>
            <Icon id="retire" size={14} /> 退役…
          </button>
        )
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------

export function CandidateSheet({ candidate, onClose }: { candidate: number; onClose: () => void }) {
  const { view, run, game } = useCtx();
  const c = view.candidates.find((x) => x.id === candidate);
  const dry = useMemo(() => (c ? game.dryRun({ cmd: 'hire', candidate }) : null), [c, game, candidate]);
  if (!c)
    return (
      <Page title="候选" onClose={onClose}>
        <p className="muted">这名候选已经不在了。</p>
      </Page>
    );
  return (
    <Page
      title={
        <span className="row" style={{ gap: 8 }}>
          {c.op.name} <Rarity r={c.op.rarity} name={c.op.rarity_name} />
          <span className="muted small">{c.op.class_name}</span>
        </span>
      }
      icon={c.op.icon}
      onClose={onClose}
      aside={
        <div className="card striped">
          <Section title="招募" icon="hire" hint={c.source}>
            <DryRunView dry={dry} />
            <div className="row" style={{ marginTop: 10 }}>
            <button className="btn primary" disabled={!dry?.ok} onClick={() => run({ cmd: 'hire', candidate }, `${c.op.name} 加入`) && onClose()}>
              招募{c.cost ? `（${c.cost} 废材）` : '（免费）'}
            </button>
            <button className="btn ghost" onClick={() => run({ cmd: 'dismiss', candidate }, '已婉拒') && onClose()}>
              婉拒
            </button>
            </div>
          </Section>
        </div>
      }
    >
      <OpDetail o={c.op} />
    </Page>
  );
}

// ---------------------------------------------------------------------------

export function DistrictSheet({ district, onClose }: { district: number; onClose: () => void }) {
  const { view, open } = useCtx();
  const d = view.districts.find((x) => x.id === district)!;
  return (
    <Page
      title={d.name}
      icon="trust"
      onClose={onClose}
      aside={
        <>
          <DistrictPriority d={d} />
          <DistrictPolicy d={d} />
          <DistrictCustomize d={d} />
          <Section title="格子" icon="map" hint={`${d.owned}/${d.total}`}>
            <div className="row wrap" style={{ gap: 4 }}>
              {d.tiles.map((i) => (
                <button key={i} className={'btn small' + (view.tiles[i]!.owned ? '' : ' ghost')} onClick={() => open({ kind: 'tile', tile: i })}>
                  {view.tiles[i]!.label}
                </button>
              ))}
            </div>
          </Section>
        </>
      }
    >
      <div className="small muted">
        {d.owned}/{d.total} 格{d.contiguous ? ' · 连片（产出 +25%，信任 +10）' : ''} · 人口 {d.population} · 住房 {d.housing} · 在岗 {d.employed}
        {d.unhoused ? <b className="bad"> · 无处安置 {d.unhoused}</b> : null}
      </div>
      <div className="trait">
        <div className="row small">
          <Icon id={d.flavor.icon} size={14} /> 街区风气：<b>{d.flavor.name}</b>
          <span className="faint">（这里招到的人会带上）</span>
        </div>
        <div className="tiny muted">{d.flavor.desc}</div>
      </div>

      <Section title={`信任 ${d.trust}`} icon="trust" hint={`每小时向 ${d.trust_target} 靠拢`}>
        <Bar value={d.trust} max={100} tone={d.trust >= 60 ? 'good' : d.trust < 35 ? 'bad' : undefined} />
        <div className="stack" style={{ gap: 2, marginTop: 6 }}>
          {d.trust_factors.map(([k, v]) => (
            <div key={k} className="row small">
              <span className="grow">{k}</span>
              <span className={'num ' + (v > 0 ? 'good' : v < 0 ? 'bad' : '')}>{v > 0 ? `+${v}` : v}</span>
            </div>
          ))}
        </div>
        <div className="small muted" style={{ marginTop: 4 }}>
          守城民兵 {d.militia}（其中志愿 {d.volunteers}：住得下又信任你的居民会出来帮忙）
        </div>
      </Section>

      <Section title="人才" icon="talent" hint={d.training_bp ? `训练加成 +${d.training_bp / 100}%` : undefined}>
        <div className="stack" style={{ gap: 6 }}>
          {d.talent.map((t) => (
            <div key={t.prof}>
              <div className="row small">
                <Icon id={t.icon} size={14} />
                <span>{t.name}</span>
                <span className="faint tiny">
                  在岗 {t.workers}/{t.jobs}
                  {t.instructors ? ` · 教官 ${t.instructors}` : ''}
                </span>
                <span className="grow" />
                <span className="num tiny">
                  {t.pct}%{t.eta_hours !== null ? ` · 约 ${t.eta_hours} 时` : ''}
                </span>
              </div>
              <Bar value={t.pct} max={100} tone="warn" />
            </div>
          ))}
        </div>
      </Section>

    </Page>
  );
}

function DistrictPriority({ d }: { d: DistrictView }) {
  const { run } = useCtx();
  const [prio, setPrio] = useState<string[]>(d.priority);
  const profName = (id: string) => d.talent.find((t) => t.prof === id)?.name ?? id;
  const prioDirty = JSON.stringify(prio) !== JSON.stringify(d.priority);
  return (
    <Section title="岗位优先级" icon="priority" hint="人手不够时先填上面的">
      <div className="stack" style={{ gap: 3 }}>
        {prio.map((p, k) => (
          <div key={p} className="row small">
            <span className="num faint">{k + 1}</span>
            <span className="grow">{profName(p)}</span>
            <button
              className="btn small ghost"
              disabled={k === 0}
              onClick={() => {
                const n = [...prio];
                [n[k - 1], n[k]] = [n[k]!, n[k - 1]!];
                setPrio(n);
              }}
            >
              ↑
            </button>
          </div>
        ))}
      </div>
      {prioDirty ? (
        <button className="btn small primary" style={{ marginTop: 6 }} onClick={() => run({ cmd: 'set_priority', district: d.id, priority: prio }, '优先级已改')}>
          保存
        </button>
      ) : null}
    </Section>
  );
}

function DistrictPolicy({ d }: { d: DistrictView }) {
  const { game, run } = useCtx();
  const [dry, setDry] = useState<{ id: string | null; r: DryRun } | null>(null);
  return (
    <Section title="政策" icon="policy" hint={d.policy_unlocked ? (d.policy ? '已定' : '可以定一条') : '信任 60 解锁'}>
      <div className="stack" style={{ gap: 6 }}>
        {d.policy_options.map((p) => (
          <button
            key={p.id}
            disabled={!p.available && d.policy !== p.id}
            onClick={() => setDry({ id: d.policy === p.id ? null : p.id, r: game.dryRun({ cmd: 'set_policy', district: d.id, policy: d.policy === p.id ? null : p.id }) })}
            className={'list-item' + (d.policy === p.id ? ' selected' : '')}
          >
            <div className="grow">
              <b>{p.name}</b> {d.policy === p.id ? <span className="chip good">生效中</span> : null}
              <div className="small muted">{p.desc}</div>
              {!p.available && p.reason ? <div className="tiny faint">{p.reason}</div> : null}
            </div>
          </button>
        ))}
      </div>
      {dry ? (
        <div className="card flat" style={{ marginTop: 6 }}>
          <DryRunView dry={dry.r} />
          <button className="btn small primary" style={{ marginTop: 6 }} disabled={!dry.r.ok} onClick={() => run({ cmd: 'set_policy', district: d.id, policy: dry.id }, '政策已改') && setDry(null)}>
            {dry.id ? '定下这条政策' : '取消政策'}
          </button>
        </div>
      ) : null}
    </Section>
  );
}

const RARITY_FLOORS: [number | null, string][] = [
  [null, '不保底'],
  [1, 'R'],
  [2, 'SR'],
  [3, 'SSR'],
];

/** 定制招募: spend this district's residents on a candidate of your choosing. */
function DistrictCustomize({ d }: { d: DistrictView }) {
  const { game, run, view } = useCtx();
  const cz = d.customize;
  const [prof, setProf] = useState<string | null>(null);
  const [cls, setCls] = useState<string | null>(null);
  const [lock, setLock] = useState<string | null>(null);
  const [rarity, setRarity] = useState<number | null>(null);
  const opt = cz.options.find((o) => o.prof === prof) ?? cz.options.find((o) => o.ok) ?? null;
  const klass = opt?.classes.find((c) => c.id === cls && c.ok) ?? opt?.classes[0] ?? null;
  const cost = cz.base + (klass?.mixed ? cz.class_cost : 0) + (lock ? cz.lock_cost : 0) + (rarity !== null ? cz.rarity_cost[rarity]! : 0);
  const cmd =
    opt && klass
      ? ({ cmd: 'customize', district: d.id, profession: opt.prof, class: klass.mixed ? klass.id : null, lock, rarity } as const)
      : null;
  const key = JSON.stringify(cmd);
  const dry = useMemo(
    () => (cmd && cz.unlocked && opt?.ok ? game.dryRun(cmd) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key, cz.unlocked, opt?.ok, game, view.revision, view.time],
  );
  const hint = cz.unlocked ? `用居民换一名指定的候选 · 现有 ${d.population} 人` : '未开放';
  return (
    <Section title="定制招募" icon="candidate" hint={hint}>
      {!cz.unlocked ? (
        <div className="small muted">{cz.hint}。</div>
      ) : cz.options.length === 0 ? (
        <div className="small muted">这个街区还没有人在岗，带不出新人。</div>
      ) : (
        <div className="stack" style={{ gap: 8 }}>
          <div className="stack" style={{ gap: 4 }}>
            {cz.options.map((o) => (
              <button
                key={o.prof}
                disabled={!o.ok}
                className={'list-item' + (opt?.prof === o.prof ? ' selected' : '')}
                onClick={() => {
                  setProf(o.prof);
                  setCls(null);
                  setLock(null);
                }}
              >
                <Icon id={o.prof_icon} size={18} />
                <div className="grow">
                  <b>{o.prof_name}</b> <span className="small muted">在岗 {o.workers}</span>
                  <div className="tiny faint">{o.reason ?? `带出 ${o.classes.map((c) => c.name).join(' / ')}`}</div>
                </div>
              </button>
            ))}
          </div>
          {opt && opt.ok && opt.classes.length > 1 ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="seg">
                {opt.classes.map((c) => (
                  <button
                    key={c.id}
                    disabled={!c.ok}
                    aria-pressed={klass?.id === c.id}
                    onClick={() => {
                      setCls(c.id);
                      setLock(null);
                    }}
                  >
                    {c.name}
                    {c.mixed ? ` +${cz.class_cost}` : ''}
                  </button>
                ))}
              </div>
              {opt.classes
                .filter((c) => !c.ok && c.reason)
                .map((c) => (
                  <div key={c.id} className="tiny faint">
                    {c.name}：{c.reason}
                  </div>
                ))}
            </div>
          ) : null}
          {opt && opt.ok && klass && klass.traits.length ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="small">
                指定一条词条 <span className="faint">+{cz.lock_cost} 人</span>
              </div>
              <button className={'list-item' + (lock === null ? ' selected' : '')} onClick={() => setLock(null)}>
                <div className="grow small">不指定（随机）</div>
              </button>
              {klass.traits.map((t) => (
                <button key={t.id} className={'list-item' + (lock === t.id ? ' selected' : '')} onClick={() => setLock(t.id)}>
                  <Icon id={t.icon} size={16} />
                  <div className="grow">
                    <b className="small">{t.name}</b>
                    <div className="tiny muted">{t.desc}</div>
                  </div>
                </button>
              ))}
            </div>
          ) : null}
          {opt && opt.ok ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="small">稀有度保底</div>
              <div className="seg">
                {RARITY_FLOORS.map(([r, name]) => (
                  <button key={name} aria-pressed={rarity === r} onClick={() => setRarity(r)}>
                    {name}
                    {r !== null && cz.rarity_cost[r] ? ` +${cz.rarity_cost[r]}` : ''}
                  </button>
                ))}
              </div>
            </div>
          ) : null}
          {opt && opt.ok ? (
            <div className="card flat">
              <div className="small">
                共消耗 <b>{cost}</b> 名居民（现有 {d.population}）。空出的岗位由剩下的人按优先级补，人口按住房和信任慢慢长回来。
              </div>
              <div style={{ marginTop: 6 }}>
                <DryRunView dry={dry} />
              </div>
              <button
                className="btn small primary"
                style={{ marginTop: 6 }}
                disabled={!dry?.ok}
                onClick={() => cmd && run(cmd, '定制候选已进招募池')}
              >
                下单定制
              </button>
            </div>
          ) : null}
        </div>
      )}
    </Section>
  );
}

// ---------------------------------------------------------------------------

export function SquadSheet({ squad, onClose }: { squad: number; onClose: () => void }) {
  const { view, open, run, setRouteEdit } = useCtx();
  const s = view.squads.find((x) => x.id === squad)!;
  const free = view.operatives.filter((o) => !s.members.includes(o.id));
  const trip = s.last ?? s.running;
  return (
    <Page
      title={s.name}
      icon="squad"
      onClose={onClose}
      aside={trip ? <TripBlock squad={squad} /> : undefined}
    >
      <Section title="队员" icon="roster" hint={`${s.members.length}/3`}>
        <div className="list">
          {s.members.map((id) => {
            const o = view.operatives.find((x) => x.id === id);
            return o ? (
              <OpRow
                key={id}
                o={o}
                onClick={() => open({ kind: 'op', op: id })}
                right={
                  <span
                    className="btn small ghost"
                    onClick={(e) => {
                      e.stopPropagation();
                      run({ cmd: 'set_squad', squad, members: s.members.filter((m) => m !== id) });
                    }}
                  >
                    移出
                  </span>
                }
              />
            ) : null;
          })}
        </div>
        {s.members.length < 3 && free.length ? (
          <div className="row wrap" style={{ marginTop: 6 }}>
            {free.map((o) => (
              <button key={o.id} className="btn small" onClick={() => run({ cmd: 'set_squad', squad, members: [...s.members, o.id] }, `${o.name} 加入${s.name}`)}>
                <Icon id={o.icon} size={14} /> {o.name}
                <span className="faint tiny">{o.status}</span>
              </button>
            ))}
          </div>
        ) : null}
      </Section>
      <Section title="路线" icon="route" hint={`走过 ${s.trips} 趟`}>
        <div className="small">{s.route.length ? s.route.map((i) => view.tiles[i]!.label).join(' → ') : '没有路线'}</div>
        <button className="btn small" style={{ marginTop: 6 }} onClick={() => setRouteEdit({ squad, route: s.route })} disabled={!s.members.length}>
          在地图上改路线
        </button>
      </Section>
    </Page>
  );
}

function TripBlock({ squad }: { squad: number }) {
  const { view, now, open, game } = useCtx();
  const s = view.squads.find((x) => x.id === squad)!;
  const trip = (s.last ?? s.running)!;
  return (
    <Section title={s.running ? '这一趟' : '上一趟'} icon="report" hint={s.running ? `${fmtIn(s.running.end - now)}后回来` : OUTCOME_NAMES[trip.outcome]}>
      <div className="list">
        {trip.results.map((r) => (
          <div key={r.tile} className="list-item small">
            <span className={r.outcome === 'victory' ? 'good' : 'bad'}>{r.label}</span>
            <span className="grow">{OUTCOME_NAMES[r.outcome]}</span>
            <button
              className="btn small ghost"
              onClick={() => open({ kind: 'report', title: `${s.name} · ${r.label}`, load: () => game.tripBattle(squad, r.tile) ?? (r.report !== null ? game.report(r.report) : null) })}
            >
              回放
            </button>
          </div>
        ))}
        {trip.results.length === 0 ? <div className="empty">这趟只在领地里巡逻。</div> : null}
      </div>
      <div className="row wrap small" style={{ gap: 10, marginTop: 6 }}>
        <span className="muted">带回：</span>
        {RES_KEYS.filter((k) => trip.loot[k] > 0).map((k) => (
          <span key={k} className="res-item">
            <Icon id={`res-${k}`} size={14} />
            {RES_NAMES[k]} <span className="num">{trip.loot[k]}</span>
          </span>
        ))}
        {trip.drops.map((d, k) => (
          <span key={k} className="chip">
            {d}
          </span>
        ))}
      </div>
    </Section>
  );
}

export function TideSheet({ onClose }: { onClose: () => void }) {
  const { view } = useCtx();
  return (
    <Page title="潮汐表 · 未来 12 小时" icon="tide" onClose={onClose}>
      <p className="small muted" style={{ margin: 0 }}>
        每个边界格每 4 小时来潮一次，时间固定、公开。表里的结果是引擎按现在的布置把每一场都提前打了一遍。
      </p>
      <TideRows rows={view.tide} />
      {view.modifier ? (
        <div className="small muted">
          本轮：{view.modifier[1]}。{view.modifier[2]}
        </div>
      ) : null}
    </Page>
  );
}
