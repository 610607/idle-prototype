export type TagId =
  | 'shocked'
  | 'marked'
  | 'overclock'
  | 'erosion'
  | 'shielded'
  | 'fortified';

export type RowType = 'front' | 'back';
export type SlotIndex = 0 | 1 | 2;
export type RetreatOutcome = 'PLANNED' | 'EMERGENCY' | 'WIPE';
export type OperativeRarity = 'N' | 'R' | 'SR' | 'SSR';
export type ClassId =
  | 'vanguard'
  | 'marksman'
  | 'medic'
  | 'striker'
  | 'occultist'
  | 'technician';

export interface CoCStats {
  str: number; // 力量：近战威力与重装驾驭
  con: number; // 体质：生命基础与发电值班
  siz: number; // 体型：前排生命厚实度与体力活
  dex: number; // 敏捷：战斗先攻 SPD 与远程威力
  app: number; // 外貌：联络招募谈判与人际交涉
  int: number; // 智力：战术槽位上限与研习工坊
  pow: number; // 意志：异界抗性、异常技与演武专注
  edu: number; // 教育：档案研究、医疗与手札编译
  luk: number; // 幸运：小队稀有掉落与招募品质
}

export type CoCStatKey = keyof CoCStats;

export type ConditionType =
  | 'ALWAYS'
  | 'SELF_HP_BELOW'
  | 'SELF_HP_ABOVE'
  | 'ALLY_LOWEST_HP_BELOW'
  | 'SQUAD_AVG_HP_BELOW'
  | 'FRONT_ROW_ALIVE_BELOW'
  | 'ENEMY_HAS_TAG'
  | 'SELF_HAS_TAG'
  | 'ENEMY_COUNT_AT_LEAST'
  | 'WAVE_AT_LEAST'
  | 'ROUND_AT_LEAST';

export interface GambitCondition {
  type: ConditionType;
  threshold?: number;
  tag?: TagId;
}

export type TargetSelector =
  | 'DEFAULT'
  | 'FRONT_LOWEST_HP'
  | 'BACK_PRIORITY'
  | 'HIGHEST_ATK'
  | 'ENEMY_WITH_TAG'
  | 'LOWEST_HP_ALLY'
  | 'SELF';

export type GambitActionType = 'SKILL' | 'RETREAT_PLANNED' | 'RETREAT_EMERGENCY';

export interface GambitRule {
  id: string;
  enabled: boolean;
  actionType: GambitActionType;
  skillId?: string;
  condition1: GambitCondition;
  condition2?: GambitCondition;
  target: TargetSelector;
}

export type SkillRange = 'melee' | 'ranged' | 'support';
export type SkillKind = 'active' | 'passive';
export type PassiveTrigger =
  | 'ON_ALLY_TARGETED'
  | 'ON_ALLY_DEAL_DAMAGE'
  | 'ON_TAG_APPLIED'
  | 'ON_ALLY_LOW_HP'
  | 'ON_ROUND_START';

export interface SkillDef {
  id: string;
  name: string;
  kind: SkillKind;
  range: SkillRange;
  scaleStat?: CoCStatKey;
  apCost: number;
  ppCost: number;
  trigger?: PassiveTrigger;
  requiredEnemyTag?: TagId;
  requiredSelfTag?: TagId;
  targetAlly?: boolean;
  targetSelf?: boolean;
  powerMultiplier: number;
  flatBonus?: number;
  healRatio?: number;
  shieldRatio?: number;
  applyTargetTags?: Array<{ tag: TagId; stacks: number }>;
  applySelfTags?: Array<{ tag: TagId; stacks: number }>;
  applyAttackerTags?: Array<{ tag: TagId; stacks: number }>;
  spreadTag?: TagId;
  consumeEnemyTagBonus?: { tag: TagId; bonusPerStackPct: number; consume?: boolean };
  selfDamagePctMaxHp?: number;
  guardIntercept?: boolean;
  description: string;
}

export interface TraitDef {
  id: string;
  name: string;
  flavor: string;
  summary: string;
  isUnique?: boolean;
  minRarity?: OperativeRarity;
  statBonus: {
    hp?: number;
    atk?: number;
    def?: number;
    spd?: number;
    ap?: number;
    pp?: number;
    str?: number;
    con?: number;
    siz?: number;
    dex?: number;
    app?: number;
    int?: number;
    pow?: number;
    edu?: number;
    luk?: number;
  };
  startTags?: Array<{ tag: TagId; stacks: number }>;
  damageVsTagBonusPct?: { tag: TagId; bonusPct: number };
  onHitApplyTag?: { tag: TagId; stacks: number };
  bonusSkillId?: string;
}

export interface CombatUnitInput {
  id: string;
  name: string;
  side: 'ally' | 'enemy';
  row: RowType;
  slot: SlotIndex;
  maxHp: number;
  atk: number;
  def: number;
  spd: number;
  maxAp: number;
  maxPp: number;
  coc?: CoCStats;
  classId?: ClassId;
  traitIds: string[];
  gambits: GambitRule[];
  skillIds: string[];
  initialTags?: Array<{ tag: TagId; stacks: number }>;
}

export interface CombatLogEntry {
  wave: number;
  round: number;
  kind:
    | 'WAVE_START'
    | 'ROUND_START'
    | 'ACTIVE_AP'
    | 'PASSIVE_PP'
    | 'FACILITY_SUPPORT'
    | 'MODIFIER'
    | 'RETREAT'
    | 'DEFEAT'
    | 'VICTORY';
  actorName?: string;
  text: string;
  tags?: TagId[];
}

export interface CombatUnitSummary {
  id: string;
  name: string;
  side: 'ally' | 'enemy';
  damageDealt: number;
  damageTaken: number;
  healingShieldDone: number;
  ppReactions: number;
  aliveAtEnd: boolean;
  hpEndPct: number;
}

export interface CombatReport {
  victory: boolean;
  fullClear: boolean;
  outcome: RetreatOutcome;
  wavesCleared: number;
  totalWaves: number;
  totalRounds: number;
  durationSec: number;
  yieldFactor: number;
  dropRateFactor: number;
  erosionBonusPct: number;
  entries: CombatLogEntry[];
  unitStats: CombatUnitSummary[];
}
