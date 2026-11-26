import { z } from 'zod';

const TagIdSchema = z.enum([
  'shocked',
  'marked',
  'overclock',
  'erosion',
  'shielded',
  'fortified',
]);

const RowTypeSchema = z.enum(['front', 'back']);
const SlotIndexSchema = z.union([z.literal(0), z.literal(1), z.literal(2)]);
const RaritySchema = z.enum(['common', 'uncommon', 'rare']);
const OperativeRaritySchema = z.enum(['N', 'R', 'SR', 'SSR']);
const ClassIdSchema = z.enum([
  'vanguard',
  'marksman',
  'medic',
  'striker',
  'occultist',
  'technician',
]);

const CoCStatsSchema = z.object({
  str: z.number(),
  con: z.number(),
  siz: z.number(),
  dex: z.number(),
  app: z.number(),
  int: z.number(),
  pow: z.number(),
  edu: z.number(),
  luk: z.number(),
});

const ResourceBundleSchema = z.object({
  materials: z.number(),
  parts: z.number(),
  crystals: z.number(),
  manuals: z.number(),
  intel: z.number(),
});

const GambitConditionSchema = z.object({
  type: z.enum([
    'ALWAYS',
    'SELF_HP_BELOW',
    'SELF_HP_ABOVE',
    'ALLY_LOWEST_HP_BELOW',
    'SQUAD_AVG_HP_BELOW',
    'FRONT_ROW_ALIVE_BELOW',
    'ENEMY_HAS_TAG',
    'SELF_HAS_TAG',
    'ENEMY_COUNT_AT_LEAST',
    'WAVE_AT_LEAST',
    'ROUND_AT_LEAST',
  ]),
  threshold: z.number().optional(),
  tag: TagIdSchema.optional(),
});

const GambitRuleSchema = z.object({
  id: z.string(),
  enabled: z.boolean(),
  actionType: z.enum(['SKILL', 'RETREAT_PLANNED', 'RETREAT_EMERGENCY']),
  skillId: z.string().optional(),
  condition1: GambitConditionSchema,
  condition2: GambitConditionSchema.optional(),
  target: z.enum([
    'DEFAULT',
    'FRONT_LOWEST_HP',
    'BACK_PRIORITY',
    'HIGHEST_ATK',
    'ENEMY_WITH_TAG',
    'LOWEST_HP_ALLY',
    'SELF',
  ]),
});

const EquipmentInstanceSchema = z.object({
  uid: z.string(),
  templateId: z.string(),
  rarity: RaritySchema,
  bonusRollPct: z.number(),
});

const OperativeInstanceSchema = z.object({
  id: z.string(),
  archetypeId: z.string(),
  name: z.string(),
  title: z.string(),
  rarity: OperativeRaritySchema,
  isSpecial: z.boolean(),
  flavor: z.string(),
  coc: CoCStatsSchema,
  classId: ClassIdSchema,
  classMastery: z.record(z.string(), z.number()),
  level: z.number(),
  xp: z.number(),
  skillPoints: z.number(),
  unlockedNodeIds: z.array(z.string()),
  learnedSkillIds: z.array(z.string()),
  innateTraitIds: z.array(z.string()),
  learnedTraitIds: z.array(z.string()),
  equippedTraitIds: z.array(z.string()),
  preferredRow: RowTypeSchema,
  baseStats: z.object({
    hp: z.number(),
    atk: z.number(),
    def: z.number(),
    spd: z.number(),
    ap: z.number(),
    pp: z.number(),
  }),
  equipment: z.object({
    weapon: EquipmentInstanceSchema.optional(),
    armor: EquipmentInstanceSchema.optional(),
    module: EquipmentInstanceSchema.optional(),
  }),
  gambits: z.array(GambitRuleSchema),
});

const RecruitCandidateSchema = z.object({
  candidateId: z.string(),
  name: z.string(),
  title: z.string(),
  rarity: OperativeRaritySchema,
  flavor: z.string(),
  coc: CoCStatsSchema,
  classId: ClassIdSchema,
  preferredRow: RowTypeSchema,
  baseStats: z.object({
    hp: z.number(),
    atk: z.number(),
    def: z.number(),
    spd: z.number(),
    ap: z.number(),
    pp: z.number(),
  }),
  innateTraitIds: z.array(z.string()),
  innateSkillIds: z.array(z.string()),
  defaultGambits: z.array(GambitRuleSchema),
  hireCostMaterials: z.number(),
});

const CombatLogEntrySchema = z.object({
  wave: z.number(),
  round: z.number(),
  kind: z.enum([
    'WAVE_START',
    'ROUND_START',
    'ACTIVE_AP',
    'PASSIVE_PP',
    'FACILITY_SUPPORT',
    'MODIFIER',
    'RETREAT',
    'DEFEAT',
    'VICTORY',
  ]),
  actorName: z.string().optional(),
  text: z.string(),
  tags: z.array(TagIdSchema).optional(),
});

