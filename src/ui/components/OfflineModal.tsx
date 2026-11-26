import { EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES } from '../../content/facilities.ts';
import { SPECIAL_OPERATIVES } from '../../content/classesAndTrees.ts';
import { TACTIC_BLUEPRINTS } from '../../content/skillsAndTraits.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';

function formatDuration(sec: number): string {
  const s = Math.floor(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  if (h > 0) return `${h} 小时 ${m} 分`;
  if (m > 0) return `${m} 分 ${r} 秒`;
  return `${r} 秒`;
}

export function OfflineModal() {
  const { offlineSummary } = useGameStore();
  if (!offlineSummary) return null;

  return (
    <div className="modal-backdrop">
      <div className="modal-box">
        <div className="panel-header">
          <h2>离线归来结算（经过 {formatDuration(offlineSummary.elapsedSec)}）</h2>
          <button className="small" onClick={() => gameController.dismissOfflineModal()}>
            关闭
          </button>
        </div>

        <div className="card">
          <strong>据点产线净产出</strong>
          <div className="mono">
            废材 {offlineSummary.passiveYield.materials >= 0 ? '+' : ''}
            {offlineSummary.passiveYield.materials} · 零件 +
            {offlineSummary.passiveYield.parts} · 原晶 +
            {offlineSummary.passiveYield.crystals} · 手札 +
            {offlineSummary.passiveYield.manuals} · 情报 +
            {offlineSummary.passiveYield.intel}
          </div>
        </div>

        {offlineSummary.dojoCompletedText && (
          <div className="card" style={{ borderColor: 'var(--emerald)' }}>
            <strong style={{ color: 'var(--emerald)' }}>联合演武室传授完成</strong>
            <div>{offlineSummary.dojoCompletedText}</div>
          </div>
        )}

        {offlineSummary.squadReports.length > 0 && (
          <div className="card">
            <strong>小队自动巡逻收拢（确定性直算）</strong>
            {offlineSummary.squadReports.map((rep) => (
              <div
                key={rep.squadId}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  borderTop: '1px solid var(--border)',
                  paddingTop: '4px',
                  flexWrap: 'wrap',
                  gap: '6px',
                }}
              >
                <span>
                  {rep.squadName} · {rep.depthName}（{rep.completedCycles} 轮，
                  {rep.outcome === 'PLANNED'
                    ? '计划返回'
                    : rep.outcome === 'EMERGENCY'
                      ? '紧急撤退'
                      : '全灭保底'}
                  ）
                </span>
                <span className="mono">
                  废材+{rep.yieldEarned.materials} 零件+{rep.yieldEarned.parts} 原晶+
                  {rep.yieldEarned.crystals} 手札+{rep.yieldEarned.manuals}
                </span>
              </div>
            ))}
          </div>
        )}

        <div className="card">
          <strong>随机战利品开箱结果</strong>
          {offlineSummary.newBlueprints.length === 0 &&
            offlineSummary.newTactics.length === 0 &&
            offlineSummary.newSpecialOperatives.length === 0 &&
            offlineSummary.newEquipment.length === 0 &&
            offlineSummary.salvagedCount === 0 && (
              <div className="muted">本次离线暂无额外稀有掉落。</div>
            )}

          {offlineSummary.newSpecialOperatives.map((id) => (
            <div key={id} style={{ color: 'var(--amber)' }}>
              ★ 找回 SSR 特别队员：{SPECIAL_OPERATIVES[id]?.name ?? id}
            </div>
          ))}
          {offlineSummary.newBlueprints.map((id) => (
            <div key={id} style={{ color: 'var(--cyan)' }}>
              ❖ 解锁设施蓝图：{FACILITIES[id]?.name ?? id}
            </div>
          ))}
          {offlineSummary.newTactics.map((id) => (
            <div key={id} style={{ color: 'var(--cyan)' }}>
              ❖ 解锁战术芯片：{TACTIC_BLUEPRINTS[id]?.name ?? id}
            </div>
          ))}

          {offlineSummary.newEquipment.length > 0 && (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
              {offlineSummary.newEquipment.map((eq) => {
                const tpl = EQUIPMENT_TEMPLATES[eq.templateId];
                return (
                  <span key={eq.uid} className={`pill rarity-${eq.rarity}`}>
                    {tpl?.name ?? eq.templateId} (+{eq.bonusRollPct}%)
                  </span>
                );
              })}
            </div>
          )}

          {offlineSummary.salvagedCount > 0 && (
            <div className="muted mono">
              自动分解/溢出低阶装备 {offlineSummary.salvagedCount} 件（换得废材+
              {offlineSummary.salvageYield.materials} 零件+{offlineSummary.salvageYield.parts}）
            </div>
          )}
        </div>

        <button className="primary" onClick={() => gameController.dismissOfflineModal()}>
          收下战利品
        </button>
      </div>
    </div>
  );
}
