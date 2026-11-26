import { OTHERWORLD_MODIFIERS } from '../../content/hqAndModifiers.ts';
import { SKILLS, TRAITS } from '../../content/skillsAndTraits.ts';
import { TAG_META } from '../../content/tags.ts';
import type {
  CoCStats,
  CombatLogEntry,
  CombatReport,
  CombatUnitInput,
  CombatUnitSummary,
  RetreatOutcome,
  SkillDef,
  TagId,
} from '../../types/combat.ts';
import {
  evaluateRuleConditions,
  getTagStacks,
  selectTarget,
  type LiveCombatUnit,
} from './conditions.ts';

export interface SimulateExpeditionInput {
  allies: CombatUnitInput[];
  enemiesByWave: CombatUnitInput[][];
  baseTravelSec: number;
  poweredFacilities: Record<string, number>;
  supportBonusPct?: Record<string, number>;
  modifierId: string;
}

const DEFAULT_COC: CoCStats = {
  str: 50,
  con: 50,
  siz: 50,
  dex: 50,
  app: 50,
  int: 50,
  pow: 50,
  edu: 50,
  luk: 50,
};

interface InternalUnit extends LiveCombatUnit {
  coc: CoCStats;
  damageDealt: number;
  damageTaken: number;
  healingShieldDone: number;
  ppReactions: number;
}

const MAX_ROUNDS_PER_WAVE = 15;

function cloneUnit(
  input: CombatUnitInput,
  hpMult = 1,
  atkMult = 1,
  extraTags?: Array<{ tag: TagId; stacks: number }>,
): InternalUnit {
  const maxHp = Math.max(1, Math.round(input.maxHp * hpMult));
  const atk = Math.max(1, Math.round(input.atk * atkMult));
  const tags: Partial<Record<TagId, number>> = {};

  for (const t of input.initialTags ?? []) {
    const cap = TAG_META[t.tag].maxStacks;
    tags[t.tag] = Math.min(cap, (tags[t.tag] ?? 0) + t.stacks);
  }
  for (const t of extraTags ?? []) {
    const cap = TAG_META[t.tag].maxStacks;
    tags[t.tag] = Math.min(cap, (tags[t.tag] ?? 0) + t.stacks);
  }

  return {
    id: input.id,
    name: input.name,
    side: input.side,
    row: input.row,
    slot: input.slot,
    hp: maxHp,
    maxHp,
    shield: tags.shielded ? Math.round(maxHp * 0.15) : 0,
    atk,
    def: input.def,
    spd: input.spd,
    ap: input.maxAp,
    maxAp: input.maxAp,
    pp: input.maxPp,
    maxPp: input.maxPp,
    alive: true,
    tags,
    traitIds: [...input.traitIds],
    gambits: input.gambits.map((g) => ({ ...g })),
    skillIds: [...input.skillIds],
    coc: input.coc ? { ...input.coc } : { ...DEFAULT_COC },
    damageDealt: 0,
    damageTaken: 0,
    healingShieldDone: 0,
    ppReactions: 0,
  };
}

function addTag(unit: LiveCombatUnit, tag: TagId, stacks: number): number {
  const cap = TAG_META[tag].maxStacks;
  const before = unit.tags[tag] ?? 0;
  const after = Math.min(cap, before + stacks);
  unit.tags[tag] = after;
  return after - before;
}

function findReachableEnemyCandidates(
  skill: SkillDef,
  enemies: InternalUnit[],
): InternalUnit[] {
  const living = enemies.filter((u) => u.alive);
  if (skill.range === 'melee') {
    const front = living.filter((u) => u.row === 'front');
    if (front.length > 0) return front;
  }
  return living;
}

function sortTurnOrder(units: InternalUnit[]): InternalUnit[] {
  return [...units]
    .filter((u) => u.alive)
    .sort((a, b) => {
      if (a.spd !== b.spd) return b.spd - a.spd;
      if (a.side !== b.side) return a.side === 'ally' ? -1 : 1;
      if (a.row !== b.row) return a.row === 'front' ? -1 : 1;
      return a.slot - b.slot;
    });
}

