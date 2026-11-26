import type {
  ClassId,
  CoCStatKey,
  CoCStats,
  CombatReport,
  GambitRule,
  OperativeRarity,
  RetreatOutcome,
  RowType,
  SlotIndex,
  TagId,
} from './combat.ts';

export type Rarity = 'common' | 'uncommon' | 'rare';
export type EquipmentSlot = 'weapon' | 'armor' | 'module';

export interface ResourceBundle {
  materials: number;
  parts: number;
  crystals: number;
  manuals: number;
  intel: number;
}

export interface EquipmentStats {
  hp?: number;
  atk?: number;
  def?: number;
  spd?: number;
  ap?: number;
  pp?: number;
  str?: number;
  dex?: number;
  int?: number;
  pow?: number;
  luk?: number;
}

export interface EquipmentTemplate {
  id: string;
  name: string;
  slot: EquipmentSlot;
  rarity: Rarity;
  flavor: string;
  baseStats: EquipmentStats;
  grantedSkillId?: string;
  startTags?: Array<{ tag: TagId; stacks: number }>;
  craftCost?: { materials: number; parts: number; crystals: number };
  salvageYield: ResourceBundle;
}

export interface EquipmentInstance {
  uid: string;
  templateId: string;
  rarity: Rarity;
  bonusRollPct: number;
}

export interface SkillTreeNodeDef {
  id: string;
  name: string;
  icon: string;
  x: number;
  y: number;
  isKeystone?: boolean;
  tier: 1 | 2 | 3;
  costSp: number;
  prereqNodeIds: string[];
  description: string;
  statBonus?: Partial<CoCStats> & {
    hp?: number;
    atk?: number;
    def?: number;
    spd?: number;
    ap?: number;
    pp?: number;
    gambitSlots?: number;
  };
  grantSkillId?: string;
  grantTraitId?: string;
  grantTacticId?: string;
}

export interface ClassDef {
  id: ClassId;
  name: string;
  icon: string;
  roleLabel: string;
  flavor: string;
  preferredRow: RowType;
  aptitude: {
    hpPct: number;
    atkPct: number;
    defPct: number;
    spdPct: number;
  };
  starterSkillIds: string[];
  masteryTraitId: string;
  treeNodes: SkillTreeNodeDef[];
}

export interface OperativeBaseTemplate {
  archetypeId: string;
  name: string;
  title: string;
  rarity: OperativeRarity;
  isSpecial: boolean;
  flavor: string;
  preferredRow: RowType;
  defaultClassId: ClassId;
  coc: CoCStats;
  baseStats: {
    hp: number;
    atk: number;
    def: number;
    spd: number;
    ap: number;
    pp: number;
  };
  innateSkillIds: string[];
  innateTraitIds: string[];
  personalTreeNodes: SkillTreeNodeDef[];
  defaultGambits: GambitRule[];
}

export interface OperativeInstance {
  id: string;
  archetypeId: string;
  name: string;
  title: string;
  rarity: OperativeRarity;
  isSpecial: boolean;
  flavor: string;
  coc: CoCStats;
  classId: ClassId;
  classMastery: Partial<Record<ClassId, number>>;
  level: number;
  xp: number;
  skillPoints: number;
  unlockedNodeIds: string[];
  learnedSkillIds: string[];
  innateTraitIds: string[];
  learnedTraitIds: string[];
  equippedTraitIds: string[];
  preferredRow: RowType;
  baseStats: {
    hp: number;
    atk: number;
    def: number;
    spd: number;
    ap: number;
    pp: number;
  };
  equipment: Partial<Record<EquipmentSlot, EquipmentInstance>>;
  gambits: GambitRule[];
}

export interface SpecialOperativeProgress {
  archetypeId: string;
  unlockedAtCycle: number;
  timesDeployed: number;
  classId?: ClassId;
  classMastery?: Partial<Record<ClassId, number>>;
  learnedSkillIds?: string[];
  learnedTraitIds?: string[];
}

export interface RecruitCandidate {
  candidateId: string;
  name: string;
  title: string;
  rarity: OperativeRarity;
  flavor: string;
  coc: CoCStats;
  classId: ClassId;
  preferredRow: RowType;
  baseStats: {
    hp: number;
    atk: number;
    def: number;
    spd: number;
    ap: number;
    pp: number;
  };
  innateTraitIds: string[];
  innateSkillIds: string[];
  defaultGambits: GambitRule[];
  hireCostMaterials: number;
}

export interface SquadMemberAssignment {
  operativeId: string;
  row: RowType;
  slot: SlotIndex;
}

export interface SquadState {
  id: string;
  name: string;
  members: SquadMemberAssignment[];
  assignedDepthId: string | null;
  patrolProgressSec: number;
}

export interface PatrolBenchmark {
  depthId: string;
  squadId: string;
  durationSec: number;
  outcome: RetreatOutcome;
  wavesCleared: number;
  totalWaves: number;
  yieldFactor: number;
  dropRateFactor: number;
  erosionBonusPct: number;
  guaranteedYield: ResourceBundle;
  sampleReport: CombatReport;
}

export interface DistrictRuntimeState {
  id: string;
  captured: boolean;
}

export interface BuildingPlot {
  plotId: string;
  sourceId: 'hq_base' | string;
  terrainLabel: string;
  bonusCategory?: 'power' | 'economy' | 'research' | 'support';
  bonusPct: number;
  facilityId: string | null;
  level: number;
  enabled: boolean;
  powered: boolean;
  stationedOperativeId: string | null;
}

export interface DojoTrainingState {
  mentorId: string;
  studentId: string;
  targetKind: 'skill' | 'trait' | 'tactic';
  targetId: string;
  progressSec: number;
  requiredSec: number;
}

