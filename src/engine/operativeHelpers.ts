import { CLASSES, SPECIAL_OPERATIVES } from '../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../content/equipment.ts';
import { FACILITIES } from '../content/facilities.ts';
import { TRAITS } from '../content/skillsAndTraits.ts';
import { DISTRICTS } from '../content/worldNodes.ts';
import type {
  ClassId,
  CoCStats,
  CombatUnitInput,
  GambitRule,
  OperativeRarity,
  RowType,
  SlotIndex,
  TagId,
} from '../types/combat.ts';
import type {
  FacilityDef,
  GameState,
  OperativeInstance,
  PermanentHQState,
  SkillTreeNodeDef,
} from '../types/game.ts';

export interface EffectiveStats {
  maxHp: number;
  atk: number;
  def: number;
  spd: number;
  maxAp: number;
  maxPp: number;
  gambitSlotCap: number;
  evilitySlotCap: number;
  coc: CoCStats;
  skillIds: string[];
  startTags: Array<{ tag: TagId; stacks: number }>;
  activeTraitIds: string[];
  totalMasteryStars: number;
}

const SLOTS = ['weapon', 'armor', 'module'] as const;

export function getClassMasteryStars(xp: number): number {
  return Math.min(5, Math.floor(Math.max(0, xp) / 100));
}

export function getTotalMasteryStars(op: OperativeInstance): number {
  let total = 0;
  for (const val of Object.values(op.classMastery)) {
    total += getClassMasteryStars(val ?? 0);
  }
  return total;
}

export function getEvilitySlotCap(rarity: OperativeRarity): number {
  switch (rarity) {
    case 'N':
      return 1;
    case 'R':
      return 2;
    case 'SR':
    case 'SSR':
      return 3;
  }
}

export function getOperativeTreeNodes(op: OperativeInstance): {
  classNodes: SkillTreeNodeDef[];
  personalNodes: SkillTreeNodeDef[];
  allNodes: SkillTreeNodeDef[];
} {
  const classNodes = CLASSES[op.classId]?.treeNodes ?? [];
  const personalNodes = SPECIAL_OPERATIVES[op.archetypeId]?.personalTreeNodes ?? [];
  return {
    classNodes,
    personalNodes,
    allNodes: [...personalNodes, ...classNodes],
  };
}

