import { CLASSES, SPECIAL_OPERATIVES } from '../../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES, getFacilityUpgradeCost } from '../../content/facilities.ts';
import { HQ_DEPARTMENTS, OTHERWORLD_MODIFIERS, getDepartmentUpgradeCost } from '../../content/hqAndModifiers.ts';
import { SKILLS, TACTIC_BLUEPRINTS, TRAITS } from '../../content/skillsAndTraits.ts';
import { TAG_META } from '../../content/tags.ts';
import { getPlotEfficiencyMultiplier } from '../../engine/economy/baseAndPatrol.ts';
import { getStationingBonusPct } from '../../engine/operativeHelpers.ts';
import type {
  ClassId,
  CoCStatKey,
  ConditionType,
  GambitRule,
  TagId,
  TargetSelector,
} from '../../types/combat.ts';
import type {
  BuildingPlot,
  DistrictDef,
  EnemyUnitDef,
  EquipmentInstance,
  GameState,
  OtherworldDepthDef,
  SkillTreeNodeDef,
} from '../../types/game.ts';

function collectSkillTags(skillId: string): TagId[] {
  const sk = SKILLS[skillId];
  if (!sk) return [];
  const tags = new Set<TagId>();
  if (sk.requiredEnemyTag) tags.add(sk.requiredEnemyTag);
  if (sk.requiredSelfTag) tags.add(sk.requiredSelfTag);
  if (sk.spreadTag) tags.add(sk.spreadTag);
  if (sk.consumeEnemyTagBonus) tags.add(sk.consumeEnemyTagBonus.tag);
  if (sk.shieldRatio) tags.add('shielded');
  for (const t of sk.applyTargetTags ?? []) tags.add(t.tag);
  for (const t of sk.applySelfTags ?? []) tags.add(t.tag);
  for (const t of sk.applyAttackerTags ?? []) tags.add(t.tag);
  return Array.from(tags);
}

export function TagTooltipCard({ tagId }: { tagId: TagId }) {
  const meta = TAG_META[tagId];
  if (!meta) return null;
  return (
    <div className="tt-subcard">
      <div className="tt-head-row">
        <span style={{ color: meta.color, fontWeight: 700 }}>{meta.label}</span>
        <span className="tt-meta">叠层上限 {meta.maxStacks}</span>
      </div>
      <div className="tt-body">{meta.description}</div>
    </div>
  );
}

export function SkillTooltipCard({
  skillId,
  nested = false,
}: {
  skillId: string;
  nested?: boolean;
}) {
  const sk = SKILLS[skillId];
  if (!sk) return null;
  const tags = collectSkillTags(skillId);
  const rangeLabel =
    sk.range === 'melee' ? '近战（优先前排）' : sk.range === 'ranged' ? '远程（可越排）' : '支援';

  return (
    <div className={nested ? 'tt-subcard' : 'tt-card'}>
      <div className="tt-head-row">
        <strong style={{ color: sk.kind === 'passive' ? '#c084fc' : '#38bdf8' }}>
          {sk.name}
        </strong>
        <span className="tt-badge">
          {sk.kind === 'passive' ? `${sk.ppCost} PP 被动反应` : `${sk.apCost} AP 主动`}
        </span>
      </div>
      <div className="tt-meta-row">
        <span>射程：{rangeLabel}</span>
        {sk.scaleStat && <span>关联：{sk.scaleStat.toUpperCase()}</span>}
        {sk.powerMultiplier > 0 && <span>倍率：{Math.round(sk.powerMultiplier * 100)}%</span>}
      </div>
      <div className="tt-body">{sk.description}</div>
      {tags.length > 0 && (
        <div className="tt-nested-stack">
          {tags.map((t) => (
            <TagTooltipCard key={t} tagId={t} />
          ))}
        </div>
      )}
    </div>
  );
}

