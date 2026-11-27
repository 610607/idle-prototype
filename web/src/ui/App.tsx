import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { Game } from '../engine/client';
import { Icon } from '../icons';
import type { Command, View } from '../types';
import { ReportSheet } from './BattleReport';
import { Board, WASH } from './Board';
import { ConfirmDialog, fmtClock, fmtRate, GameCtx, OUTCOME_NAMES, RES_KEYS, RES_NAMES, SQUAD_HEX, useCtx, type ConfirmAsk, type Ctx, type RouteEdit, type Sheet } from './common';
import { CommandPanel, DistrictsPanel, HQPanel, PendingList, RosterPanel, TideStrip } from './Panels';
import { Archive, ResearchPanel } from './Research';
import { CandidateSheet, DistrictSheet, OpSheet, SquadSheet, TideSheet } from './Sheets';
import { TileSheet } from './TileSheet';

/** `map` only exists on phones, where the map and the workspace take turns. */
type Tab = 'map' | 'command' | 'roster' | 'districts' | 'research' | 'hq';
const TABS: { id: Exclude<Tab, 'map'>; name: string; icon: string }[] = [
  { id: 'command', name: '指挥', icon: 'squad' },
  { id: 'roster', name: '队员', icon: 'roster' },
  { id: 'districts', name: '街区', icon: 'trust' },
  { id: 'research', name: '研究', icon: 'trait-book' },
  { id: 'hq', name: '总部', icon: 'org' },
];

function sameSheet(a: Sheet, b: Sheet): boolean {
  if (a.kind !== b.kind) return false;
  switch (a.kind) {
    case 'tile':
      return a.tile === (b as typeof a).tile;
    case 'op':
      return a.op === (b as typeof a).op;
    case 'candidate':
      return a.candidate === (b as typeof a).candidate;
    case 'district':
      return a.district === (b as typeof a).district;
    case 'squad':
      return a.squad === (b as typeof a).squad;
    case 'tide':
      return true;
    case 'report':
      return a.title === (b as typeof a).title;
  }
}

