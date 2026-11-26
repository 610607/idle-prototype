import { useState } from 'react';
import { OTHERWORLD_MODIFIERS } from '../../content/hqAndModifiers.ts';
import { getPassiveBreakdown } from '../../engine/economy/baseAndPatrol.ts';
import { getNextStepGuide } from '../../engine/unfolding.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import { GameTooltip } from './GameTooltip.tsx';
import { ModifierTooltipCard } from './TooltipCards.tsx';

interface TopBarProps {
  onOpenSaveModal: () => void;
}

function CurrencyChip({
  icon,
  label,
  value,
  rate,
  color,
  detail,
}: {
  icon: string;
  label: string;
  value: number;
  rate?: number;
  color?: string;
  detail: string;
}) {
  return (
    <GameTooltip
      content={
        <div className="tt-card">
          <div className="tt-head-row">
            <strong style={{ color: color ?? '#e2e8f0' }}>
              {icon} {label}
            </strong>
            <span className="tt-badge mono">{Math.floor(value)}</span>
          </div>
          <div className="tt-body">{detail}</div>
        </div>
      }
    >
      <div className="currency-chip">
        <span>{icon}</span>
        <span className="muted" style={{ fontSize: '11px' }}>
          {label}
        </span>
        <strong className="mono" style={{ color }}>
          {Math.floor(value)}
        </strong>
        {rate !== undefined && rate !== 0 && (
          <span
            className="mono"
            style={{
              fontSize: '10.5px',
              color: rate > 0 ? 'var(--emerald)' : 'var(--amber)',
            }}
          >
            {rate > 0 ? '+' : ''}
            {rate.toFixed(1)}/s
          </span>
        )}
      </div>
    </GameTooltip>
  );
}