export function getOperativeEffectiveStats(
  op: OperativeInstance,
  hq: PermanentHQState,
): EffectiveStats {
  const levelOffset = Math.max(0, op.level - 1);
  const trainingLvl = hq.departments['dept_training'] ?? 0;
  const classDef = CLASSES[op.classId];
  const totalStars = getTotalMasteryStars(op);
  const currentClassStars = getClassMasteryStars(op.classMastery[op.classId] ?? 0);

  const coc: CoCStats = { ...op.coc };
  let flatHp = 0;
  let flatAtk = 0;
  let flatDef = 0;
  let flatSpd = 0;
  let ap = op.baseStats.ap;
  let pp = op.baseStats.pp;
  let gambitBonus = 0;

  const skillSet = new Set<string>([
    ...(SPECIAL_OPERATIVES[op.archetypeId]?.innateSkillIds ?? []),
    ...(classDef?.starterSkillIds ?? []),
    ...op.learnedSkillIds,
  ]);
  const startTags: Array<{ tag: TagId; stacks: number }> = [];
  const activeTraitIds = [...op.innateTraitIds, ...op.equippedTraitIds];

  // Apply traits (innate + equipped evilities)
  for (const traitId of activeTraitIds) {
    const trait = TRAITS[traitId];
    if (!trait) continue;
    flatHp += trait.statBonus.hp ?? 0;
    flatAtk += trait.statBonus.atk ?? 0;
    flatDef += trait.statBonus.def ?? 0;
    flatSpd += trait.statBonus.spd ?? 0;
    ap += trait.statBonus.ap ?? 0;
    pp += trait.statBonus.pp ?? 0;
    coc.str += trait.statBonus.str ?? 0;
    coc.con += trait.statBonus.con ?? 0;
    coc.siz += trait.statBonus.siz ?? 0;
    coc.dex += trait.statBonus.dex ?? 0;
    coc.app += trait.statBonus.app ?? 0;
    coc.int += trait.statBonus.int ?? 0;
    coc.pow += trait.statBonus.pow ?? 0;
    coc.edu += trait.statBonus.edu ?? 0;
    coc.luk += trait.statBonus.luk ?? 0;
    if (trait.bonusSkillId) skillSet.add(trait.bonusSkillId);
    if (trait.startTags) startTags.push(...trait.startTags);
  }

  // Apply unlocked skill tree nodes (personal + current class)
  const { allNodes } = getOperativeTreeNodes(op);
  const nodeMap = new Map(allNodes.map((n) => [n.id, n]));
  for (const nodeId of op.unlockedNodeIds) {
    const node = nodeMap.get(nodeId);
    if (!node) continue;
    if (node.grantSkillId) skillSet.add(node.grantSkillId);
    const sb = node.statBonus;
    if (!sb) continue;
    flatHp += sb.hp ?? 0;
    flatAtk += sb.atk ?? 0;
    flatDef += sb.def ?? 0;
    flatSpd += sb.spd ?? 0;
    ap += sb.ap ?? 0;
    pp += sb.pp ?? 0;
    gambitBonus += sb.gambitSlots ?? 0;
    coc.str += sb.str ?? 0;
    coc.con += sb.con ?? 0;
    coc.siz += sb.siz ?? 0;
    coc.dex += sb.dex ?? 0;
    coc.app += sb.app ?? 0;
    coc.int += sb.int ?? 0;
    coc.pow += sb.pow ?? 0;
    coc.edu += sb.edu ?? 0;
    coc.luk += sb.luk ?? 0;
  }

  // Apply equipment
  for (const slot of SLOTS) {
    const eq = op.equipment[slot];
    if (!eq) continue;
    const tpl = EQUIPMENT_TEMPLATES[eq.templateId];
    if (!tpl) continue;
    const scale = 1 + eq.bonusRollPct / 100;
    flatHp += (tpl.baseStats.hp ?? 0) * scale;
    flatAtk += (tpl.baseStats.atk ?? 0) * scale;
    flatDef += (tpl.baseStats.def ?? 0) * scale;
    flatSpd += Math.round((tpl.baseStats.spd ?? 0) * scale);
    ap += tpl.baseStats.ap ?? 0;
    pp += tpl.baseStats.pp ?? 0;
    coc.str += tpl.baseStats.str ?? 0;
    coc.dex += tpl.baseStats.dex ?? 0;
    coc.int += tpl.baseStats.int ?? 0;
    coc.pow += tpl.baseStats.pow ?? 0;
    coc.luk += tpl.baseStats.luk ?? 0;
    if (tpl.grantedSkillId) skillSet.add(tpl.grantedSkillId);
    if (tpl.startTags) startTags.push(...tpl.startTags);
  }

  const starMult = 1 + totalStars * 0.02 + currentClassStars * 0.04;
  const hpApt = (classDef?.aptitude.hpPct ?? 100) / 100;
  const atkApt = (classDef?.aptitude.atkPct ?? 100) / 100;
  const defApt = (classDef?.aptitude.defPct ?? 100) / 100;
  const spdApt = (classDef?.aptitude.spdPct ?? 100) / 100;

  const cocHpPool = Math.round((coc.con + coc.siz) * 0.42);
  const rawHp =
    (op.baseStats.hp * 0.65 + cocHpPool) *
    (1 + levelOffset * 0.1) *
    hpApt *
    starMult *
    (1 + trainingLvl * 0.12) +
    flatHp;

  const rawAtk =
    op.baseStats.atk *
    (1 + levelOffset * 0.08) *
    atkApt *
    starMult *
    (1 + trainingLvl * 0.08) +
    flatAtk;

  const rawDef =
    (op.baseStats.def + Math.round(coc.con * 0.05)) *
    (1 + levelOffset * 0.06) *
    defApt *
    starMult +
    flatDef;

  const rawSpd =
    op.baseStats.spd * spdApt +
    (coc.dex - 50) * 0.14 +
    Math.floor(levelOffset * 0.5) +
    flatSpd;

  const gambitSlotCap = Math.min(7, 2 + Math.floor(coc.int / 30) + gambitBonus);

  return {
    maxHp: Math.max(25, Math.round(rawHp)),
    atk: Math.max(5, Math.round(rawAtk)),
    def: Math.max(0, Math.round(rawDef)),
    spd: Math.max(4, Math.round(rawSpd)),
    maxAp: Math.max(1, ap),
    maxPp: Math.max(0, pp),
    gambitSlotCap,
    evilitySlotCap: getEvilitySlotCap(op.rarity),
    coc,
    skillIds: Array.from(skillSet),
    startTags,
    activeTraitIds,
    totalMasteryStars: totalStars,
  };
}