export function TraitTooltipCard({
  traitId,
  nested = false,
}: {
  traitId: string;
  nested?: boolean;
}) {
  const tr = TRAITS[traitId];
  if (!tr) return null;
  return (
    <div className={nested ? 'tt-subcard' : 'tt-card'}>
      <div className="tt-head-row">
        <strong style={{ color: tr.isUnique ? '#fbbf24' : '#38bdf8' }}>{tr.name}</strong>
        <span className="tt-badge">{tr.isUnique ? '独特词条' : '可传承词条'}</span>
      </div>
      <div className="tt-body">{tr.summary}</div>
      <div className="tt-flavor">{tr.flavor}</div>
      {(tr.bonusSkillId || tr.startTags || tr.onHitApplyTag || tr.damageVsTagBonusPct) && (
        <div className="tt-nested-stack">
          {tr.bonusSkillId && <SkillTooltipCard skillId={tr.bonusSkillId} nested />}
          {tr.startTags?.map((t) => (
            <TagTooltipCard key={t.tag} tagId={t.tag} />
          ))}
          {tr.onHitApplyTag && <TagTooltipCard tagId={tr.onHitApplyTag.tag} />}
          {tr.damageVsTagBonusPct && <TagTooltipCard tagId={tr.damageVsTagBonusPct.tag} />}
        </div>
      )}
    </div>
  );
}

export function EquipmentTooltipCard({ eq }: { eq: EquipmentInstance }) {
  const tpl = EQUIPMENT_TEMPLATES[eq.templateId];
  if (!tpl) return null;
  const scale = 1 + eq.bonusRollPct / 100;
  const stats = tpl.baseStats;
  const parts: string[] = [];
  if (stats.hp) parts.push(`HP +${Math.round(stats.hp * scale)}`);
  if (stats.atk) parts.push(`ATK +${Math.round(stats.atk * scale)}`);
  if (stats.def) parts.push(`DEF +${Math.round(stats.def * scale)}`);
  if (stats.spd) parts.push(`SPD +${Math.round(stats.spd * scale)}`);
  if (stats.ap) parts.push(`AP +${stats.ap}`);
  if (stats.pp) parts.push(`PP +${stats.pp}`);
  if (stats.str) parts.push(`STR +${stats.str}`);
  if (stats.dex) parts.push(`DEX +${stats.dex}`);
  if (stats.int) parts.push(`INT +${stats.int}`);
  if (stats.pow) parts.push(`POW +${stats.pow}`);
  if (stats.luk) parts.push(`LUK +${stats.luk}`);

  const slotName = tpl.slot === 'weapon' ? '武器' : tpl.slot === 'armor' ? '护甲' : '插件';
  const rarityName =
    eq.rarity === 'rare' ? '异界遗物' : eq.rarity === 'uncommon' ? '特制装备' : '常规装备';

  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong className={`rarity-${eq.rarity}`}>
          {tpl.name} (+{eq.bonusRollPct}%)
        </strong>
        <span className="tt-badge">
          {rarityName} · {slotName}
        </span>
      </div>
      <div className="tt-body mono">{parts.join('  ') || '基础组件'}</div>
      <div className="tt-meta-row">
        <span>
          分解可得：废材+{tpl.salvageYield.materials} 零件+{tpl.salvageYield.parts} 原晶+
          {tpl.salvageYield.crystals}
        </span>
      </div>
      <div className="tt-flavor">{tpl.flavor}</div>
      {(tpl.grantedSkillId || tpl.startTags) && (
        <div className="tt-nested-stack">
          {tpl.grantedSkillId && <SkillTooltipCard skillId={tpl.grantedSkillId} nested />}
          {tpl.startTags?.map((t) => (
            <TagTooltipCard key={t.tag} tagId={t.tag} />
          ))}
        </div>
      )}
    </div>
  );
}