export function TopBar({ onOpenSaveModal }: TopBarProps) {
  const { state, powerGrid } = useGameStore();
  const [showDebug, setShowDebug] = useState(false);

  const breakdown = getPassiveBreakdown(state);
  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  const u = state.hq.story.unfolding;
  const guide = getNextStepGuide(state);

  const showParts = u.workshopUnlocked || state.run.resources.parts > 0;
  const showCrystals = u.otherworldUnlocked || state.run.resources.crystals > 0;
  const showManuals = u.workshopUnlocked || state.run.resources.manuals > 0;
  const showIntel = u.otherworldUnlocked || state.run.resources.intel > 0;
  const showOrgExp = u.hqUnlocked || state.hq.orgExp > 0;
  const showPower = u.baseUnlocked || powerGrid.demand > 0;
  const hasBrownout = powerGrid.brownedOutPlotIds.length > 0;

  return (
    <header className="hud-topbar">
      <div className="hud-brand">
        <h1>自治街区</h1>
        {(u.hqUnlocked || state.hq.story.cycleCount > 0) && (
          <span className="pill cyan mono">R{state.hq.story.cycleCount}</span>
        )}
        {(u.otherworldUnlocked || state.hq.story.cycleCount > 0) && modifier && (
          <GameTooltip content={<ModifierTooltipCard modifierId={modifier.id} />}>
            <span className="pill purple" style={{ cursor: 'help' }}>
              {modifier.name}
            </span>
          </GameTooltip>
        )}
      </div>

      <div className="hud-currencies">
        <CurrencyChip
          icon="⚙"
          label="废材"
          value={state.run.resources.materials}
          rate={breakdown.net.materials}
          detail={`产出 +${breakdown.gross.materials.toFixed(1)}/s · 工坊加工耗材 -${breakdown.materialsConsume.toFixed(1)}/s`}
        />

        {showParts && (
          <CurrencyChip
            icon="🔩"
            label="零件"
            value={state.run.resources.parts}
            rate={breakdown.net.parts}
            color="#93c5fd"
            detail="由精密改装工坊加工废材产出，用于高阶设施建造与装备锻造。"
          />
        )}

        {showCrystals && (
          <CurrencyChip
            icon="💎"
            label="原晶"
            value={state.run.resources.crystals}
            rate={breakdown.net.crystals}
            color="#c084fc"
            detail="来自异界探索与巡逻，用于设施突破改造与遗物锻造。"
          />
        )}

        {showManuals && (
          <CurrencyChip
            icon="📖"
            label="手札"
            value={state.run.resources.manuals}
            rate={breakdown.net.manuals}
            color="#34d399"
            detail="由档案研习室编译或异界获取，用于队员特训升级、转职与战术破译。"
          />
        )}

        {showIntel && (
          <CurrencyChip
            icon="📡"
            label="情报"
            value={state.run.resources.intel}
            rate={breakdown.net.intel}
            color="#38bdf8"
            detail="累计情报越多，大渗透重开时结算的组织经验越高。"
          />
        )}

        {showOrgExp && (
          <CurrencyChip
            icon="🏛"
            label="经验"
            value={state.hq.orgExp}
            color="#fbbf24"
            detail="组织总部永久货币，大渗透结算后用于升级永久部门规章。"
          />
        )}

        {showPower && (
          <GameTooltip
            content={
              <div className="tt-card">
                <div className="tt-head-row">
                  <strong>⚡ 电网负载</strong>
                  <span className="tt-badge mono">
                    {powerGrid.demand} / {powerGrid.generated} kW
                  </span>
                </div>
                <div className="tt-body">
                  {hasBrownout
                    ? `当前欠电，${powerGrid.brownedOutPlotIds.length} 处边缘设施暂缓运转（不丢失等级与占领）。请升级发电设施或驻扎 STR+CON 队员。`
                    : '电力充沛，所有启用的据点设施满效率运转中。'}
                </div>
              </div>
            }
          >
            <div
              className="currency-chip"
              style={{
                borderColor: hasBrownout ? 'var(--amber)' : 'rgba(52, 211, 153, 0.4)',
              }}
            >
              <span>⚡</span>
              <span
                className="mono"
                style={{
                  fontWeight: 600,
                  color: hasBrownout ? 'var(--amber)' : 'var(--emerald)',
                }}
              >
                {powerGrid.demand}/{powerGrid.generated}kW
              </span>
            </div>
          </GameTooltip>
        )}
      </div>

      <div className="hud-right">
        <GameTooltip
          content={
            <div className="tt-card">
              <strong style={{ color: 'var(--cyan)' }}>{guide.title}</strong>
              <div className="tt-body">{guide.detail}</div>
              <div className="tt-meta">点击即可快速跳转至对应页面</div>
            </div>
          }
        >
          <button
            className="guide-pill"
            onClick={() => guide.targetTab && gameController.setTab(guide.targetTab)}
          >
            🎯 {guide.title.replace(/^当前目标：|^提示：/, '')}
          </button>
        </GameTooltip>

        <button className="small" onClick={onOpenSaveModal} title="存档导入导出">
          💾
        </button>
        <button className="small" onClick={() => setShowDebug((v) => !v)} title="原型调试">
          🛠
        </button>
      </div>

      {showDebug && (
        <div className="debug-popover">
          <div className="card-title-row">
            <strong>原型调试工具</strong>
            <button className="small" onClick={() => setShowDebug(false)}>
              ×
            </button>
          </div>
          <button className="small" onClick={() => gameController.runSimulateOffline(600)}>
            模拟离线 10 分钟
          </button>
          <button className="small" onClick={() => gameController.runSimulateOffline(7200)}>
            模拟离线 2 小时
          </button>
          <button className="small" onClick={() => gameController.debugAddResources()}>
            +全资源测试包
          </button>
          <button className="small primary" onClick={() => gameController.doDebugUnlockAll()}>
            🔓 展开全部系统（跳过渐进引导）
          </button>
          <div className="muted mono" style={{ fontSize: '10.5px' }}>
            PRNG: {state.run.prngState >>> 0} · 已运行 {Math.floor(state.totalPlayTimeSec)}s
          </div>
        </div>
      )}
    </header>
  );
}