export function buildCombatUnitFromOperative(
  op: OperativeInstance,
  hq: PermanentHQState,
  row: RowType,
  slot: SlotIndex,
): CombatUnitInput {
  const stats = getOperativeEffectiveStats(op, hq);
  // Enforce INT + tree gambitSlotCap (keeping retreat rules free from cap? Actually slice active/passive up to cap + keep retreat instructions if needed, or slice to cap cleanly!)
  const enabledRules = op.gambits.slice(0, stats.gambitSlotCap);
  return {
    id: op.id,
    name: op.name,
    side: 'ally',
    row,
    slot,
    maxHp: stats.maxHp,
    atk: stats.atk,
    def: stats.def,
    spd: stats.spd,
    maxAp: stats.maxAp,
    maxPp: stats.maxPp,
    coc: stats.coc,
    classId: op.classId,
    traitIds: stats.activeTraitIds,
    gambits: enabledRules.map((g) => ({ ...g })),
    skillIds: stats.skillIds,
    initialTags: stats.startTags,
  };
}

export function getSquadLuckDropBonus(state: GameState, squadId: string): number {
  const squad = state.run.squads.find((s) => s.id === squadId);
  if (!squad || squad.members.length === 0) return 0;
  let totalLuckBonus = 0;
  let count = 0;
  for (const m of squad.members) {
    const op = state.run.roster.find((o) => o.id === m.operativeId);
    if (!op) continue;
    const eff = getOperativeEffectiveStats(op, state.hq);
    totalLuckBonus += Math.max(0, eff.coc.luk - 40) * 0.004;
    count++;
  }
  return count > 0 ? totalLuckBonus / count : 0;
}

export function getStationingBonusPct(
  op: OperativeInstance,
  facilityDef: FacilityDef,
  hq: PermanentHQState,
): number {
  const eff = getOperativeEffectiveStats(op, hq);
  const [statA, statB] = facilityDef.stationStats;
  const avg = (eff.coc[statA] + eff.coc[statB]) / 2;
  const statPct = Math.max(5, Math.round((avg - 30) * 0.85));
  const rarityPct =
    op.rarity === 'SSR' ? 25 : op.rarity === 'SR' ? 16 : op.rarity === 'R' ? 8 : 0;
  return statPct + rarityPct;
}

export type OperativeLocation =
  | { kind: 'idle'; label: string }
  | { kind: 'squad'; squadId: string; label: string }
  | { kind: 'station'; plotId: string; label: string }
  | { kind: 'dojo_mentor'; label: string }
  | { kind: 'dojo_student'; label: string };

export function getOperativeLocation(
  state: GameState,
  operativeId: string,
): OperativeLocation {
  for (const sq of state.run.squads) {
    if (sq.members.some((m) => m.operativeId === operativeId)) {
      return { kind: 'squad', squadId: sq.id, label: sq.name };
    }
  }
  for (const plot of state.run.plots) {
    if (plot.stationedOperativeId === operativeId && plot.facilityId) {
      const fname = FACILITIES[plot.facilityId]?.name ?? plot.facilityId;
      return { kind: 'station', plotId: plot.plotId, label: `驻扎:${fname}` };
    }
  }
  if (state.run.dojo?.mentorId === operativeId) {
    return { kind: 'dojo_mentor', label: '演武:师父' };
  }
  if (state.run.dojo?.studentId === operativeId) {
    return { kind: 'dojo_student', label: '演武:学徒' };
  }
  return { kind: 'idle', label: '待命' };
}

export function getSquadMemberCap(state: GameState): number {
  let barracksBonus = 0;
  for (const plot of state.run.plots) {
    if (plot.facilityId === 'barracks_expansion' && plot.powered && plot.level > 0) {
      barracksBonus += plot.level * (FACILITIES['barracks_expansion']?.squadMemberCapBonusPerLevel ?? 1);
    }
  }
  const hqCommandBonus = state.hq.departments['dept_command'] ?? 0;
  return Math.min(6, 2 + barracksBonus + hqCommandBonus);
}

export function getMaxSquadCount(state: GameState): number {
  let count = 1;
  for (const d of state.run.districts) {
    if (!d.captured) continue;
    const def = DISTRICTS.find((x) => x.id === d.id);
    if (def?.unlocksSquadCount && def.unlocksSquadCount > count) {
      count = def.unlocksSquadCount;
    }
  }
  return Math.min(3, count);
}