const CombatUnitSummarySchema = z.object({
  id: z.string(),
  name: z.string(),
  side: z.enum(['ally', 'enemy']),
  damageDealt: z.number(),
  damageTaken: z.number(),
  healingShieldDone: z.number(),
  ppReactions: z.number(),
  aliveAtEnd: z.boolean(),
  hpEndPct: z.number(),
});

const CombatReportSchema = z.object({
  victory: z.boolean(),
  fullClear: z.boolean(),
  outcome: z.enum(['PLANNED', 'EMERGENCY', 'WIPE']),
  wavesCleared: z.number(),
  totalWaves: z.number(),
  totalRounds: z.number(),
  durationSec: z.number(),
  yieldFactor: z.number(),
  dropRateFactor: z.number(),
  erosionBonusPct: z.number(),
  entries: z.array(CombatLogEntrySchema),
  unitStats: z.array(CombatUnitSummarySchema),
});

export const GameSaveSchemaV2 = z.object({
  version: z.literal(2),
  savedAt: z.number(),
  totalPlayTimeSec: z.number(),
  hq: z.object({
    orgExp: z.number(),
    departments: z.record(z.string(), z.number()),
    unlockedBlueprints: z.array(z.string()),
    unlockedTactics: z.array(z.string()),
    vaultEquipment: z.array(EquipmentInstanceSchema),
    specialOperatives: z.array(
      z.object({
        archetypeId: z.string(),
        unlockedAtCycle: z.number(),
        timesDeployed: z.number(),
        classId: ClassIdSchema.optional(),
        classMastery: z.record(z.string(), z.number()).optional(),
        learnedSkillIds: z.array(z.string()).optional(),
        learnedTraitIds: z.array(z.string()).optional(),
      }),
    ),
    story: z.object({
      cycleCount: z.number(),
      maxDistrictRingEver: z.number(),
      maxOtherworldDepthEver: z.number(),
      seenFlavorIds: z.array(z.string()),
      truthFlags: z.record(z.string(), z.union([z.number(), z.boolean()])),
      unfolding: z.object({
        baseUnlocked: z.boolean(),
        workshopUnlocked: z.boolean(),
        otherworldUnlocked: z.boolean(),
        dojoUnlocked: z.boolean(),
        tacticsGuided: z.boolean(),
        hqUnlocked: z.boolean(),
        unlockedAllDebug: z.boolean().optional(),
      }),
    }),
  }),
  run: z.object({
    runSeed: z.number(),
    prngState: z.number(),
    activeModifierId: z.string(),
    selectedLoadout: z.object({
      blueprints: z.array(z.string()),
      tactics: z.array(z.string()),
    }),
    resources: ResourceBundleSchema,
    districts: z.array(
      z.object({
        id: z.string(),
        captured: z.boolean(),
      }),
    ),
    plots: z.array(
      z.object({
        plotId: z.string(),
        sourceId: z.string(),
        terrainLabel: z.string(),
        bonusCategory: z.enum(['power', 'economy', 'research', 'support']).optional(),
        bonusPct: z.number(),
        facilityId: z.string().nullable(),
        level: z.number(),
        enabled: z.boolean(),
        powered: z.boolean(),
        stationedOperativeId: z.string().nullable(),
      }),
    ),
    dojo: z
      .object({
        mentorId: z.string(),
        studentId: z.string(),
        targetKind: z.enum(['skill', 'trait', 'tactic']),
        targetId: z.string(),
        progressSec: z.number(),
        requiredSec: z.number(),
      })
      .nullable(),
    roster: z.array(OperativeInstanceSchema),
    recruitPool: z.array(RecruitCandidateSchema),
    recruitRefreshCost: z.number(),
    inventory: z.array(EquipmentInstanceSchema),
    autoSalvageMaxRarity: z.enum(['none', 'common', 'uncommon']),
    squads: z.array(
      z.object({
        id: z.string(),
        name: z.string(),
        members: z.array(
          z.object({
            operativeId: z.string(),
            row: RowTypeSchema,
            slot: SlotIndexSchema,
          }),
        ),
        assignedDepthId: z.string().nullable(),
        patrolProgressSec: z.number(),
      }),
    ),
    clearedDepths: z.record(z.string(), z.boolean()),
    patrolBenchmarks: z.record(
      z.string(),
      z.object({
        depthId: z.string(),
        squadId: z.string(),
        durationSec: z.number(),
        outcome: z.enum(['PLANNED', 'EMERGENCY', 'WIPE']),
        wavesCleared: z.number(),
        totalWaves: z.number(),
        yieldFactor: z.number(),
        dropRateFactor: z.number(),
        erosionBonusPct: z.number(),
        guaranteedYield: ResourceBundleSchema,
        sampleReport: CombatReportSchema,
      }),
    ),
    lastBattleReport: CombatReportSchema.nullable(),
    eventTicker: z.array(z.string()),
  }),
});