export function SkillTreeNodeTooltipCard({
  node,
  stateLabel,
  prereqNames,
}: {
  node: SkillTreeNodeDef;
  stateLabel: 'unlocked' | 'available' | 'no_sp' | 'locked';
  prereqNames: string[];
}) {
  const statusText =
    stateLabel === 'unlocked'
      ? '✓ 已点亮'
      : stateLabel === 'available'
        ? `可学习（点击消耗 ${node.costSp} SP）`
        : stateLabel === 'no_sp'
          ? `SP 不足（需要 ${node.costSp} SP）`
          : `未解锁（需前置：${prereqNames.join(' + ')}）`;

  const tactic = node.grantTacticId ? TACTIC_BLUEPRINTS[node.grantTacticId] : null;

  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong style={{ color: node.isKeystone ? '#fbbf24' : '#38bdf8' }}>
          {node.icon} {node.name}
        </strong>
        <span className="tt-badge">
          {node.isKeystone ? '基石节点' : `T${node.tier}`} · {node.costSp} SP
        </span>
      </div>
      <div
        className="tt-meta"
        style={{
          color:
            stateLabel === 'unlocked'
              ? '#34d399'
              : stateLabel === 'available'
                ? '#fbbf24'
                : '#f87171',
        }}
      >
        {statusText}
      </div>
      <div className="tt-body">{node.description}</div>
      {(node.grantSkillId || node.grantTraitId || tactic) && (
        <div className="tt-nested-stack">
          {node.grantSkillId && <SkillTooltipCard skillId={node.grantSkillId} nested />}
          {node.grantTraitId && <TraitTooltipCard traitId={node.grantTraitId} nested />}
          {tactic && (
            <div className="tt-subcard">
              <div className="tt-head-row">
                <strong style={{ color: '#38bdf8' }}>{tactic.name}</strong>
                <span className="tt-meta">解锁战术条件</span>
              </div>
              <div className="tt-body">{tactic.description}</div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

const COND_TEXT: Record<ConditionType, string> = {
  ALWAYS: '无条件直接执行。',
  SELF_HP_BELOW: '自身当前 HP 百分比低于阈值时触发。',
  SELF_HP_ABOVE: '自身当前 HP 百分比高于等于阈值时触发。',
  ALLY_LOWEST_HP_BELOW: '己方血量最低的存活队员 HP% 低于阈值时触发。',
  SQUAD_AVG_HP_BELOW: '己方全队存活队员平均 HP% 低于阈值时触发。',
  FRONT_ROW_ALIVE_BELOW: '己方前排存活人数少于阈值时触发。',
  ENEMY_HAS_TAG: '敌方存在带有指定异常标签的目标时触发。',
  SELF_HAS_TAG: '自身带有指定标签时触发。',
  ENEMY_COUNT_AT_LEAST: '敌方存活人数不少于阈值时触发。',
  WAVE_AT_LEAST: '当前战斗波次达到指定波数时触发。',
  ROUND_AT_LEAST: '当前回合数达到指定回合时触发。',
};

const TARGET_TEXT: Record<TargetSelector, string> = {
  DEFAULT: '默认按站位优先打最前排 1 号位。',
  FRONT_LOWEST_HP: '优先集火前排血量最少的敌人。',
  BACK_PRIORITY: '远程无视前排，优先打击敌方后排脆皮。',
  HIGHEST_ATK: '优先锁定敌方攻击力最高的高危单位。',
  ENEMY_WITH_TAG: '只瞄准当前带有前置条件标签的敌人。',
  LOWEST_HP_ALLY: '瞄准己方当前血量百分比最低的队友。',
  SELF: '以自身为目标（或用于撤退条件判定）。',
};

export function TacticRuleTooltipCard({ rule }: { rule: GambitRule }) {
  const actionSummary =
    rule.actionType === 'RETREAT_PLANNED'
      ? '波间计划收队：清完当前波次后按 100% 无损系数提前返航，享完整掉率。'
      : rule.actionType === 'RETREAT_EMERGENCY'
        ? '交战紧急撤退：战斗中立即脱离战场，保住 60% 基础物资与 0.5x 掉率（首回合逃跑收益大幅折损）。'
        : null;

  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong>战术优先级规则详解</strong>
        <span className="tt-badge">{rule.enabled ? '启用中' : '已禁用'}</span>
      </div>
      <div className="tt-body">
        <div>• 条件1：{COND_TEXT[rule.condition1.type]}</div>
        {rule.condition2 && <div>• 条件2：{COND_TEXT[rule.condition2.type]}</div>}
        <div>• 目标选择：{TARGET_TEXT[rule.target]}</div>
      </div>
      {actionSummary && <div className="tt-subcard tt-body">{actionSummary}</div>}
      {rule.actionType === 'SKILL' && rule.skillId && (
        <div className="tt-nested-stack">
          <SkillTooltipCard skillId={rule.skillId} nested />
        </div>
      )}
    </div>
  );
}

export function ClassTooltipCard({
  classId,
  stars,
}: {
  classId: ClassId;
  stars: number;
}) {
  const cdef = CLASSES[classId];
  if (!cdef) return null;
  const masteryTrait = TRAITS[cdef.masteryTraitId];
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong>
          {cdef.icon} {cdef.name}
        </strong>
        <span className="tt-badge">熟练度 {stars}★ / 5★</span>
      </div>
      <div className="tt-meta-row">
        <span>适性：HP {cdef.aptitude.hpPct}% · ATK {cdef.aptitude.atkPct}%</span>
        <span>DEF {cdef.aptitude.defPct}% · SPD {cdef.aptitude.spdPct}%</span>
      </div>
      <div className="tt-body">{cdef.roleLabel}。每 1★ 熟练度为当前职业 +4%、全职业 +2% 属性。</div>
      <div className="tt-flavor">{cdef.flavor}</div>
      {masteryTrait && (
        <div className="tt-nested-stack">
          <TraitTooltipCard traitId={masteryTrait.id} nested />
        </div>
      )}
    </div>
  );
}

export function CoCStatTooltipCard({
  statKey,
  value,
  title,
  detail,
}: {
  statKey: CoCStatKey;
  value: number;
  title: string;
  detail: string;
}) {
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong>{title}</strong>
        <span className="tt-badge mono">
          {statKey.toUpperCase()} {value}
        </span>
      </div>
      <div className="tt-body">{detail}</div>
    </div>
  );
}

export function FacilityTooltipCard({
  plot,
  state,
}: {
  plot: BuildingPlot;
  state: GameState;
}) {
  if (!plot.facilityId) return null;
  const def = FACILITIES[plot.facilityId];
  if (!def) return null;
  const mult = getPlotEfficiencyMultiplier(state, plot);
  const cost = getFacilityUpgradeCost(def, plot.level);
  const stationed = plot.stationedOperativeId
    ? state.run.roster.find((o) => o.id === plot.stationedOperativeId)
    : null;
  const stationPct = stationed ? getStationingBonusPct(stationed, def, state.hq) : 0;

  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong>
          {def.name} Lv.{plot.level}/{def.maxLevel}
        </strong>
        <span className="tt-badge mono">总效率 ×{mult.toFixed(2)}</span>
      </div>
      <div className="tt-meta-row">
        <span>电力：{def.powerDeltaPerLevel > 0 ? `+${def.powerDeltaPerLevel}` : def.powerDeltaPerLevel} kW/级</span>
        <span>驻扎专精：{def.stationStats.map((s) => s.toUpperCase()).join(' + ')}</span>
      </div>
      <div className="tt-body">{def.effectSummary}</div>
      <div className="tt-subcard">
        <div className="tt-body">
          <div>• 地块：{plot.terrainLabel}</div>
          <div>
            • 驻扎：{stationed ? `${stationed.name} 提供 +${stationPct}%` : '无人值守（+0%）'}
          </div>
          {plot.level < def.maxLevel && (
            <div>
              • 下级消耗：废材 {cost.materials}
              {cost.parts > 0 ? ` · 零件 ${cost.parts}` : ''}
              {cost.crystals > 0 ? ` · 原晶 ${cost.crystals}` : ''}
            </div>
          )}
        </div>
      </div>
      <div className="tt-flavor">{def.flavor}</div>
    </div>
  );
}

export function EnemyTooltipCard({ enemy }: { enemy: EnemyUnitDef }) {
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong style={{ color: '#f87171' }}>{enemy.name}</strong>
        <span className="tt-badge">{enemy.row === 'front' ? '前排' : '后排'}</span>
      </div>
      <div className="tt-meta-row mono">
        <span>HP {enemy.hp}</span>
        <span>ATK {enemy.atk}</span>
        <span>DEF {enemy.def}</span>
        <span>SPD {enemy.spd}</span>
        <span>AP {enemy.ap}/PP {enemy.pp}</span>
      </div>
      {enemy.skillIds.length > 0 && (
        <div className="tt-nested-stack">
          {enemy.skillIds.map((sid) => (
            <SkillTooltipCard key={sid} skillId={sid} nested />
          ))}
        </div>
      )}
    </div>
  );
}