export interface FacilityDef {
  id: string;
  name: string;
  category: 'power' | 'economy' | 'research' | 'support';
  flavor: string;
  effectSummary: string;
  starterAvailable: boolean;
  unlockDistrictId?: string;
  requiresBlueprint?: boolean;
  maxPerBase?: number;
  maxLevel: number;
  powerDeltaPerLevel: number;
  baseCostMaterials: number;
  baseCostParts?: number;
  costGrowth: number;
  breakthroughEvery: number;
  baseCostCrystals: number;
  stationStats: [CoCStatKey, CoCStatKey];
  materialsPerSecPerLevel?: number;
  materialsConsumePerSecPerLevel?: number;
  partsPerSecPerLevel?: number;
  crystalsPerSecPerLevel?: number;
  manualsPerSecPerLevel?: number;
  intelPerSecPerLevel?: number;
  squadMemberCapBonusPerLevel?: number;
}

export interface EnemyUnitDef {
  name: string;
  row: RowType;
  slot: SlotIndex;
  hp: number;
  atk: number;
  def: number;
  spd: number;
  ap: number;
  pp: number;
  skillIds: string[];
  traitIds?: string[];
  gambits: GambitRule[];
  initialTags?: Array<{ tag: TagId; stacks: number }>;
}

export interface DistrictPlotTemplate {
  suffix: string;
  terrainLabel: string;
  bonusCategory?: 'power' | 'economy' | 'research' | 'support';
  bonusPct: number;
}

export interface DistrictDef {
  id: string;
  ring: number;
  name: string;
  flavor: string;
  firstClearReward: ResourceBundle;
  unlocksSquadCount?: number;
  unlocksFacilityIds?: string[];
  plotTemplates: DistrictPlotTemplate[];
  recruitTier: number;
  waves: EnemyUnitDef[][];
}

export interface LootEntry {
  kind: 'equipment' | 'blueprint_facility' | 'blueprint_tactic' | 'special_operative';
  refId: string;
  baseWeight: number;
  baseChance: number;
}

export interface OtherworldDepthDef {
  id: string;
  depth: number;
  name: string;
  flavor: string;
  recommendedPower: number;
  baseTravelSec: number;
  baseYield: ResourceBundle;
  firstClearBonus: ResourceBundle;
  waves: EnemyUnitDef[][];
  lootTable: LootEntry[];
}

export interface OtherworldModifierDef {
  id: string;
  name: string;
  flavor: string;
  ruleSummary: string;
  enemyHpMultiplier?: number;
  enemyAtkMultiplier?: number;
  allyStartTags?: Array<{ tag: TagId; stacks: number }>;
  enemyStartTags?: Array<{ tag: TagId; stacks: number }>;
  crystalYieldMultiplier?: number;
  dropRateMultiplier?: number;
  powerGenerationBonus?: number;
}

export interface HQDepartmentDef {
  id: string;
  name: string;
  flavor: string;
  effectSummary: string;
  maxLevel: number;
  baseOrgExpCost: number;
  costMultiplier: number;
}

export interface TacticBlueprintDef {
  id: string;
  name: string;
  category: 'condition' | 'directive' | 'target';
  refKey: string;
  description: string;
  researchCostManuals?: number;
  starterUnlocked?: boolean;
}

export interface UnfoldingState {
  baseUnlocked: boolean;
  workshopUnlocked: boolean;
  otherworldUnlocked: boolean;
  dojoUnlocked: boolean;
  tacticsGuided: boolean;
  hqUnlocked: boolean;
  unlockedAllDebug?: boolean;
}

export interface StoryHooksState {
  cycleCount: number;
  maxDistrictRingEver: number;
  maxOtherworldDepthEver: number;
  seenFlavorIds: string[];
  truthFlags: Record<string, number | boolean>;
  unfolding: UnfoldingState;
}

export interface PermanentHQState {
  orgExp: number;
  departments: Record<string, number>;
  unlockedBlueprints: string[];
  unlockedTactics: string[];
  vaultEquipment: EquipmentInstance[];
  specialOperatives: SpecialOperativeProgress[];
  story: StoryHooksState;
}

export interface RunState {
  runSeed: number;
  prngState: number;
  activeModifierId: string;
  selectedLoadout: {
    blueprints: string[];
    tactics: string[];
  };
  resources: ResourceBundle;
  districts: DistrictRuntimeState[];
  plots: BuildingPlot[];
  dojo: DojoTrainingState | null;
  roster: OperativeInstance[];
  recruitPool: RecruitCandidate[];
  recruitRefreshCost: number;
  inventory: EquipmentInstance[];
  autoSalvageMaxRarity: 'none' | 'common' | 'uncommon';
  squads: SquadState[];
  clearedDepths: Record<string, boolean>;
  patrolBenchmarks: Record<string, PatrolBenchmark>;
  lastBattleReport: CombatReport | null;
  eventTicker: string[];
}

export interface GameState {
  version: number;
  savedAt: number;
  totalPlayTimeSec: number;
  hq: PermanentHQState;
  run: RunState;
}

export interface PowerGridSummary {
  generated: number;
  demand: number;
  net: number;
  brownedOutPlotIds: string[];
}

export interface OfflineSquadReport {
  squadId: string;
  squadName: string;
  depthName: string;
  completedCycles: number;
  outcome: RetreatOutcome;
  yieldEarned: ResourceBundle;
}

export interface OfflineSummary {
  elapsedSec: number;
  passiveYield: ResourceBundle;
  squadReports: OfflineSquadReport[];
  newEquipment: EquipmentInstance[];
  salvagedCount: number;
  salvageYield: ResourceBundle;
  newBlueprints: string[];
  newTactics: string[];
  newSpecialOperatives: string[];
  dojoCompletedText?: string;
}
