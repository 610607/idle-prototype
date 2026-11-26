import { useMemo, useState } from 'react';
import { Icon } from '../icons';
import type { Command, Dial, DryRun, GarrisonOption, TileView } from '../types';
import { Cost, DIAL_NAMES, DryRunView, fmtIn, Section, SheetFrame, TagIO, useCtx } from './common';

const TERRAIN_NAMES: Record<string, string> = {
  unknown: '迷雾',
  hq: '总部',
  city: '城区',
  rift: '裂隙',
  special: '特殊地点',
  barrier: '天险',
};
const SPECIAL_NAMES: Record<string, string> = { landmark: '地标', cache: '物资点', clue: '线索', core: '异界核心' };

export function TileSheet({ tile, onClose }: { tile: number; onClose: () => void }) {
  const { view } = useCtx();
  const t = view.tiles[tile]!;
  const district = t.district !== undefined ? view.districts.find((d) => d.id === t.district) : undefined;
  const title = (
    <span>
      {t.label} · {t.special ? SPECIAL_NAMES[t.special] : TERRAIN_NAMES[t.terrain]}
      {district ? <span className="muted small"> · {district.name}</span> : null}
    </span>
  );
  const icon = t.terrain === 'hq' ? 'tile-hq' : t.terrain === 'rift' ? 'tile-rift' : t.terrain === 'barrier' ? 'tile-barrier' : t.revealed ? 'tile-city' : 'fog';
  return (
    <SheetFrame title={title} icon={icon} onClose={onClose}>
      <Status t={t} />
      {!t.revealed && t.terrain !== 'barrier' ? <p className="muted">还在迷雾里。小队走到旁边、或者瞭望点，会把它揭开。</p> : null}
      {t.terrain === 'barrier' ? <p className="muted">天险：路线过不去。老邮差这类人物可以带队穿过。</p> : null}
      {t.claimable ? <ClaimBlock t={t} /> : null}
      {t.alert ? <AlertBlock t={t} /> : null}
      {t.border ? <DefenseBlock t={t} /> : null}
      {t.owned && t.terrain === 'city' ? <BuildBlock t={t} /> : null}
      {t.owned && t.terrain === 'rift' && t.rift ? <DialBlock t={t} /> : null}
      {!t.owned && t.revealed && t.enemies.length ? <EnemyBlock t={t} /> : null}
      {district ? <DistrictLink id={district.id} /> : null}
      {t.owned && t.terrain !== 'hq' ? <AbandonBlock t={t} /> : null}
    </SheetFrame>
  );
}

function Status({ t }: { t: TileView }) {
  const { view, now } = useCtx();
  const items: [string, React.ReactNode][] = [];
  items.push(['圈层', `第 ${t.ring} 圈`]);
  if (t.revealed && t.terrain !== 'barrier') {
    items.push(['镇守度', <span className={t.alert ? 'bad' : ''}>{t.warden} / 100{t.owned ? '' : '（满了可占领）'}</span>]);
  }
  if (t.revealed && t.natives) items.push(['原住民', `${t.natives} 人`]);
  if (t.owned && t.workers) items.push(['在岗', `${t.workers[0]} / ${t.workers[1]}`]);
  if (t.frozen) items.push(['冻结的用途', t.frozen]);
  if (t.rift) items.push(['裂隙', `${t.rift.name} · 居民 ${t.rift.resident} ${t.rift.residents} 人`]);
  if (t.next_surge !== undefined) items.push(['下次来潮', `${fmtIn(t.next_surge - now)}后（来自 ${view.tiles[t.surge_source ?? 0]?.label}）`]);
  if (t.squads.length) items.push(['途经路线', t.squads.map((id) => view.squads.find((s) => s.id === id)?.name).join('、')]);
  return (
    <dl className="kv">
      {items.map(([k, v]) => (
        <FragmentKV key={k} k={k} v={v} />
      ))}
    </dl>
  );
}

function FragmentKV({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <>
      <dt>{k}</dt>
      <dd>{v}</dd>
    </>
  );
}