function computeFactors(
  outcome: RetreatOutcome,
  wavesCleared: number,
  totalWaves: number,
  totalRounds: number,
): { yieldFactor: number; dropRateFactor: number } {
  if (outcome === 'PLANNED') {
    if (wavesCleared >= totalWaves) {
      return { yieldFactor: 1.0, dropRateFactor: 1.0 };
    }
    const ratio = wavesCleared / Math.max(1, totalWaves);
    return {
      yieldFactor: Number((0.35 + 0.65 * ratio).toFixed(4)),
      dropRateFactor: Number(ratio.toFixed(4)),
    };
  }
  if (outcome === 'EMERGENCY') {
    if (wavesCleared >= 1) {
      return { yieldFactor: 0.6, dropRateFactor: 0.5 };
    }
    if (totalRounds >= 2) {
      return { yieldFactor: 0.45, dropRateFactor: 0.25 };
    }
    return { yieldFactor: 0.3, dropRateFactor: 0 };
  }
  return { yieldFactor: 0.3, dropRateFactor: 0 };
}

function buildUnitSummaries(allies: InternalUnit[]): CombatUnitSummary[] {
  return allies.map((u) => ({
    id: u.id,
    name: u.name,
    side: u.side,
    damageDealt: u.damageDealt,
    damageTaken: u.damageTaken,
    healingShieldDone: u.healingShieldDone,
    ppReactions: u.ppReactions,
    aliveAtEnd: u.alive,
    hpEndPct: Math.round((u.hp / Math.max(1, u.maxHp)) * 100),
  }));
}