export function getVaultCapacity(hq: PermanentHQState): number {
  return 1 + (hq.departments['dept_vault'] ?? 0);
}

export function getLoadoutCapacity(hq: PermanentHQState): {
  blueprints: number;
  tactics: number;
} {
  const bonus = hq.departments['dept_archive'] ?? 0;
  return {
    blueprints: 2 + bonus,
    tactics: 2 + bonus,
  };
}

export function ensureSkillInGambits(
  gambits: GambitRule[],
  skillId: string,
  isPassive: boolean,
): GambitRule[] {
  if (gambits.some((g) => g.actionType === 'SKILL' && g.skillId === skillId)) {
    return gambits;
  }
  const newRule: GambitRule = {
    id: `auto_${skillId}_${Date.now()}`,
    enabled: true,
    actionType: 'SKILL',
    skillId,
    condition1:
      skillId === 'pursuit_shot'
        ? { type: 'ENEMY_HAS_TAG', tag: 'shocked' }
        : skillId === 'mark_follow_up'
          ? { type: 'ENEMY_HAS_TAG', tag: 'marked' }
          : skillId === 'overclock_echo'
            ? { type: 'SELF_HAS_TAG', tag: 'overclock' }
            : skillId === 'emergency_aegis' || skillId === 'guard_cover'
              ? { type: 'ALLY_LOWEST_HP_BELOW', threshold: skillId === 'emergency_aegis' ? 45 : 95 }
              : { type: 'ALWAYS' },
    target:
      skillId === 'pursuit_shot' || skillId === 'mark_follow_up'
        ? 'ENEMY_WITH_TAG'
        : skillId === 'emergency_aegis' || skillId === 'guard_cover'
          ? 'LOWEST_HP_ALLY'
          : 'DEFAULT',
  };
  if (isPassive) {
    return [newRule, ...gambits];
  }
  if (gambits.length > 0) {
    return [...gambits.slice(0, -1), newRule, gambits[gambits.length - 1]!];
  }
  return [newRule];
}

export function getXpToNextLevel(level: number): number {
  return Math.round(45 * Math.pow(1.38, Math.max(0, level - 1)));
}

export function grantOperativeXpAndMastery(
  op: OperativeInstance,
  xpGain: number,
  masteryGain: number,
): {
  nextOp: OperativeInstance;
  leveledUp: boolean;
  unlockedMasteryTraitId: string | null;
} {
  let level = op.level;
  let xp = op.xp + xpGain;
  let skillPoints = op.skillPoints;
  let leveledUp = false;

  while (level < 10 && xp >= getXpToNextLevel(level)) {
    xp -= getXpToNextLevel(level);
    level++;
    skillPoints += 1;
    // High EDU investigators earn extra skill points at Lv.4 and Lv.8
    if (op.coc.edu >= 65 && (level === 4 || level === 8)) {
      skillPoints += 1;
    }
    leveledUp = true;
  }

  const prevMastery = op.classMastery[op.classId] ?? 0;
  const nextMastery = Math.min(500, prevMastery + masteryGain);
  const prevStars = getClassMasteryStars(prevMastery);
  const nextStars = getClassMasteryStars(nextMastery);

  let unlockedMasteryTraitId: string | null = null;
  let learnedTraitIds = op.learnedTraitIds;
  if (prevStars < 3 && nextStars >= 3) {
    const masteryTraitId = CLASSES[op.classId]?.masteryTraitId;
    if (masteryTraitId && !learnedTraitIds.includes(masteryTraitId)) {
      learnedTraitIds = [...learnedTraitIds, masteryTraitId];
      unlockedMasteryTraitId = masteryTraitId;
    }
  }

  return {
    nextOp: {
      ...op,
      level,
      xp,
      skillPoints,
      classMastery: {
        ...op.classMastery,
        [op.classId]: nextMastery,
      },
      learnedTraitIds,
    },
    leveledUp,
    unlockedMasteryTraitId,
  };
}

export function getClassMasteryProgressPct(xp: number): number {
  const stars = getClassMasteryStars(xp);
  if (stars >= 5) return 100;
  return Math.round((xp % 100));
}

export function getStarterSkillIdsForClass(classId: ClassId): string[] {
  return CLASSES[classId]?.starterSkillIds ?? ['baton_strike'];
}