/** Confirm block with a dry-run preview: the engine says what changes before the player commits. */
function Confirm({ cmd, label, danger, okText, onDone }: { cmd: Command; label: string; danger?: boolean; okText?: string; onDone?: () => void }) {
  const { game, run } = useCtx();
  const [dry, setDry] = useState<DryRun | null>(null);
  return (
    <div className="stack">
      {dry ? <DryRunView dry={dry} /> : null}
      <div className="row wrap">
        {!dry ? (
          <button className="btn small" onClick={() => setDry(game.dryRun(cmd))}>
            <Icon id="tide" size={14} /> 预演
          </button>
        ) : null}
        <button
          className={'btn ' + (danger ? 'danger' : 'primary')}
          disabled={dry !== null && !dry.ok}
          onClick={() => {
            if (run(cmd, okText)) onDone?.();
          }}
        >
          {label}
        </button>
      </div>
    </div>
  );
}

function ClaimBlock({ t }: { t: TileView }) {
  const { game } = useCtx();
  const dry = useMemo(() => game.dryRun({ cmd: 'claim', tile: t.i }), [game, t.i]);
  return (
    <div className="card" style={{ borderColor: 'var(--good)' }}>
      <Section title={t.terrain === 'rift' ? '镇压这道裂隙' : '占领这一格'} icon="check" hint="改变地图形状，只由你决定">
        <DryRunView dry={dry} />
        <div className="row" style={{ marginTop: 8 }}>
          <ClaimButton tile={t.i} ok={dry.ok} />
        </div>
      </Section>
    </div>
  );
}

function ClaimButton({ tile, ok }: { tile: number; ok: boolean }) {
  const { run, open } = useCtx();
  return (
    <button className="btn primary" disabled={!ok} onClick={() => run({ cmd: 'claim', tile }, '已占领') && open(null)}>
      占领
    </button>
  );
}

function AlertBlock({ t }: { t: TileView }) {
  const { game, open } = useCtx();
  const report = useMemo(() => game.previewSurge(t.i, undefined), [game, t.i]);
  return (
    <div className="card" style={{ borderColor: 'var(--bad)' }}>
      <Section title="告急" icon="alert" hint="停产，但不会自己丢">
        <p className="small muted" style={{ margin: 0 }}>
          反击：用当前驻守打一场。赢了镇守度回到 50（街区有诊所回到 75）。也可以放弃这一格。
        </p>
        {report ? (
          <div className="row small" style={{ margin: '8px 0' }}>
            <span className={report.outcome === 'victory' ? 'good' : 'bad'}>
              预演：{report.outcome === 'victory' ? '能打赢' : `第 ${report.lost_wave ?? 1} 波会输`}
            </span>
            <button className="btn small ghost" onClick={() => open({ kind: 'report', title: `${t.label} 反击预演`, load: () => game.previewSurge(t.i, undefined) })}>
              看过程
            </button>
          </div>
        ) : null}
        <Confirm cmd={{ cmd: 'counter', tile: t.i }} label="反击" okText="反击成功" />
      </Section>
    </div>
  );
}