export function DistrictTooltipCard({ dist }: { dist: DistrictDef }) {
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong>{dist.name}</strong>
        <span className="tt-badge">{dist.waves.length} 波敌人</span>
      </div>
      <div className="tt-body">
        首通奖励：废材+{dist.firstClearReward.materials} 零件+{dist.firstClearReward.parts}
        {dist.firstClearReward.manuals > 0 ? ` 手札+${dist.firstClearReward.manuals}` : ''}
        <br />
        开放 2 块带加成的街区建设地块
        {dist.unlocksFacilityIds
          ? `；解锁 ${dist.unlocksFacilityIds.map((id) => FACILITIES[id]?.name ?? id).join('、')}`
          : ''}
      </div>
      <div className="tt-flavor">{dist.flavor}</div>
    </div>
  );
}

export function OtherworldTooltipCard({
  depth,
  cleared,
}: {
  depth: OtherworldDepthDef;
  cleared: boolean;
}) {
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong style={{ color: '#c084fc' }}>{depth.name}</strong>
        <span className="tt-badge">{depth.waves.length} 波 · 基准 {depth.baseTravelSec}s+</span>
      </div>
      <div className="tt-body mono">
        每轮保底：废材 {depth.baseYield.materials} · 零件 {depth.baseYield.parts} · 原晶{' '}
        {depth.baseYield.crystals} · 手札 {depth.baseYield.manuals}
      </div>
      <div className="tt-subcard tt-body">
        <strong>{cleared ? '稀有掉落池：' : '首通后开放巡逻掉落：'}</strong>
        <div style={{ marginTop: '3px', display: 'flex', flexWrap: 'wrap', gap: '4px' }}>
          {depth.lootTable.map((entry, idx) => {
            const label =
              entry.kind === 'equipment'
                ? EQUIPMENT_TEMPLATES[entry.refId]?.name
                : entry.kind === 'blueprint_facility'
                  ? `蓝图:${FACILITIES[entry.refId]?.name}`
                  : entry.kind === 'blueprint_tactic'
                    ? `战术:${TACTIC_BLUEPRINTS[entry.refId]?.name}`
                    : `SSR:${SPECIAL_OPERATIVES[entry.refId]?.name}`;
            return (
              <span key={`${entry.refId}_${idx}`} className="tt-badge">
                {label ?? entry.refId}
              </span>
            );
          })}
        </div>
      </div>
      <div className="tt-flavor">{depth.flavor}</div>
    </div>
  );
}

export function ModifierTooltipCard({ modifierId }: { modifierId: string }) {
  const mod = OTHERWORLD_MODIFIERS[modifierId];
  if (!mod) return null;
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong style={{ color: '#c084fc' }}>本轮环境：{mod.name}</strong>
        <span className="tt-badge">全局修正</span>
      </div>
      <div className="tt-body">{mod.ruleSummary}</div>
      <div className="tt-flavor">{mod.flavor}</div>
    </div>
  );
}

export function DepartmentTooltipCard({
  deptId,
  level,
}: {
  deptId: string;
  level: number;
}) {
  const def = HQ_DEPARTMENTS[deptId];
  if (!def) return null;
  const cost = getDepartmentUpgradeCost(def, level);
  return (
    <div className="tt-card">
      <div className="tt-head-row">
        <strong style={{ color: '#fbbf24' }}>{def.name}</strong>
        <span className="tt-badge">
          Lv.{level}/{def.maxLevel}
        </span>
      </div>
      <div className="tt-body">{def.effectSummary}</div>
      {level < def.maxLevel && (
        <div className="tt-meta-row">
          <span>下级需要组织经验：{cost}</span>
        </div>
      )}
      <div className="tt-flavor">{def.flavor}</div>
    </div>
  );
}
