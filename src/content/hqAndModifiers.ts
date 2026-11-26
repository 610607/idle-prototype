import type { HQDepartmentDef, OtherworldModifierDef } from '../types/game.ts';

export const OTHERWORLD_MODIFIERS: Record<string, OtherworldModifierDef> = {
  dense_fog: {
    id: 'dense_fog',
    name: '浓雾滞留期',
    flavor: '能见度降到路灯底下半米，但老居民说雾里能多听到一点声响。',
    ruleSummary: '全员开局自带 1 层 [固守]；异常情报与装备掉率 +15%。',
    allyStartTags: [{ tag: 'fortified', stacks: 1 }],
    dropRateMultiplier: 1.15,
  },
  static_turbulence: {
    id: 'static_turbulence',
    name: '静电紊流期',
    flavor: '空气像没接地线的变电箱，随手推开门都会劈啪作响。',
    ruleSummary: '敌我开局均附带 1 层 [震荡]；基地发电上限 +8 kW。',
    allyStartTags: [{ tag: 'shocked', stacks: 1 }],
    enemyStartTags: [{ tag: 'shocked', stacks: 1 }],
    powerGenerationBonus: 8,
  },
  phase_tide: {
    id: 'phase_tide',
    name: '相位高潮期',
    flavor: '异界像涨潮一样往市中心漫过半条街，结晶却沿着水洼成片冒头。',
    ruleSummary: '敌方生命 +15%，但异界结晶产出 ×1.35。',
    enemyHpMultiplier: 1.15,
    crystalYieldMultiplier: 1.35,
  },
  rich_vein: {
    id: 'rich_vein',
    name: '余晖富矿期',
    flavor: '上次大渗透退潮后在沥青缝里留下了格外亮的结晶脉络。',
    ruleSummary: '结晶产出 ×1.25、掉率 +10%，敌方攻击 +10%。',
    enemyAtkMultiplier: 1.1,
    crystalYieldMultiplier: 1.25,
    dropRateMultiplier: 1.1,
  },
};

export const MODIFIER_IDS = Object.keys(OTHERWORLD_MODIFIERS);

export const HQ_DEPARTMENTS: Record<string, HQDepartmentDef> = {
  dept_fast_response: {
    id: 'dept_fast_response',
    name: '外围快反规章',
    flavor: '记住哪几条巷口容易漏电、哪家五金店后门能直接借电。',
    effectSummary: '每级在大渗透重开时自动夺回前 N 个街区，并预先架好基础手摇发电机（跳过重复开荒）。',
    maxLevel: 3,
    baseOrgExpCost: 18,
    costMultiplier: 2.0,
  },
  dept_logistics: {
    id: 'dept_logistics',
    name: '后勤统筹处',
    flavor: '把散落各街区的废铜烂铁和建材统一编目，效率翻倍。',
    effectSummary: '每级永久提升全基地与远征的常规物资及异界结晶产出 +15%。',
    maxLevel: 8,
    baseOrgExpCost: 12,
    costMultiplier: 1.65,
  },
  dept_archive: {
    id: 'dept_archive',
    name: '知识库扩容科',
    flavor: '腾出地下室档案柜，允许每轮携带更多图纸与高级战术出门。',
    effectSummary: '每级让新轮回初始可勾选带入的知识库蓝图与战术容量 +1（基础各 2 项）。',
    maxLevel: 4,
    baseOrgExpCost: 16,
    costMultiplier: 1.8,
  },
  dept_vault: {
    id: 'dept_vault',
    name: '总部铅封库',
    flavor: '特制铅箱能在大渗透冲刷时保住最关键的几件装备不随街区坍缩。',
    effectSummary: '每级增加 1 个总部跨轮回装备封存格（初始 1 格，最高 4 格）。',
    maxLevel: 3,
    baseOrgExpCost: 22,
    costMultiplier: 2.1,
  },
  dept_command: {
    id: 'dept_command',
    name: '联勤参谋室',
    flavor: '整理了历次小队交班手册，前线无需重新磨合就能扩大编制。',
    effectSummary: '每级永久提升每支小队基础编入人数 +1（开局即生效，最高 +2）。',
    maxLevel: 2,
    baseOrgExpCost: 35,
    costMultiplier: 2.5,
  },
  dept_training: {
    id: 'dept_training',
    name: '应急教导队',
    flavor: '老队员总结出的防身顺口溜，新人听一遍就能少挨两下闷棍。',
    effectSummary: '每级永久提升全员基础生命 +12%、基础攻击 +8%。',
    maxLevel: 6,
    baseOrgExpCost: 15,
    costMultiplier: 1.7,
  },
};

export const HQ_DEPARTMENT_IDS = Object.keys(HQ_DEPARTMENTS);

export function getDepartmentUpgradeCost(def: HQDepartmentDef, currentLevel: number): number {
  return Math.round(def.baseOrgExpCost * Math.pow(def.costMultiplier, currentLevel));
}