function DefenseBlock({ t }: { t: TileView }) {
  const { game, view, run, open } = useCtx();
  const [opts, setOpts] = useState<GarrisonOption[] | null>(null);
  const current = t.garrison !== undefined ? view.operatives.find((o) => o.id === t.garrison) : undefined;
  const tide = view.tide.filter((e) => e.tile === t.i);
  const load = () => setOpts(game.garrisonOptions(t.i));
  return (
    <Section title="守线" icon="garrison" hint={current ? `驻守：${current.name}` : '没人驻守'}>
      {tide.length ? (
        <div className="row wrap small" style={{ gap: 6, marginBottom: 8 }}>
          <span className="muted">12 小时内来潮：</span>
          {tide.map((e, k) => (
            <span key={k} className={'chip ' + (e.held ? 'good' : 'bad')}>
              <Icon id="wave" size={12} />
              {e.held ? '守住' : `第 ${e.lost_wave} 波失守`}
            </span>
          ))}
        </div>
      ) : null}
      {!opts ? (
        <button className="btn" onClick={load}>
          <Icon id="garrison" size={16} /> 挑选驻守（逐人预演下一次来潮）
        </button>
      ) : (
        <div className="list">
          {opts.map((o) => (
            <div className="list-item" key={String(o.op)}>
              <Icon id={o.icon} size={20} />
              <div className="grow">
                <div className="row" style={{ gap: 6 }}>
                  <b>{o.name}</b>
                  {o.current ? <span className="chip">当前</span> : null}
                  {o.busy ? <span className="faint tiny">{o.busy}</span> : null}
                </div>
                <div className={'small ' + (o.held ? 'good' : 'bad')}>
                  {o.held ? `守住 · 剩 ${o.hp_left_pct}% 生命 · ${o.rounds} 回合` : `第 ${o.lost_wave}/${o.total_waves} 波失守`}
                  {o.warden_gain ? <span className="muted"> · 接地 +{o.warden_gain}</span> : null}
                </div>
              </div>
              <button
                className="btn small ghost"
                onClick={() =>
                  open({ kind: 'report', title: `${t.label} 来潮预演 · ${o.name}`, load: () => game.previewSurge(t.i, o.op) })
                }
              >
                过程
              </button>
              {!o.current ? (
                <button
                  className="btn small"
                  onClick={() => {
                    const cmd: Command = o.op === null ? { cmd: 'garrison', op: t.garrison!, tile: null } : { cmd: 'garrison', op: o.op, tile: t.i };
                    if (run(cmd, o.op === null ? '已撤下驻守' : `${o.name} 驻守 ${t.label}`)) load();
                  }}
                >
                  {o.op === null ? '撤下' : '派驻'}
                </button>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </Section>
  );
}

function BuildBlock({ t }: { t: TileView }) {
  const { view, game, run } = useCtx();
  const [pick, setPick] = useState<string | null>(null);
  const [dry, setDry] = useState<DryRun | null>(null);
  const current = t.usage;
  const options = view.facilities.filter((f) => f.visible);
  const hiddenCount = view.facilities.length - options.length;
  const choose = (id: string | null) => {
    setPick(id ?? 'housing');
    setDry(game.dryRun({ cmd: 'build', tile: t.i, facility: id }));
  };
  const usageName = current === 'housing' ? '民居' : current === 'none' ? '空地' : view.facilities.find((f) => f.id === current)?.name ?? current;
  return (
    <Section title="用途" icon="housing" hint={`现在：${usageName}`}>
      <div className="list">
        {current !== 'housing' ? (
          <button className={'list-item' + (pick === 'housing' ? ' selected' : '')} onClick={() => choose(null)}>
            <Icon id="housing" size={20} />
            <div className="grow">
              <b>改回民居</b>
              <div className="small muted">安置 40 名居民。</div>
            </div>
          </button>
        ) : null}
        {options
          .filter((f) => f.id !== current)
          .map((f) => (
            <button key={f.id} className="list-item" onClick={() => choose(f.id)} style={pick === f.id ? { background: 'var(--paper-2)' } : undefined}>
              <Icon id={f.icon} size={20} />
              <div className="grow">
                <div className="row" style={{ gap: 6 }}>
                  <b>{f.name}</b>
                  {f.jobs ? <span className="faint tiny">岗位 {f.jobs}</span> : null}
                  {f.power ? <span className="faint tiny">耗电 {f.power}</span> : null}
                </div>
                <div className="small muted">{f.desc}</div>
              </div>
              <Cost cost={f.cost} have={view.resources} />
            </button>
          ))}
      </div>
      {hiddenCount > 0 ? <div className="faint tiny">还有 {hiddenCount} 种设施，材料攒到一半就会出现。</div> : null}
      {pick && dry ? (
        <div className="card flat" style={{ marginTop: 8 }}>
          <DryRunView dry={dry} />
          <div className="row" style={{ marginTop: 8 }}>
            <button
              className="btn primary"
              disabled={!dry.ok}
              onClick={() => {
                if (run({ cmd: 'build', tile: t.i, facility: pick === 'housing' ? null : pick }, '已改建')) {
                  setPick(null);
                  setDry(null);
                }
              }}
            >
              确定改建
            </button>
            <button className="btn ghost" onClick={() => setPick(null)}>
              取消
            </button>
          </div>
        </div>
      ) : null}
    </Section>
  );
}

function DialBlock({ t }: { t: TileView }) {
  const { game, run } = useCtx();
  const r = t.rift!;
  const dials: Dial[] = ['mine', 'balance', 'seal'];
  const [dry, setDry] = useState<{ d: Dial; r: DryRun } | null>(null);
  return (
    <Section title="裂隙旋钮" icon={`dial-${r.dial}`} hint={`人才 ${r.talent_pct}%`}>
      <p className="small muted" style={{ margin: '0 0 8px' }}>
        开采：晶体多、来潮强、居民流失。平衡：居民慢慢出人才。封印：来潮弱、几乎不产出。
      </p>
      <div className="seg">
        {dials.map((d) => (
          <button key={d} aria-pressed={r.dial === d} onClick={() => (r.dial === d ? null : setDry({ d, r: game.dryRun({ cmd: 'set_dial', tile: t.i, dial: d }) }))}>
            {DIAL_NAMES[d]}
          </button>
        ))}
      </div>
      {dry ? (
        <div className="card flat" style={{ marginTop: 8 }}>
          <DryRunView dry={dry.r} />
          <div className="row" style={{ marginTop: 8 }}>
            <button className="btn primary" onClick={() => run({ cmd: 'set_dial', tile: t.i, dial: dry.d }, `旋钮转到${DIAL_NAMES[dry.d]}`) && setDry(null)}>
              转到{DIAL_NAMES[dry.d]}
            </button>
          </div>
        </div>
      ) : null}
    </Section>
  );
}

function EnemyBlock({ t }: { t: TileView }) {
  const { view, setRouteEdit, open, toast, game } = useCtx();
  return (
    <Section title="盘踞的敌人" icon="wave" hint={`${t.enemies.length} 波`}>
      <div className="stack" style={{ gap: 4 }}>
        {t.enemies.map((w, k) => (
          <div key={k} className="row wrap small" style={{ gap: 10 }}>
            <span className="faint num">第{k + 1}波</span>
            {w.map((e, j) => (
              <span key={j} className="row" style={{ gap: 4 }}>
                <Icon id={e.icon} size={16} />
                {e.name}
                <TagIO reads={[]} writes={e.writes} />
              </span>
            ))}
          </div>
        ))}
      </div>
      <p className="small muted" style={{ margin: '8px 0' }}>
        小队路线每赢一次这一格，镇守度 +10；满 100 就可以占领。
      </p>
      <div className="row wrap">
        {view.squads
          .filter((s) => s.members.length)
          .map((s) => (
            <button
              key={s.id}
              className="btn small"
              onClick={() => {
                const next = s.route.includes(t.i) ? s.route : [...s.route, t.i];
                const p = game.previewRoute(s.id, next);
                if (!p.ok) {
                  toast(p.error ?? '路线不通', true);
                  setRouteEdit({ squad: s.id, route: s.route });
                } else {
                  setRouteEdit({ squad: s.id, route: next });
                }
                open(null);
              }}
            >
              <Icon id="route" size={14} /> 加进{s.name}路线
            </button>
          ))}
      </div>
    </Section>
  );
}

function DistrictLink({ id }: { id: number }) {
  const { view, open } = useCtx();
  const d = view.districts.find((x) => x.id === id)!;
  return (
    <button className="list-item card flat" onClick={() => open({ kind: 'district', district: id })}>
      <Icon id="trust" size={20} />
      <div className="grow">
        <b>{d.name}</b>
        <div className="small muted">
          {d.owned}/{d.total} 格 · 信任 {d.trust}（趋向 {d.trust_target}）· 人口 {d.population}
          {d.contiguous ? ' · 连片' : ''}
        </div>
      </div>
      <Icon id="chevron-right" size={16} />
    </button>
  );
}

function AbandonBlock({ t }: { t: TileView }) {
  const [show, setShow] = useState(false);
  if (!show)
    return (
      <div>
        <button className="btn small ghost faint" onClick={() => setShow(true)}>
          放弃这一格…
        </button>
      </div>
    );
  return (
    <div className="card flat" style={{ borderColor: 'var(--bad)' }}>
      <Section title="放弃这一格" icon="warn">
        <Confirm cmd={{ cmd: 'abandon', tile: t.i }} label="确定放弃" danger okText="已放弃" />
      </Section>
    </div>
  );
}
