import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Game } from '../engine/client';
import { Icon } from '../icons';
import type { Command, View } from '../types';
import { ReportSheet } from './BattleReport';
import { Board } from './Board';
import { fmtClock, fmtRate, GameCtx, OUTCOME_NAMES, RES_KEYS, RES_NAMES, SQUAD_HEX, useCtx, type Ctx, type RouteEdit, type Sheet } from './common';
import { HQPanel, MapPanel, RosterPanel } from './Panels';
import { CandidateSheet, DistrictSheet, OpSheet, SquadSheet, TideSheet } from './Sheets';
import { TileSheet } from './TileSheet';

type Tab = 'map' | 'roster' | 'hq';

export function App({ game }: { game: Game }) {
  const [view, setView] = useState<View>(() => game.view());
  const [now, setNow] = useState(() => game.now());
  const [tab, setTab] = useState<Tab>('map');
  const [sheets, setSheets] = useState<Sheet[]>([]);
  const [toastMsg, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const [routeEdit, setRouteEdit] = useState<RouteEdit | null>(null);
  const [focusTile, setFocusTile] = useState<number | null>(null);
  const lastRefresh = useRef(0);

  const refresh = useCallback(() => {
    setView(game.view());
    lastRefresh.current = Date.now();
  }, [game]);

  // Clock: advance the engine every second; rebuild the view when something happened or every 5s.
  useEffect(() => {
    const id = window.setInterval(() => {
      const n = game.tick();
      setNow(game.now());
      if (n > 0 || Date.now() - lastRefresh.current > 5000) refresh();
    }, 1000);
    const save = window.setInterval(() => game.persist(), 10_000);
    const onHide = () => game.persist();
    const onShow = () => {
      if (document.visibilityState === 'visible') {
        const n = game.tick();
        refresh();
        if (n > 0) toast(`离开期间发生了 ${n} 件事，看看「待你决定」`);
      } else onHide();
    };
    document.addEventListener('visibilitychange', onShow);
    window.addEventListener('pagehide', onHide);
    return () => {
      window.clearInterval(id);
      window.clearInterval(save);
      document.removeEventListener('visibilitychange', onShow);
      window.removeEventListener('pagehide', onHide);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game, refresh]);

  useEffect(() => {
    if (game.loadError) toast(`存档读不出来，已开新档：${game.loadError}`, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (focusTile === null) return;
    const id = window.setTimeout(() => setFocusTile(null), 4000);
    return () => window.clearTimeout(id);
  }, [focusTile]);

  const toast = useCallback((text: string, error = false) => {
    setToast({ text, error });
    window.setTimeout(() => setToast((t) => (t && t.text === text ? null : t)), error ? 4200 : 2600);
  }, []);

  const run = useCallback(
    (cmd: Command, okText?: string) => {
      const ack = game.command(cmd);
      if (!ack.ok) toast(ack.error ?? '做不到', true);
      else if (okText) toast(okText);
      refresh();
      return ack.ok;
    },
    [game, refresh, toast],
  );

  const open = useCallback((s: Sheet | null) => setSheets((st) => (s ? [...st, s] : [])), []);
  const close = useCallback(() => {
    setSheets((st) => st.slice(0, -1));
    refresh();
  }, [refresh]);

  // Debug skip changes time; refresh after any toast-producing action in HQ.
  useEffect(() => {
    refresh();
  }, [toastMsg, refresh]);

  const ctx: Ctx = useMemo(
    () => ({ game, view, now, open, run, toast, routeEdit, setRouteEdit, focusTile, setFocusTile }),
    [game, view, now, open, run, toast, routeEdit, focusTile],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (sheets.length) close();
      else if (routeEdit) setRouteEdit(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [sheets.length, close, routeEdit]);
  const alerts = view.pending.filter((p) => p.kind === 'alert' || p.kind === 'defend').length;

  return (
    <GameCtx.Provider value={ctx}>
      <div className="app">
        <TopBar />
        {view.phase === 'choosing_map' ? (
          <MapChoice />
        ) : (
          <div className="main">
            <div className="board-pane">
              {routeEdit ? <RouteEditor /> : null}
              <Board />
            </div>
            <div className="side-pane">
              <div className="tabs" role="tablist">
                <button className="tab" role="tab" aria-selected={tab === 'map'} onClick={() => setTab('map')}>
                  <Icon id="map" size={16} /> 地图
                  {view.pending.length ? <span className={'badge' + (alerts ? '' : ' ink')}>{view.pending.length}</span> : null}
                </button>
                <button className="tab" role="tab" aria-selected={tab === 'roster'} onClick={() => setTab('roster')}>
                  <Icon id="roster" size={16} /> 队员
                  {view.candidates.length ? <span className="badge ink">{view.candidates.length}</span> : null}
                </button>
                <button className="tab" role="tab" aria-selected={tab === 'hq'} onClick={() => setTab('hq')}>
                  <Icon id="org" size={16} /> 总部
                </button>
              </div>
              <div className="pane-body">
                {tab === 'map' ? <MapPanel /> : tab === 'roster' ? <RosterPanel /> : <HQPanel />}
              </div>
            </div>
          </div>
        )}
      </div>
      {sheets.map((s, k) => (
        // Lower sheets stay mounted (hidden) so going back keeps their state.
        <div key={k} style={{ display: k === sheets.length - 1 ? 'contents' : 'none' }}>
          <SheetHost sheet={s} onClose={close} />
        </div>
      ))}
      {toastMsg ? <div className={'toast' + (toastMsg.error ? ' error' : '')}>{toastMsg.text}</div> : null}
    </GameCtx.Provider>
  );
}

function SheetHost({ sheet, onClose }: { sheet: Sheet; onClose: () => void }) {
  switch (sheet.kind) {
    case 'tile':
      return <TileSheet key={sheet.tile} tile={sheet.tile} onClose={onClose} />;
    case 'op':
      return <OpSheet key={sheet.op} op={sheet.op} onClose={onClose} />;
    case 'candidate':
      return <CandidateSheet candidate={sheet.candidate} onClose={onClose} />;
    case 'district':
      return <DistrictSheet district={sheet.district} onClose={onClose} />;
    case 'report':
      return <ReportSheet title={sheet.title} load={sheet.load} onClose={onClose} />;
    case 'squad':
      return <SquadSheet squad={sheet.squad} onClose={onClose} />;
    case 'tide':
      return <TideSheet onClose={onClose} />;
  }
}

function TopBar() {
  const { view, now, game } = useCtx();
  return (
    <header className="topbar">
      <span className="brand">
        裂隙城 · 第 {view.cycle} 轮
        {view.modifier ? <span className="small muted"> · {view.modifier[1]}</span> : null}
      </span>
      {view.phase === 'active' ? (
        <div className="res">
          {RES_KEYS.map((k, i) => (
            <span key={k} className="res-item" title={RES_NAMES[k]}>
              <Icon id={`res-${k}`} size={16} />
              <span className="num">{view.resources[k]}</span>
              {view.income[i] ? <span className="rate">{fmtRate(view.income[i]!)}/时</span> : null}
            </span>
          ))}
          <span className={'res-item ' + (view.power[0] < view.power[1] ? 'bad' : '')} title="电力 供/需">
            <Icon id="power" size={16} />
            <span className="num">
              {view.power[0]}/{view.power[1]}
            </span>
          </span>
          {view.self_sufficient ? (
            <span className="chip good" title="所有边界格不派人也守得住">
              防线自给
            </span>
          ) : null}
        </div>
      ) : null}
      <span className="clock small muted">
        <Icon id="clock" size={14} />
        <span className="num">{fmtClock(now)}</span>
        {game.offset ? <span className="chip warn">快进 +{Math.round(game.offset / 3600)} 时</span> : null}
      </span>
    </header>
  );
}

function RouteEditor() {
  const { game, view, routeEdit, setRouteEdit, run } = useCtx();
  const edit = routeEdit!;
  const si = view.squads.findIndex((s) => s.id === edit.squad);
  const squad = view.squads[si]!;
  const preview = useMemo(() => (edit.route.length ? game.previewRoute(edit.squad, edit.route) : null), [game, edit]);
  return (
    <div className="route-mode-banner" style={{ borderColor: SQUAD_HEX[si] }}>
      <Icon id="route" size={18} />
      <div className="grow">
        <div>
          <b style={{ color: SQUAD_HEX[si] }}>{squad.name}</b> 编路线：从领地边上出发，依次点相邻的格子（点最后一格撤回）。
        </div>
        <div className="small">
          {edit.route.length ? edit.route.map((i) => view.tiles[i]!.label).join(' → ') : <span className="faint">还没选格子</span>}
        </div>
        {preview ? (
          preview.ok ? (
            <div className="small">
              预演：
              <b className={preview.outcome === 'victory' ? 'good' : 'bad'}>{OUTCOME_NAMES[preview.outcome ?? 'victory']}</b>
              {preview.steps
                .filter((s) => s.fought)
                .map((s) => (
                  <span key={s.tile} className={s.outcome === 'victory' ? 'good' : 'bad'}>
                    {' '}
                    {s.label}
                    {s.outcome === 'victory' ? '✓' : '✗'}
                  </span>
                ))}
              <span className="muted"> · 一趟约 {preview.minutes} 分</span>
              {preview.attention_after !== preview.attention_before ? (
                <span className="warn">
                  {' '}
                  · 关注度 {preview.attention_before}→{preview.attention_after}
                </span>
              ) : null}
            </div>
          ) : (
            <div className="small bad">{preview.error}</div>
          )
        ) : null}
      </div>
      <button
        className="btn primary small"
        disabled={!!preview && !preview.ok}
        onClick={() => {
          if (run({ cmd: 'set_route', squad: edit.squad, route: edit.route }, `${squad.name}路线已更新`)) setRouteEdit(null);
        }}
      >
        保存
      </button>
      <button className="btn small" onClick={() => setRouteEdit({ ...edit, route: [] })}>
        清空
      </button>
      <button className="btn small ghost" onClick={() => setRouteEdit(null)}>
        取消
      </button>
    </div>
  );
}

function MapChoice() {
  const { view, run } = useCtx();
  return (
    <div className="pane-body" style={{ maxWidth: 1000, margin: '0 auto', width: '100%' }}>
      <div>
        <h2 style={{ fontSize: 22 }}>第 {view.cycle} 轮 · 选一张城市地图</h2>
        <p className="muted">每张图的天险、裂隙和特殊地点都不同；本轮的异界气候决定来敌常带什么标签。</p>
      </div>
      <div className="mapchoice">
        {view.previews.map((p) => (
          <div key={p.index} className="card hatch stack">
            <MiniMap barriers={p.barriers} />
            <h3>{p.modifier_name}</h3>
            <div className="small muted">{p.modifier_desc}</div>
            <div className="small">
              裂隙 {p.rifts} · 特殊地点 {p.specials} · 街区 {p.districts}
            </div>
            <button className="btn primary" onClick={() => run({ cmd: 'choose_map', index: p.index }, '出发')}>
              选这张
            </button>
          </div>
        ))}
      </div>
      <HQPanel />
    </div>
  );
}

function MiniMap({ barriers }: { barriers: number[] }) {
  const N = 15;
  const s = 8;
  return (
    <svg viewBox={`0 0 ${N * s} ${N * s}`} style={{ width: '100%', maxWidth: 200, alignSelf: 'center' }}>
      <rect width={N * s} height={N * s} fill="#efe8d8" stroke="#26241f" />
      {barriers.map((i) => (
        <rect key={i} x={(i % N) * s} y={Math.floor(i / N) * s} width={s} height={s} fill="#4a4538" />
      ))}
      <rect x={7 * s} y={7 * s} width={s} height={s} fill="#d9472b" />
      {[1, 2, 3].map((r) => (
        <rect key={r} x={(7 - r) * s} y={(7 - r) * s} width={(2 * r + 1) * s} height={(2 * r + 1) * s} fill="none" stroke="#26241f" strokeOpacity={0.15} />
      ))}
    </svg>
  );
}