export function simulateExpedition(input: SimulateExpeditionInput): CombatReport {
  const modifier = OTHERWORLD_MODIFIERS[input.modifierId];
  const enemyHpMult = modifier?.enemyHpMultiplier ?? 1;
  const enemyAtkMult = modifier?.enemyAtkMultiplier ?? 1;

  const relayLevel = input.poweredFacilities['tactical_relay'] ?? 0;
  const relayMult = 1 + (input.supportBonusPct?.['tactical_relay'] ?? 0) / 100;

  const cryoLevel = input.poweredFacilities['cryo_booster'] ?? 0;
  const cryoMult = 1 + (input.supportBonusPct?.['cryo_booster'] ?? 0) / 100;

  const analyzerLevel = input.poweredFacilities['erosion_analyzer'] ?? 0;
  const analyzerMult = 1 + (input.supportBonusPct?.['erosion_analyzer'] ?? 0) / 100;

  const entries: CombatLogEntry[] = [];
  const totalWaves = input.enemiesByWave.length;

  const allies: InternalUnit[] = input.allies.map((u) =>
    cloneUnit(u, 1, 1, modifier?.allyStartTags),
  );

  let wavesCleared = 0;
  let totalRounds = 0;
  let outcome: RetreatOutcome = 'PLANNED';
  let allyPpReactionCount = 0;
  let erosionBonusPct = 0;

  const pushLog = (entry: CombatLogEntry) => {
    entries.push(entry);
  };

  if (
    modifier &&
    (modifier.allyStartTags || modifier.enemyStartTags || enemyHpMult !== 1 || enemyAtkMult !== 1)
  ) {
    pushLog({
      wave: 1,
      round: 0,
      kind: 'MODIFIER',
      text: `本轮异界修正【${modifier.name}】生效：${modifier.ruleSummary}`,
    });
  }

  const getTeamSides = (unit: InternalUnit, enemies: InternalUnit[]) =>
    unit.side === 'ally'
      ? { own: allies, opp: enemies }
      : { own: enemies, opp: allies };

  const recordDeathIfNeeded = (victim: InternalUnit, wave: number, round: number) => {
    if (victim.hp > 0 || !victim.alive) return;
    victim.hp = 0;
    victim.alive = false;
    if (victim.side === 'enemy' && analyzerLevel > 0) {
      const stacks = getTagStacks(victim, 'erosion');
      if (stacks >= 3) {
        const gain = Math.round((6 + 3 * analyzerLevel) * analyzerMult);
        erosionBonusPct = Math.min(180, erosionBonusPct + gain);
        pushLog({
          wave,
          round,
          kind: 'FACILITY_SUPPORT',
          actorName: '异界解析阵列',
          text: `${victim.name} 带着 ${stacks} 层 [侵蚀] 崩解，解析阵列记录波纹（原晶收益 +${gain}%）。`,
          tags: ['erosion'],
        });
      }
    }
  };

  const applyDamage = (
    attacker: InternalUnit,
    defender: InternalUnit,
    skill: SkillDef,
    wave: number,
    round: number,
    isPassive: boolean,
  ): { dealt: number; bonusCryo: number } => {
    const overclockAttacker = getTagStacks(attacker, 'overclock');
    const shockedTarget = getTagStacks(defender, 'shocked');
    const overclockTarget = getTagStacks(defender, 'overclock');
    const fortifiedTarget = getTagStacks(defender, 'fortified');
    const erosionTarget = getTagStacks(defender, 'erosion');

    // CoC attribute scaling based on skill scaleStat
    const statKey = skill.scaleStat ?? (skill.range === 'ranged' ? 'dex' : 'str');
    const statVal = attacker.coc[statKey] ?? 50;
    const cocBonusMult = 1 + Math.max(0, (statVal - 40) * 0.005);

    let atkFactor = skill.powerMultiplier * cocBonusMult;
    atkFactor *= 1 + overclockAttacker * 0.15;

    for (const tid of attacker.traitIds) {
      const trait = TRAITS[tid];
      if (trait?.damageVsTagBonusPct) {
        if (getTagStacks(defender, trait.damageVsTagBonusPct.tag) > 0) {
          atkFactor *= 1 + trait.damageVsTagBonusPct.bonusPct / 100;
        }
      }
    }

    if (skill.consumeEnemyTagBonus) {
      const tag = skill.consumeEnemyTagBonus.tag;
      const stacks = getTagStacks(defender, tag);
      if (stacks > 0) {
        atkFactor *= 1 + (stacks * skill.consumeEnemyTagBonus.bonusPerStackPct) / 100;
        if (skill.consumeEnemyTagBonus.consume) {
          defender.tags[tag] = 0;
        }
      }
    }

    const raw = attacker.atk * atkFactor + (skill.flatBonus ?? 0);
    const mitigation = 100 / (100 + defender.def * 2.2);
    let dmg = raw * mitigation;

    let dmgMult = 1 + shockedTarget * 0.1 + overclockTarget * 0.08;
    if (fortifiedTarget > 0) dmgMult *= 0.75;
    if (defender.side === 'enemy' && analyzerLevel > 0 && erosionTarget > 0) {
      dmgMult *= 1 + erosionTarget * (((4 + 2 * analyzerLevel) * analyzerMult) / 100);
    }

    let finalDmg = Math.max(1, Math.round(dmg * dmgMult));

    let bonusCryo = 0;
    if (!isPassive && attacker.side === 'ally' && cryoLevel > 0 && overclockAttacker > 0) {
      bonusCryo = Math.round((8 + 5 * cryoLevel) * cryoMult);
      finalDmg += bonusCryo;
      const healAmt = Math.max(2, Math.round(attacker.maxHp * 0.05));
      attacker.hp = Math.min(attacker.maxHp, attacker.hp + healAmt);
      attacker.healingShieldDone += healAmt;
      pushLog({
        wave,
        round,
        kind: 'FACILITY_SUPPORT',
        actorName: '超频冷凝站',
        text: `为处于 [过载] 的 ${attacker.name} 注入冷媒：附加 ${bonusCryo} 真实伤害并恢复 ${healAmt} 生命。`,
        tags: ['overclock'],
      });
    }

    let remaining = finalDmg;
    if (defender.shield > 0) {
      const absorbed = Math.min(defender.shield, remaining);
      defender.shield -= absorbed;
      remaining -= absorbed;
      if (defender.shield === 0) {
        defender.tags.shielded = 0;
      }
    }
    defender.hp = Math.max(0, defender.hp - remaining);
    attacker.damageDealt += finalDmg;
    defender.damageTaken += remaining;
    recordDeathIfNeeded(defender, wave, round);
    return { dealt: finalDmg, bonusCryo };
  };

  const checkTacticalRelay = (wave: number, round: number) => {
    if (relayLevel <= 0) return;
    if (allyPpReactionCount > 0 && allyPpReactionCount % 3 === 0) {
      const shieldPct = ((12 + 4 * relayLevel) * relayMult) / 100;
      const livingFront = allies.filter((u) => u.alive && u.row === 'front');
      const shieldTargets = livingFront.length > 0 ? livingFront : allies.filter((u) => u.alive);
      for (const ally of shieldTargets) {
        const gain = Math.max(4, Math.round(ally.maxHp * shieldPct));
        ally.shield += gain;
        ally.healingShieldDone += gain;
        addTag(ally, 'shielded', 1);
      }
      const needPp = allies
        .filter((u) => u.alive && u.pp < u.maxPp)
        .sort((a, b) => a.pp - b.pp || a.slot - b.slot)[0];
      let ppText = '';
      if (needPp) {
        needPp.pp = Math.min(needPp.maxPp, needPp.pp + 1);
        ppText = `，并为 ${needPp.name} 回复 +1 PP`;
      }
      pushLog({
        wave,
        round,
        kind: 'FACILITY_SUPPORT',
        actorName: '战术谐振电台',
        text: `累计 ${allyPpReactionCount} 次 PP 协同触发谐振！前排获得 ${Math.round(shieldPct * 100)}% [护盾]${ppText}。`,
        tags: ['shielded'],
      });
    }
  };

  const tryTriggerPassives = (
    trigger: 'ON_ALLY_TARGETED' | 'ON_ALLY_DEAL_DAMAGE' | 'ON_TAG_APPLIED' | 'ON_ALLY_LOW_HP',
    side: 'ally' | 'enemy',
    enemies: InternalUnit[],
    wave: number,
    round: number,
    reactedThisAction: Set<string>,
    context: {
      target?: InternalUnit;
      attacker?: InternalUnit;
      tagApplied?: TagId;
    },
  ): InternalUnit | null => {
    const team = side === 'ally' ? allies : enemies;
    const opp = side === 'ally' ? enemies : allies;
    const orderedTeam = [...team].sort((a, b) =>
      a.row !== b.row ? (a.row === 'front' ? -1 : 1) : a.slot - b.slot,
    );

    for (const unit of orderedTeam) {
      if (!unit.alive || unit.pp <= 0 || reactedThisAction.has(unit.id)) continue;

      for (const rule of unit.gambits) {
        if (!rule.enabled || rule.actionType !== 'SKILL' || !rule.skillId) continue;
        const skill = SKILLS[rule.skillId];
        if (!skill || skill.kind !== 'passive' || skill.trigger !== trigger) continue;
        if (!unit.skillIds.includes(skill.id) || unit.pp < skill.ppCost) continue;
        if (skill.requiredSelfTag && getTagStacks(unit, skill.requiredSelfTag) <= 0) continue;

        if (trigger === 'ON_ALLY_TARGETED') {
          if (!context.target || context.target.id === unit.id) continue;
          const ok = evaluateRuleConditions(rule, {
            actor: unit,
            allies: team,
            enemies: opp,
            wave,
            round,
          });
          if (!ok) continue;

          unit.pp -= skill.ppCost;
          unit.ppReactions += 1;
          reactedThisAction.add(unit.id);
          if (side === 'ally') allyPpReactionCount++;

          const appliedTags: TagId[] = [];
          for (const st of skill.applySelfTags ?? []) {
            addTag(unit, st.tag, st.stacks);
            appliedTags.push(st.tag);
          }
          if (context.attacker) {
            for (const at of skill.applyAttackerTags ?? []) {
              addTag(context.attacker, at.tag, at.stacks);
              appliedTags.push(at.tag);
            }
          }
          pushLog({
            wave,
            round,
            kind: 'PASSIVE_PP',
            actorName: unit.name,
            text: `↳ [PP 掩护] ${unit.name} 消耗 1 PP 发动「${skill.name}」，替 ${context.target.name} 挡下攻击并反震 ${context.attacker?.name ?? ''}。`,
            tags: appliedTags,
          });
          if (side === 'ally') checkTacticalRelay(wave, round);
          return unit;
        }

        if (trigger === 'ON_ALLY_DEAL_DAMAGE' || trigger === 'ON_TAG_APPLIED') {
          const enemyTarget = context.target;
          if (!enemyTarget || !enemyTarget.alive) continue;
          if (
            skill.requiredEnemyTag &&
            getTagStacks(enemyTarget, skill.requiredEnemyTag) <= 0
          ) {
            continue;
          }
          if (trigger === 'ON_TAG_APPLIED' && skill.requiredEnemyTag) {
            if (context.tagApplied !== skill.requiredEnemyTag) continue;
          }

          const ok = evaluateRuleConditions(rule, {
            actor: unit,
            allies: team,
            enemies: opp,
            wave,
            round,
            preferredTag: skill.requiredEnemyTag ?? skill.requiredSelfTag,
          });
          if (!ok) continue;

          unit.pp -= skill.ppCost;
          unit.ppReactions += 1;
          reactedThisAction.add(unit.id);
          if (side === 'ally') allyPpReactionCount++;

          const { dealt } = applyDamage(unit, enemyTarget, skill, wave, round, true);
          pushLog({
            wave,
            round,
            kind: 'PASSIVE_PP',
            actorName: unit.name,
            text: `↳ [PP 追击] ${unit.name} 触发「${skill.name}」补击 ${enemyTarget.name}，造成 ${dealt} 伤害${!enemyTarget.alive ? '（击倒）' : ''}。`,
            tags: skill.requiredEnemyTag ? [skill.requiredEnemyTag] : undefined,
          });
          if (side === 'ally') checkTacticalRelay(wave, round);
          return unit;
        }

        if (trigger === 'ON_ALLY_LOW_HP') {
          const hurtAlly = context.target;
          if (!hurtAlly || !hurtAlly.alive) continue;
          const ok = evaluateRuleConditions(rule, {
            actor: unit,
            allies: team,
            enemies: opp,
            wave,
            round,
          });
          if (!ok) continue;

          unit.pp -= skill.ppCost;
          unit.ppReactions += 1;
          reactedThisAction.add(unit.id);
          if (side === 'ally') allyPpReactionCount++;

          const eduScale = 1 + Math.max(0, (unit.coc.edu - 40) * 0.005);
          const heal = skill.healRatio ? Math.round(unit.atk * skill.healRatio * eduScale) : 0;
          const shield = skill.shieldRatio ? Math.round(hurtAlly.maxHp * skill.shieldRatio) : 0;
          if (heal > 0) {
            hurtAlly.hp = Math.min(hurtAlly.maxHp, hurtAlly.hp + heal);
            unit.healingShieldDone += heal;
          }
          if (shield > 0) {
            hurtAlly.shield += shield;
            unit.healingShieldDone += shield;
            addTag(hurtAlly, 'shielded', 1);
          }
          pushLog({
            wave,
            round,
            kind: 'PASSIVE_PP',
            actorName: unit.name,
            text: `↳ [PP 急救] ${unit.name} 触发「${skill.name}」，为 ${hurtAlly.name} 恢复 ${heal} 生命并附加 ${shield} 护盾。`,
            tags: ['shielded'],
          });
          if (side === 'ally') checkTacticalRelay(wave, round);
          return unit;
        }
      }
    }
    return null;
  };

  for (let wIdx = 0; wIdx < totalWaves; wIdx++) {
    const waveNum = wIdx + 1;
    const waveDef = input.enemiesByWave[wIdx] ?? [];
    const enemies: InternalUnit[] = waveDef.map((e) =>
      cloneUnit(e, enemyHpMult, enemyAtkMult, modifier?.enemyStartTags),
    );

    pushLog({
      wave: waveNum,
      round: 0,
      kind: 'WAVE_START',
      text: `第 ${waveNum}/${totalWaves} 波接敌：${enemies.map((e) => `${e.name}(${e.row === 'front' ? '前' : '后'})`).join('、')}`,
    });

    let waveOver = false;
    for (let r = 1; r <= MAX_ROUNDS_PER_WAVE && !waveOver; r++) {
      totalRounds++;
      const allUnits = [...allies, ...enemies];
      for (const u of allUnits) {
        if (!u.alive) continue;
        u.ap = u.maxAp;
        u.pp = u.maxPp;
      }

      pushLog({
        wave: waveNum,
        round: r,
        kind: 'ROUND_START',
        text: `—— 第 ${waveNum} 波 · 回合 ${r}（AP/PP 刷新）——`,
      });

      for (const u of allUnits) {
        if (!u.alive) continue;
        const stacks = getTagStacks(u, 'erosion');
        if (stacks > 0) {
          const dot = Math.max(2, Math.round(u.maxHp * 0.04 * stacks));
          let rem = dot;
          if (u.shield > 0) {
            const abs = Math.min(u.shield, rem);
            u.shield -= abs;
            rem -= abs;
          }
          u.hp = Math.max(0, u.hp - rem);
          u.damageTaken += rem;
          recordDeathIfNeeded(u, waveNum, r);
          pushLog({
            wave: waveNum,
            round: r,
            kind: 'MODIFIER',
            actorName: u.name,
            text: `${u.name} 受到 ${stacks} 层 [侵蚀] 持续伤害 ${dot}${!u.alive ? '（阵亡）' : ''}。`,
            tags: ['erosion'],
          });
        }
      }

      if (enemies.every((e) => !e.alive)) {
        wavesCleared++;
        waveOver = true;
        break;
      }
      if (allies.every((a) => !a.alive)) {
        outcome = 'WIPE';
        waveOver = true;
        break;
      }

      const turnQueue = sortTurnOrder(allUnits);
      for (const actor of turnQueue) {
        if (!actor.alive) continue;
        if (enemies.every((e) => !e.alive) || allies.every((a) => !a.alive)) break;

        let safety = 0;
        while (actor.alive && actor.ap > 0 && safety < 6) {
          safety++;
          if (enemies.every((e) => !e.alive) || allies.every((a) => !a.alive)) break;

          const { own, opp } = getTeamSides(actor, enemies);
          let acted = false;

          for (const rule of actor.gambits) {
            if (!rule.enabled) continue;

            if (rule.actionType === 'RETREAT_PLANNED') continue;

            if (rule.actionType === 'RETREAT_EMERGENCY') {
              if (actor.side !== 'ally') continue;
              const matched = evaluateRuleConditions(rule, {
                actor,
                allies: own,
                enemies: opp,
                wave: waveNum,
                round: r,
              });
              if (matched) {
                outcome = 'EMERGENCY';
                pushLog({
                  wave: waveNum,
                  round: r,
                  kind: 'RETREAT',
                  actorName: actor.name,
                  text: `⚠ ${actor.name} 触发战术指令【交战紧急撤退】，小队立刻脱战收拢物资！`,
                });
                const factors = computeFactors(outcome, wavesCleared, totalWaves, totalRounds);
                return {
                  victory: wavesCleared > 0,
                  fullClear: false,
                  outcome,
                  wavesCleared,
                  totalWaves,
                  totalRounds,
                  durationSec: Math.round((input.baseTravelSec + totalRounds * 1.5) * 10) / 10,
                  yieldFactor: factors.yieldFactor,
                  dropRateFactor: factors.dropRateFactor,
                  erosionBonusPct,
                  entries,
                  unitStats: buildUnitSummaries(allies),
                };
              }
              continue;
            }

            if (rule.actionType === 'SKILL' && rule.skillId) {
              const skill = SKILLS[rule.skillId];
              if (!skill || skill.kind !== 'active') continue;
              if (!actor.skillIds.includes(skill.id) || actor.ap < skill.apCost) continue;

              const preferredTag =
                rule.condition1.tag ??
                rule.condition2?.tag ??
                skill.consumeEnemyTagBonus?.tag;

              const condOk = evaluateRuleConditions(rule, {
                actor,
                allies: own,
                enemies: opp,
                wave: waveNum,
                round: r,
                preferredTag,
              });
              if (!condOk) continue;

              const candidates = skill.targetAlly
                ? own
                : findReachableEnemyCandidates(skill, opp);
              const initialTarget = selectTarget(
                rule.target,
                candidates,
                actor,
                preferredTag,
              );
              if (!initialTarget) continue;

              actor.ap -= skill.apCost;
              acted = true;
              const reactedThisAction = new Set<string>();

              if (skill.selfDamagePctMaxHp && skill.selfDamagePctMaxHp > 0) {
                const selfCost = Math.max(1, Math.round(actor.maxHp * skill.selfDamagePctMaxHp));
                actor.hp = Math.max(1, actor.hp - selfCost);
              }
              for (const st of skill.applySelfTags ?? []) {
                addTag(actor, st.tag, st.stacks);
              }

              if (skill.targetAlly) {
                const statKey = skill.scaleStat ?? 'edu';
                const supportScale = 1 + Math.max(0, (actor.coc[statKey] - 40) * 0.005);
                const heal = skill.healRatio
                  ? Math.round(actor.atk * skill.healRatio * supportScale)
                  : 0;
                const shield = skill.shieldRatio
                  ? Math.round(initialTarget.maxHp * skill.shieldRatio * supportScale)
                  : 0;
                if (heal > 0) {
                  initialTarget.hp = Math.min(initialTarget.maxHp, initialTarget.hp + heal);
                  actor.healingShieldDone += heal;
                }
                if (shield > 0) {
                  initialTarget.shield += shield;
                  actor.healingShieldDone += shield;
                  addTag(initialTarget, 'shielded', 1);
                }
                const appliedSupportTags: TagId[] = shield > 0 ? ['shielded'] : [];
                for (const tg of skill.applyTargetTags ?? []) {
                  addTag(initialTarget, tg.tag, tg.stacks);
                  appliedSupportTags.push(tg.tag);
                }
                pushLog({
                  wave: waveNum,
                  round: r,
                  kind: 'ACTIVE_AP',
                  actorName: actor.name,
                  text: `[AP] ${actor.name} 对 ${initialTarget.name} 施放「${skill.name}」（治疗 ${heal}，护盾 ${shield}）。`,
                  tags: appliedSupportTags.length > 0 ? appliedSupportTags : undefined,
                });
                break;
              }

              const defenderSide = initialTarget.side;
              const guardUnit = tryTriggerPassives(
                'ON_ALLY_TARGETED',
                defenderSide,
                enemies,
                waveNum,
                r,
                reactedThisAction,
                { target: initialTarget, attacker: actor },
              );
              const actualTarget = guardUnit && guardUnit.alive ? guardUnit : initialTarget;

              const { dealt } = applyDamage(actor, actualTarget, skill, waveNum, r, false);
              const appliedTags: TagId[] = [];

              for (const tg of skill.applyTargetTags ?? []) {
                if (actualTarget.alive) {
                  addTag(actualTarget, tg.tag, tg.stacks);
                  appliedTags.push(tg.tag);
                }
              }

              for (const tid of actor.traitIds) {
                const trait = TRAITS[tid];
                if (trait?.onHitApplyTag && actualTarget.alive) {
                  addTag(actualTarget, trait.onHitApplyTag.tag, trait.onHitApplyTag.stacks);
                  appliedTags.push(trait.onHitApplyTag.tag);
                }
              }

              if (skill.spreadTag && getTagStacks(actualTarget, skill.spreadTag) > 0) {
                for (const other of opp) {
                  if (other.alive && other.id !== actualTarget.id) {
                    addTag(other, skill.spreadTag, 1);
                  }
                }
                appliedTags.push(skill.spreadTag);
              }

              pushLog({
                wave: waveNum,
                round: r,
                kind: 'ACTIVE_AP',
                actorName: actor.name,
                text: `[AP] ${actor.name} 对 ${actualTarget.name} 使用「${skill.name}」，造成 ${dealt} 伤害${!actualTarget.alive ? '（击倒）' : ''}。`,
                tags: appliedTags.length > 0 ? appliedTags : undefined,
              });

              if (actualTarget.alive) {
                for (const tg of appliedTags) {
                  tryTriggerPassives(
                    'ON_TAG_APPLIED',
                    actor.side,
                    enemies,
                    waveNum,
                    r,
                    reactedThisAction,
                    { target: actualTarget, attacker: actor, tagApplied: tg },
                  );
                }
                if (actualTarget.alive) {
                  tryTriggerPassives(
                    'ON_ALLY_DEAL_DAMAGE',
                    actor.side,
                    enemies,
                    waveNum,
                    r,
                    reactedThisAction,
                    { target: actualTarget, attacker: actor },
                  );
                }
                if (
                  actualTarget.alive &&
                  actualTarget.hp / Math.max(1, actualTarget.maxHp) < 0.45
                ) {
                  tryTriggerPassives(
                    'ON_ALLY_LOW_HP',
                    actualTarget.side,
                    enemies,
                    waveNum,
                    r,
                    reactedThisAction,
                    { target: actualTarget },
                  );
                }
              }

              break;
            }
          }

          if (!acted) break;
        }
      }

      if (enemies.every((e) => !e.alive)) {
        wavesCleared++;
        waveOver = true;
      } else if (allies.every((a) => !a.alive)) {
        outcome = 'WIPE';
        waveOver = true;
      }
    }

    if (!waveOver) outcome = 'WIPE';

    if (outcome === 'WIPE') {
      pushLog({
        wave: waveNum,
        round: totalRounds,
        kind: 'DEFEAT',
        text: '✖ 战斗全灭或超时失利，小队丢弃重装备抢救 30% 基础物资撤回。',
      });
      const factors = computeFactors('WIPE', wavesCleared, totalWaves, totalRounds);
      return {
        victory: false,
        fullClear: false,
        outcome: 'WIPE',
        wavesCleared,
        totalWaves,
        totalRounds,
        durationSec: Math.round((input.baseTravelSec + totalRounds * 1.5 + 10) * 10) / 10,
        yieldFactor: factors.yieldFactor,
        dropRateFactor: factors.dropRateFactor,
        erosionBonusPct,
        entries,
        unitStats: buildUnitSummaries(allies),
      };
    }

    if (wIdx < totalWaves - 1) {
      for (const ally of allies) {
        if (!ally.alive) continue;
        for (const rule of ally.gambits) {
          if (!rule.enabled || rule.actionType !== 'RETREAT_PLANNED') continue;
          const ok = evaluateRuleConditions(rule, {
            actor: ally,
            allies,
            enemies: [],
            wave: waveNum,
            round: totalRounds,
          });
          if (ok) {
            outcome = 'PLANNED';
            pushLog({
              wave: waveNum,
              round: totalRounds,
              kind: 'RETREAT',
              actorName: ally.name,
              text: `⚑ 第 ${waveNum} 波清空后，${ally.name} 触发【波间计划收队】，小队按 100% 无损比例带回本段收益与完整掉率。`,
            });
            const factors = computeFactors('PLANNED', wavesCleared, totalWaves, totalRounds);
            return {
              victory: true,
              fullClear: false,
              outcome: 'PLANNED',
              wavesCleared,
              totalWaves,
              totalRounds,
              durationSec: Math.round((input.baseTravelSec + totalRounds * 1.5) * 10) / 10,
              yieldFactor: factors.yieldFactor,
              dropRateFactor: factors.dropRateFactor,
              erosionBonusPct,
              entries,
              unitStats: buildUnitSummaries(allies),
            };
          }
        }
      }
    }
  }

  pushLog({
    wave: totalWaves,
    round: totalRounds,
    kind: 'VICTORY',
    text: `★ 全波次肃清凯旋！获得 100% 基础物资${erosionBonusPct > 0 ? `（解析阵列额外提升原晶 +${erosionBonusPct}%）` : ''}与完整稀有掉率。`,
  });

  const factors = computeFactors('PLANNED', wavesCleared, totalWaves, totalRounds);
  return {
    victory: true,
    fullClear: true,
    outcome: 'PLANNED',
    wavesCleared,
    totalWaves,
    totalRounds,
    durationSec: Math.round((input.baseTravelSec + totalRounds * 1.5) * 10) / 10,
    yieldFactor: factors.yieldFactor,
    dropRateFactor: factors.dropRateFactor,
    erosionBonusPct,
    entries,
    unitStats: buildUnitSummaries(allies),
  };
}
