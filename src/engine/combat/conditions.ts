import type {
  GambitCondition,
  GambitRule,
  RowType,
  SlotIndex,
  TagId,
  TargetSelector,
} from '../../types/combat.ts';

export interface LiveCombatUnit {
  id: string;
  name: string;
  side: 'ally' | 'enemy';
  row: RowType;
  slot: SlotIndex;
  hp: number;
  maxHp: number;
  shield: number;
  atk: number;
  def: number;
  spd: number;
  ap: number;
  maxAp: number;
  pp: number;
  maxPp: number;
  alive: boolean;
  tags: Partial<Record<TagId, number>>;
  traitIds: string[];
  gambits: GambitRule[];
  skillIds: string[];
}

export interface ConditionContext {
  actor: LiveCombatUnit;
  allies: LiveCombatUnit[];
  enemies: LiveCombatUnit[];
  wave: number;
  round: number;
  preferredTag?: TagId;
}

export function getTagStacks(unit: LiveCombatUnit, tag: TagId): number {
  return unit.tags[tag] ?? 0;
}

export function evaluateCondition(
  cond: GambitCondition,
  ctx: ConditionContext,
): boolean {
  const livingAllies = ctx.allies.filter((u) => u.alive);
  const livingEnemies = ctx.enemies.filter((u) => u.alive);
  const selfHpPct = (ctx.actor.hp / Math.max(1, ctx.actor.maxHp)) * 100;

  switch (cond.type) {
    case 'ALWAYS':
      return true;
    case 'SELF_HP_BELOW':
      return selfHpPct < (cond.threshold ?? 50);
    case 'SELF_HP_ABOVE':
      return selfHpPct >= (cond.threshold ?? 50);
    case 'ALLY_LOWEST_HP_BELOW': {
      if (livingAllies.length === 0) return false;
      const lowest = Math.min(
        ...livingAllies.map((u) => (u.hp / Math.max(1, u.maxHp)) * 100),
      );
      return lowest < (cond.threshold ?? 50);
    }
    case 'SQUAD_AVG_HP_BELOW': {
      if (livingAllies.length === 0) return false;
      const avg =
        livingAllies.reduce((sum, u) => sum + (u.hp / Math.max(1, u.maxHp)) * 100, 0) /
        livingAllies.length;
      return avg < (cond.threshold ?? 50);
    }
    case 'FRONT_ROW_ALIVE_BELOW': {
      const frontAlive = livingAllies.filter((u) => u.row === 'front').length;
      return frontAlive < (cond.threshold ?? 1);
    }
    case 'ENEMY_HAS_TAG': {
      const tag = cond.tag ?? ctx.preferredTag;
      if (tag) {
        return livingEnemies.some((u) => getTagStacks(u, tag) > 0);
      }
      return livingEnemies.some((u) =>
        Object.values(u.tags).some((count) => (count ?? 0) > 0),
      );
    }
    case 'SELF_HAS_TAG': {
      const tag = cond.tag ?? ctx.preferredTag;
      if (!tag) return false;
      return getTagStacks(ctx.actor, tag) > 0;
    }
    case 'ENEMY_COUNT_AT_LEAST':
      return livingEnemies.length >= (cond.threshold ?? 1);
    case 'WAVE_AT_LEAST':
      return ctx.wave >= (cond.threshold ?? 1);
    case 'ROUND_AT_LEAST':
      return ctx.round >= (cond.threshold ?? 1);
  }
}

export function evaluateRuleConditions(
  rule: GambitRule,
  ctx: ConditionContext,
): boolean {
  if (!evaluateCondition(rule.condition1, ctx)) {
    return false;
  }
  if (rule.condition2 && !evaluateCondition(rule.condition2, ctx)) {
    return false;
  }
  return true;
}

function compareDefaultOrder(a: LiveCombatUnit, b: LiveCombatUnit): number {
  if (a.row !== b.row) {
    return a.row === 'front' ? -1 : 1;
  }
  if (a.slot !== b.slot) {
    return a.slot - b.slot;
  }
  return a.id.localeCompare(b.id);
}

export function selectTarget<T extends LiveCombatUnit>(
  selector: TargetSelector,
  candidates: T[],
  actor: T,
  preferredTag?: TagId,
): T | null {
  const alive = candidates.filter((u) => u.alive);
  if (selector === 'SELF') {
    return actor.alive ? actor : null;
  }
  if (alive.length === 0) return null;

  switch (selector) {
    case 'DEFAULT': {
      const sorted = [...alive].sort(compareDefaultOrder);
      return sorted[0] ?? null;
    }
    case 'FRONT_LOWEST_HP': {
      const sorted = [...alive].sort((a, b) => {
        if (a.row !== b.row) return a.row === 'front' ? -1 : 1;
        if (a.hp !== b.hp) return a.hp - b.hp;
        return a.slot - b.slot;
      });
      return sorted[0] ?? null;
    }
    case 'BACK_PRIORITY': {
      const sorted = [...alive].sort((a, b) => {
        if (a.row !== b.row) return a.row === 'back' ? -1 : 1;
        if (a.hp !== b.hp) return a.hp - b.hp;
        return a.slot - b.slot;
      });
      return sorted[0] ?? null;
    }
    case 'HIGHEST_ATK': {
      const sorted = [...alive].sort((a, b) => {
        if (a.atk !== b.atk) return b.atk - a.atk;
        return compareDefaultOrder(a, b);
      });
      return sorted[0] ?? null;
    }
    case 'ENEMY_WITH_TAG': {
      const tagged = alive.filter((u) =>
        preferredTag
          ? getTagStacks(u, preferredTag) > 0
          : Object.values(u.tags).some((count) => (count ?? 0) > 0),
      );
      if (tagged.length === 0) return null;
      const sorted = [...tagged].sort((a, b) => a.hp - b.hp || compareDefaultOrder(a, b));
      return sorted[0] ?? null;
    }
    case 'LOWEST_HP_ALLY': {
      const sorted = [...alive].sort((a, b) => {
        const pctA = a.hp / Math.max(1, a.maxHp);
        const pctB = b.hp / Math.max(1, b.maxHp);
        if (pctA !== pctB) return pctA - pctB;
        return compareDefaultOrder(a, b);
      });
      return sorted[0] ?? null;
    }
  }
}
