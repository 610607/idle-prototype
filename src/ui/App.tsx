import { useState } from 'react';
import { OTHERWORLD_DEPTHS } from '../content/worldNodes.ts';
import { gameController, useGameStore, type ActiveTab } from '../state/gameStore.ts';
import { BaseTab } from './components/BaseTab.tsx';
import { ExpeditionTab } from './components/ExpeditionTab.tsx';
import { FormationTab } from './components/FormationTab.tsx';
import { GameTooltip } from './components/GameTooltip.tsx';
import { HQPrestigeTab } from './components/HQPrestigeTab.tsx';
import { OfflineModal } from './components/OfflineModal.tsx';
import { OperativesTab } from './components/OperativesTab.tsx';
import { SaveModal } from './components/SaveModal.tsx';
import { TopBar } from './components/TopBar.tsx';

export function App() {
  const { state, activeTab, toast } = useGameStore();
  const [saveModalOpen, setSaveModalOpen] = useState(false);

  const u = state.hq.story.unfolding;
  const hasUnspentSp = state.run.roster.some((op) => op.skillPoints > 0);
  const dojoReady = state.run.dojo === null && u.dojoUnlocked;
  const patrollingSquads = state.run.squads.filter((s) => Boolean(s.assignedDepthId));

  const tabs: Array<{
    id: ActiveTab;
    icon: string;
    label: string;
    unlocked: boolean;
    lockHint?: string;
    dot?: boolean;
  }> = [
    { id: 'expedition', icon: '⚔', label: '前线行动', unlocked: true },
    {
      id: 'formation',
      icon: '♟',
      label: '战术编队',
      unlocked: true,
    },
    {
      id: 'operatives',
      icon: '👤',
      label: '调查员',
      unlocked: true,
      dot: hasUnspentSp,
    },
    {
      id: 'base',
      icon: '🏗',
      label: '街区基建',
      unlocked: u.baseUnlocked,
      lockHint: '夺回「一环 · 老商业街口」后开放街区地块与设施建设。',
      dot: dojoReady,
    },
    {
      id: 'hq',
      icon: '🏛',
      label: '组织总部',
      unlocked: u.hqUnlocked,
      lockHint: '攻克三环街区、打通二层异界或积累 25 点异常情报后解锁。',
    },
  ];

  return (
    <div className="app-frame">
      <TopBar onOpenSaveModal={() => setSaveModalOpen(true)} />

      <div className="app-body">
        <aside className="left-rail">
          <nav className="rail-nav">
            {tabs.map((t) =>
              t.unlocked ? (
                <button
                  key={t.id}
                  className={`rail-btn ${activeTab === t.id ? 'active' : ''}`}
                  onClick={() => gameController.setTab(t.id)}
                >
                  <span>
                    {t.icon} {t.label}
                  </span>
                  {t.dot && <span className="dot-badge" />}
                </button>
              ) : (
                <GameTooltip
                  key={t.id}
                  as="div"
                  content={
                    <div className="tt-card">
                      <strong>🔒 {t.label}（未解锁）</strong>
                      <div className="tt-body">{t.lockHint}</div>
                    </div>
                  }
                >
                  <button className="rail-btn locked" disabled style={{ width: '100%' }}>
                    <span>🔒 {t.label}</span>
                  </button>
                </GameTooltip>
              ),
            )}
          </nav>

          <div className="rail-footer">
            {patrollingSquads.length > 0 && (
              <div className="mini-patrol-box">
                <strong style={{ color: 'var(--cyan)' }}>驻点巡逻监视</strong>
                {patrollingSquads.map((ps) => {
                  const bench = state.run.patrolBenchmarks[ps.id];
                  if (!bench) return null;
                  const pct = Math.min(
                    100,
                    Math.round((ps.patrolProgressSec / bench.durationSec) * 100),
                  );
                  const dName =
                    OTHERWORLD_DEPTHS.find((d) => d.id === bench.depthId)?.name.split('·')[0] ??
                    bench.depthId;
                  return (
                    <div key={ps.id} style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                        <span>{ps.name} · {dName}</span>
                        <span className="mono">{pct}%</span>
                      </div>
                      <div className="progress-track">
                        <div className="progress-fill" style={{ width: `${pct}%` }} />
                      </div>
                    </div>
                  );
                })}
              </div>
            )}

            {state.run.eventTicker.length > 0 && (
              <div className="mini-ticker">
                <strong style={{ color: 'var(--muted)' }}>近期动态</strong>
                {state.run.eventTicker.slice(0, 2).map((line, i) => (
                  <div key={i} style={{ color: i === 0 ? 'var(--text)' : 'var(--dim)' }}>
                    • {line}
                  </div>
                ))}
              </div>
            )}
          </div>
        </aside>

        <main className="main-viewport">
          {activeTab === 'expedition' && <ExpeditionTab />}
          {activeTab === 'formation' && <FormationTab />}
          {activeTab === 'operatives' && <OperativesTab />}
          {activeTab === 'base' && <BaseTab />}
          {activeTab === 'hq' && <HQPrestigeTab />}
        </main>
      </div>

      {toast && (
        <div className="toast-floating">
          <span>{toast}</span>
          <button className="small" onClick={() => gameController.clearToast()}>
            ×
          </button>
        </div>
      )}

      <OfflineModal />
      <SaveModal open={saveModalOpen} onClose={() => setSaveModalOpen(false)} />
    </div>
  );
}