export function App({ game }: { game: Game }) {
  const [view, setView] = useState<View>(() => game.view());
  const [now, setNow] = useState(() => game.now());
  const [tab, setTab] = useState<Tab>(() => (window.matchMedia('(max-width: 900px)').matches ? 'map' : 'command'));
  const [pages, setPages] = useState<Sheet[]>([]);
  const [ask, setAsk] = useState<ConfirmAsk | null>(null);
  const [toastMsg, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const [routeEdit, setRouteEdit] = useState<RouteEdit | null>(null);
  const [focusTile, setFocusTile] = useState<number | null>(null);
  const lastRefresh = useRef(0);
  const workRef = useRef<HTMLDivElement>(null);

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

  const open = useCallback((s: Sheet | null, reset?: boolean) => {
    setPages((st) => {
      if (!s) return [];
      if (reset) return [s];
      const top = st[st.length - 1];
      return top && sameSheet(top, s) ? st : [...st, s];
    });
  }, []);
  // Dev builds only: lets scripts/text_dump.mjs open every sheet.
  useEffect(() => {
    if (import.meta.env.DEV) Object.assign(window, { __idle: { open, game, view: () => game.view() } });
  }, [open, game]);
  const popTo = useCallback(
    (n: number) => {
      setPages((st) => st.slice(0, n));
      refresh();
    },
    [refresh],
  );
  const back = useCallback(() => {
    setPages((st) => st.slice(0, -1));
    refresh();
  }, [refresh]);

  // New page: start at its top.
  useEffect(() => {
    workRef.current?.scrollTo({ top: 0 });
  }, [pages.length]);

  // Debug skip changes time; refresh after any toast-producing action in HQ.
  useEffect(() => {
    refresh();
  }, [toastMsg, refresh]);

  const top = pages[pages.length - 1];
  // What the open page points at on the map: the tile itself, or where an operative stands guard.
  const selectedTile = useMemo(() => {
    if (top?.kind === 'tile') return top.tile;
    if (top?.kind === 'op') {
      const a = view.operatives.find((o) => o.id === top.op)?.assignment;
      if (a?.t === 'garrison') return a.tile;
    }
    return null;
  }, [top, view.operatives]);
  const workTab: Exclude<Tab, 'map'> = tab === 'map' ? 'command' : tab;
  const tabName = tab === 'map' ? '地图' : TABS.find((t) => t.id === workTab)!.name;

  const goTab = useCallback((t: Exclude<Tab, 'map'>) => {
    setTab(t);
    setPages([]);
  }, []);
  const ctx: Ctx = useMemo(
    () => ({ game, view, now, open, pages, popTo, selectedTile, confirm: setAsk, run, toast, routeEdit, setRouteEdit, focusTile, setFocusTile, tabName, goTab }),
    [game, view, now, open, pages, popTo, selectedTile, run, toast, routeEdit, focusTile, tabName, goTab],
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (ask) setAsk(null);
      else if (pages.length) back();
      else if (routeEdit) setRouteEdit(null);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [ask, pages.length, back, routeEdit]);

  const alerts = view.pending.filter((p) => p.kind === 'alert' || p.kind === 'defend').length;
  const badge = (t: Tab) => {
    if (t === 'map' && view.pending.length) return <span className={'badge' + (alerts ? '' : ' ink')}>{view.pending.length}</span>;
    if (t === 'command' && view.tasks.open.some((x) => x.done)) return <span className="badge ink">!</span>;
    if (t === 'roster' && view.candidates.length) return <span className="badge ink">{view.candidates.length}</span>;
    if (t === 'districts') {
      const n = view.districts.filter((d) => d.policy_unlocked && !d.policy).length;
      return n ? <span className="badge ink">{n}</span> : null;
    }
    if (t === 'research') {
      if (view.research.offers.length) return <span className="badge ink">3</span>;
      return view.research.affordable && view.research.left ? <span className="badge ink">!</span> : null;
    }
    return null;
  };
  const pick = (t: Tab) => {
    setTab(t);
    setPages([]);
  };
  // Phones show one column at a time: the map while editing a route or on the map tab, otherwise the workspace.
  const mobileView = routeEdit ? 'map' : pages.length ? 'work' : tab === 'map' ? 'map' : 'work';

  return (
    <GameCtx.Provider value={ctx}>
      <div className="app">
        <TopBar />
        {view.phase === 'choosing_map' ? (
          <MapChoice />
        ) : (
          <div className="main" data-mv={mobileView}>
            <div className="map-col">
              <Board />
              {routeEdit ? <RouteEditor /> : null}
              <TideStrip />
              <PendingList />
            </div>
            <div className="work">
              <div className="work-tabs" role="tablist">
                {TABS.map((t) => (
                  <button key={t.id} className="tab" role="tab" aria-selected={!pages.length && workTab === t.id} onClick={() => pick(t.id)}>
                    <Icon id={t.icon} /> {t.name}
                    {badge(t.id)}
                  </button>
                ))}
              </div>
              <div className="work-body" ref={workRef}>
                {pages.length === 0 ? (
                  <div className="pane">
                    {workTab === 'command' ? <CommandPanel /> : workTab === 'roster' ? <RosterPanel /> : workTab === 'districts' ? <DistrictsPanel /> : workTab === 'research' ? <ResearchPanel /> : <HQPanel />}
                  </div>
                ) : null}
                {pages.map((s, k) => (
                  // Lower pages stay mounted (hidden) so going back keeps their state.
                  <div key={k} style={{ display: k === pages.length - 1 ? 'contents' : 'none' }}>
                    <SheetHost sheet={s} onClose={back} />
                  </div>
                ))}
              </div>
            </div>
            <nav className="bottom-nav">
              {([{ id: 'map', name: '地图', icon: 'map' }, ...TABS] as const).map((t) => (
                <button key={t.id} aria-selected={mobileView === 'map' ? t.id === 'map' : !pages.length ? tab === t.id : false} onClick={() => pick(t.id)}>
                  <Icon id={t.icon} />
                  {t.name}
                  {badge(t.id)}
                </button>
              ))}
            </nav>
          </div>
        )}
      </div>
      {ask ? <ConfirmDialog ask={ask} onDone={() => setAsk(null)} /> : null}
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
        <span className="stamp">民防</span>
        <h1>裂隙城</h1>
        <span className="cycle">
          第 {view.cycle} 轮{view.modifier ? ` · ${view.modifier[1]}` : ''}
        </span>
      </span>
      {view.phase === 'active' ? (
        <div className="res">
          {RES_KEYS.map((k, i) => (
            <span key={k} className="res-item" title={RES_NAMES[k]}>
              <Icon id={`res-${k}`} />
              <span className="num">{view.resources[k]}</span>
              {view.income[i] ? <span className="rate">{fmtRate(view.income[i]!)}/时</span> : null}
            </span>
          ))}
          <span className={'res-item ' + (view.power[0] < view.power[1] ? 'bad' : '')} title="电力 供/需">
            <Icon id="power" />
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
      <span className="clock muted">
        {game.offset ? <span className="chip warn">快进 +{Math.round(game.offset / 3600)} 时</span> : null}
        <Icon id="clock" />
        <span className="num">{fmtClock(now)}</span>
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
    <div className="route-mode-banner" style={{ ['--route-color' as string]: SQUAD_HEX[si] }}>
      <Icon id="route" />
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
    <div className="work-body">
      <div className="pane" style={{ maxWidth: 1200, margin: '0 auto', width: '100%' }}>
        <div>
          <h2 style={{ fontSize: 26 }}>第 {view.cycle} 轮 · 选一张城市地图</h2>
          <p className="muted">每张图的天险、裂隙和特殊地点都不同；本轮的异界气候决定来敌常带什么标签。</p>
        </div>
        <div className="mapchoice">
          {view.previews.map((p) => (
            <div key={p.index} className="card rivet stack">
              <MiniMap barriers={p.barriers} />
              <h3 style={{ fontSize: 18 }}>{p.modifier_name}</h3>
              <div className="small muted">{p.modifier_desc}</div>
              <div className="small">
                裂隙 <span className="num">{p.rifts}</span> · 特殊地点 <span className="num">{p.specials}</span> · 街区 <span className="num">{p.districts}</span>
              </div>
              <button className="btn primary" onClick={() => run({ cmd: 'choose_map', index: p.index }, '出发')}>
                选这张
              </button>
            </div>
          ))}
        </div>
        {view.research.library.length ? <Archive /> : null}
        <HQPanel />
      </div>
    </div>
  );
}

function MiniMap({ barriers }: { barriers: number[] }) {
  const N = 15;
  const s = 8;
  return (
    <svg viewBox={`0 0 ${N * s} ${N * s}`} style={{ width: '100%', maxWidth: 180, alignSelf: 'center' }} shapeRendering="crispEdges">
      <rect width={N * s} height={N * s} fill="#1d201a" />
      {Array.from({ length: N * N }, (_, i) => (
        <rect key={i} x={(i % N) * s} y={Math.floor(i / N) * s} width={s - 1} height={s - 1} fill={WASH[(Math.floor((i % N) / 5) + Math.floor(i / N / 5) * 3) % WASH.length]} opacity={0.35} />
      ))}
      {barriers.map((i) => (
        <rect key={'b' + i} x={(i % N) * s} y={Math.floor(i / N) * s} width={s - 1} height={s - 1} fill="#4a463d" />
      ))}
      <rect x={7 * s} y={7 * s} width={s - 1} height={s - 1} fill="#e0b43c" />
    </svg>
  );
}
