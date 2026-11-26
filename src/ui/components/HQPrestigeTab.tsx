import { useEffect, useState } from 'react';
import { CLASSES, SPECIAL_OPERATIVES } from '../../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES } from '../../content/facilities.ts';
import {
  HQ_DEPARTMENTS,
  HQ_DEPARTMENT_IDS,
  getDepartmentUpgradeCost,
} from '../../content/hqAndModifiers.ts';
import { TACTIC_BLUEPRINTS } from '../../content/skillsAndTraits.ts';
import { getLoadoutCapacity, getVaultCapacity } from '../../engine/operativeHelpers.ts';
import { calculatePendingOrgExp, collectAllRunEquipment } from '../../engine/prestige.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import { GameTooltip } from './GameTooltip.tsx';
import { DepartmentTooltipCard, EquipmentTooltipCard } from './TooltipCards.tsx';

export function HQPrestigeTab() {
  const { state, vaultMarkedUids } = useGameStore();
  const pendingOrgExp = calculatePendingOrgExp(state);
  const loadoutCap = getLoadoutCapacity(state.hq);
  const vaultCap = getVaultCapacity(state.hq);

  const [selectedBlueprints, setSelectedBlueprints] = useState<string[]>(() =>
    state.hq.unlockedBlueprints.slice(0, loadoutCap.blueprints),
  );
  const [selectedTactics, setSelectedTactics] = useState<string[]>(() =>
    state.hq.unlockedTactics.slice(0, loadoutCap.tactics),
  );

  useEffect(() => {
    setSelectedBlueprints((prev) => {
      const valid = prev.filter((id) => state.hq.unlockedBlueprints.includes(id));
      for (const id of state.hq.unlockedBlueprints) {
        if (valid.length >= loadoutCap.blueprints) break;
        if (!valid.includes(id)) valid.push(id);
      }
      return valid;
    });
  }, [state.hq.unlockedBlueprints, loadoutCap.blueprints]);

  useEffect(() => {
    setSelectedTactics((prev) => {
      const valid = prev.filter((id) => state.hq.unlockedTactics.includes(id));
      for (const id of state.hq.unlockedTactics) {
        if (valid.length >= loadoutCap.tactics) break;
        if (!valid.includes(id)) valid.push(id);
      }
      return valid;
    });
  }, [state.hq.unlockedTactics, loadoutCap.tactics]);

  const toggleBlueprint = (id: string) => {
    setSelectedBlueprints((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      if (prev.length >= loadoutCap.blueprints) return prev;
      return [...prev, id];
    });
  };

  const toggleTactic = (id: string) => {
    setSelectedTactics((prev) => {
      if (prev.includes(id)) return prev.filter((x) => x !== id);
      if (prev.length >= loadoutCap.tactics) return prev;
      return [...prev, id];
    });
  };

  const allRunEquipment = collectAllRunEquipment(state);
  const markedEquipment = allRunEquipment.filter((e) => vaultMarkedUids.includes(e.uid));

  return (
    <div className="hq-layout">
      {/* 左栏：永久部门规章（悬停看详细说明） */}
      <section className="panel">
        <div className="panel-header">
          <h2>永久部门与规章（悬停查详情）</h2>
          <span className="pill amber mono">组织经验：{state.hq.orgExp}</span>
        </div>

        <div className="plot-grid">
          {HQ_DEPARTMENT_IDS.map((id) => {
            const def = HQ_DEPARTMENTS[id]!;
            const lvl = state.hq.departments[id] ?? 0;
            const cost = getDepartmentUpgradeCost(def, lvl);
            const isMax = lvl >= def.maxLevel;
            const canBuy = !isMax && state.hq.orgExp >= cost;
            return (
              <GameTooltip
                key={id}
                as="div"
                content={<DepartmentTooltipCard deptId={id} level={lvl} />}
              >
                <div className="card">
                  <div className="card-title-row">
                    <strong>{def.name}</strong>
                    <span className="pill mono">Lv.{lvl}/{def.maxLevel}</span>
                  </div>
                  <button
                    className={`small ${canBuy ? 'affordable' : ''}`}
                    disabled={!canBuy}
                    onClick={() => gameController.doUpgradeDepartment(id)}
                  >
                    {isMax ? '满级' : `升级 (${cost} 经验)`}
                  </button>
                </div>
              </GameTooltip>
            );
          })}
        </div>

        <div className="panel-header" style={{ marginTop: '4px' }}>
          <h3>SSR 特别队员传承档案</h3>
          <span className="pill muted mono">
            Ring {state.hq.story.maxDistrictRingEver} · Depth {state.hq.story.maxOtherworldDepthEver}
          </span>
        </div>

        {state.hq.specialOperatives.length === 0 ? (
          <div className="muted" style={{ fontSize: '12px' }}>
            尚未从异界找回 SSR 特别队员。
          </div>
        ) : (
          <div className="plot-grid">
            {state.hq.specialOperatives.map((sp) => {
              const tpl = SPECIAL_OPERATIVES[sp.archetypeId];
              if (!tpl) return null;
              const active = state.run.roster.find(
                (o) => o.isSpecial && o.archetypeId === sp.archetypeId,
              );
              const curClassName =
                CLASSES[active?.classId ?? sp.classId ?? tpl.defaultClassId]?.name;
              const learnedSkills =
                active?.learnedSkillIds.length ?? sp.learnedSkillIds?.length ?? 0;
              const learnedTraits =
                active?.learnedTraitIds.length ?? sp.learnedTraitIds?.length ?? 0;
              return (
                <div key={sp.archetypeId} className="card">
                  <div className="card-title-row">
                    <span>
                      <span className="pill rarity-SSR" style={{ marginRight: '4px' }}>
                        SSR
                      </span>
                      <strong>{tpl.name}</strong>
                    </span>
                    <span className="muted" style={{ fontSize: '11px' }}>
                      {curClassName}
                    </span>
                  </div>
                  <div className="mono muted" style={{ fontSize: '11px' }}>
                    保留星级 · 继承技 {learnedSkills} · 传承词条 {learnedTraits}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {/* 右栏：大渗透祭坛与下轮选配 */}
      <section className="panel">
        <div className="panel-header">
          <h2>收束防线 · 迎接大渗透</h2>
          <span className="pill amber mono">预计结算 +{pendingOrgExp} 经验</span>
        </div>

        <div className="card">
          <div className="card-title-row">
            <strong>铅封库装备（{markedEquipment.length}/{vaultCap} 格）</strong>
          </div>
          {markedEquipment.length === 0 ? (
            <div className="muted" style={{ fontSize: '11.5px' }}>
              可在「调查员编队 → 装备库」点击 ☆ 封存极品装备。
            </div>
          ) : (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px' }}>
              {markedEquipment.map((eq) => {
                const tpl = EQUIPMENT_TEMPLATES[eq.templateId];
                return (
                  <GameTooltip key={eq.uid} content={<EquipmentTooltipCard eq={eq} />}>
                    <span className={`pill rarity-${eq.rarity}`} style={{ cursor: 'help' }}>
                      ★ {tpl?.name ?? eq.templateId}
                      <button
                        className="small"
                        style={{ padding: '0 4px', marginLeft: '3px' }}
                        onClick={() => gameController.doToggleVaultMark(eq.uid)}
                      >
                        ×
                      </button>
                    </span>
                  </GameTooltip>
                );
              })}
            </div>
          )}
        </div>

        <div className="card">
          <strong>下轮携带设施蓝图（{selectedBlueprints.length}/{loadoutCap.blueprints}）</strong>
          {state.hq.unlockedBlueprints.length === 0 ? (
            <div className="muted" style={{ fontSize: '11.5px' }}>
              尚未从异界解锁额外设施蓝图。
            </div>
          ) : (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
              {state.hq.unlockedBlueprints.map((bid) => (
                <GameTooltip
                  key={bid}
                  content={
                    <div className="tt-card">
                      <strong>{FACILITIES[bid]?.name ?? bid}</strong>
                      <div className="tt-body">{FACILITIES[bid]?.effectSummary}</div>
                    </div>
                  }
                >
                  <label style={{ display: 'inline-flex', gap: '5px', alignItems: 'center', cursor: 'help' }}>
                    <input
                      type="checkbox"
                      checked={selectedBlueprints.includes(bid)}
                      onChange={() => toggleBlueprint(bid)}
                    />
                    <span>{FACILITIES[bid]?.name ?? bid}</span>
                  </label>
                </GameTooltip>
              ))}
            </div>
          )}
        </div>

        <div className="card">
          <strong>下轮携带战术芯片（{selectedTactics.length}/{loadoutCap.tactics}）</strong>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
            {state.hq.unlockedTactics.map((tid) => (
              <GameTooltip
                key={tid}
                content={
                  <div className="tt-card">
                    <strong>{TACTIC_BLUEPRINTS[tid]?.name ?? tid}</strong>
                    <div className="tt-body">{TACTIC_BLUEPRINTS[tid]?.description}</div>
                  </div>
                }
              >
                <label style={{ display: 'inline-flex', gap: '5px', alignItems: 'center', cursor: 'help' }}>
                  <input
                    type="checkbox"
                    checked={selectedTactics.includes(tid)}
                    onChange={() => toggleTactic(tid)}
                  />
                  <span>{TACTIC_BLUEPRINTS[tid]?.name ?? tid}</span>
                </label>
              </GameTooltip>
            ))}
          </div>
        </div>

        <button
          className="primary"
          disabled={pendingOrgExp <= 0}
          onClick={() =>
            gameController.doGreatInfiltration({
              blueprints: selectedBlueprints,
              tactics: selectedTactics,
            })
          }
        >
          {pendingOrgExp <= 0
            ? '需至少夺回街区或积累情报后再收束防线'
            : `确认收束防线（+${pendingOrgExp} 组织经验，进入第 ${state.hq.story.cycleCount + 1} 轮）`}
        </button>
      </section>
    </div>
  );
}
