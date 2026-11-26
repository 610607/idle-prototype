import type { GameState, UnfoldingState } from '../types/game.ts';

export function syncUnfoldingFlags(state: GameState): GameState {
  const cur = state.hq.story.unfolding;
  const dist1 = state.run.districts.find((d) => d.id === 'district_1')?.captured ?? false;
  const dist2 = state.run.districts.find((d) => d.id === 'district_2')?.captured ?? false;
  const dist3 = state.run.districts.find((d) => d.id === 'district_3')?.captured ?? false;

  const hasWorkshop = state.run.plots.some(
    (p) => p.facilityId === 'parts_workshop' && p.level > 0,
  );
  const hasDojo = state.run.plots.some(
    (p) => p.facilityId === 'training_dojo' && p.level > 0,
  );
  const hasDepth1 = Boolean(state.run.clearedDepths['depth_1']);
  const hasDepth2 = Boolean(state.run.clearedDepths['depth_2']);

  const hasAdvancedGearOrSkill = state.run.roster.some(
    (op) =>
      op.unlockedNodeIds.length > 0 ||
      Boolean(op.equipment.weapon || op.equipment.armor || op.equipment.module),
  );

  const next: UnfoldingState = {
    baseUnlocked: cur.baseUnlocked || dist1 || cur.unlockedAllDebug === true,
    workshopUnlocked:
      cur.workshopUnlocked || hasWorkshop || dist1 || cur.unlockedAllDebug === true,
    otherworldUnlocked:
      cur.otherworldUnlocked ||
      dist2 ||
      state.run.resources.parts >= 12 ||
      hasDepth1 ||
      cur.unlockedAllDebug === true,
    dojoUnlocked: cur.dojoUnlocked || hasDojo || dist2 || cur.unlockedAllDebug === true,
    tacticsGuided:
      cur.tacticsGuided ||
      hasAdvancedGearOrSkill ||
      hasDepth1 ||
      cur.unlockedAllDebug === true,
    hqUnlocked:
      cur.hqUnlocked ||
      dist3 ||
      hasDepth2 ||
      state.run.resources.intel >= 25 ||
      state.hq.story.cycleCount > 0 ||
      cur.unlockedAllDebug === true,
    unlockedAllDebug: cur.unlockedAllDebug,
  };

  if (
    next.baseUnlocked === cur.baseUnlocked &&
    next.workshopUnlocked === cur.workshopUnlocked &&
    next.otherworldUnlocked === cur.otherworldUnlocked &&
    next.dojoUnlocked === cur.dojoUnlocked &&
    next.tacticsGuided === cur.tacticsGuided &&
    next.hqUnlocked === cur.hqUnlocked
  ) {
    return state;
  }

  return {
    ...state,
    hq: {
      ...state.hq,
      story: {
        ...state.hq.story,
        unfolding: next,
      },
    },
  };
}

export interface NextStepGuide {
  title: string;
  detail: string;
  targetTab?: 'expedition' | 'formation' | 'operatives' | 'base' | 'hq';
}

export function getNextStepGuide(state: GameState): NextStepGuide {
  const u = state.hq.story.unfolding;
  const dist1 = state.run.districts.find((d) => d.id === 'district_1')?.captured ?? false;
  const dist2 = state.run.districts.find((d) => d.id === 'district_2')?.captured ?? false;
  const hasWorkshop = state.run.plots.some((p) => p.facilityId === 'parts_workshop');
  const hasSalvage = state.run.plots.some((p) => p.facilityId === 'salvage_yard');
  const hasAnyPatrol = state.run.squads.some((s) => Boolean(s.assignedDepthId));
  const hasUnspentSp = state.run.roster.some((op) => op.skillPoints > 0);

  if (!dist1) {
    return {
      title: '当前目标：夺回一环 · 老商业街口',
      detail: '点击下方「一环 · 老商业街口」发起首次攻坚，打响收复街道与建立据点的第一步。',
      targetTab: 'expedition',
    };
  }
  if (!hasSalvage || !hasWorkshop) {
    return {
      title: '当前目标：建设街区产线与驻扎人手',
      detail: '前往「据点基建」在空地上建造「废材分类站」或「精密改装工坊」，开始产出精工零件并开放异界探测。',
      targetTab: 'base',
    };
  }
  if (hasUnspentSp) {
    return {
      title: '提示：有调查员获得了技能点 (SP)',
      detail: '前往「调查员」界面点亮职业星盘技能树节点，或到「战术编队」调整 AP/PP 条件优先级。',
      targetTab: 'operatives',
    };
  }
  if (u.otherworldUnlocked && !state.run.clearedDepths['depth_1']) {
    return {
      title: '当前目标：撕开异界浅层褶皱',
      detail: '点击「浅层褶皱 · 灰雾立交」发起首通攻坚，解锁挂机巡逻刷宝与特别队员线索。',
      targetTab: 'expedition',
    };
  }
  if (state.run.clearedDepths['depth_1'] && !hasAnyPatrol) {
    return {
      title: '当前目标：指派小队挂机巡逻',
      detail: '在「行动前线」为已打通的异界深度指派小队驻点巡逻，离线也能持续开箱刷装备。',
      targetTab: 'expedition',
    };
  }
  if (!dist2) {
    return {
      title: '当前目标：向二环 · 旧变电站推进',
      detail: '用零件锻造装备或特训队员后，攻克二环以解锁「联合演武室（技能继承）」与第 2 小队。',
      targetTab: 'expedition',
    };
  }
  if (!u.hqUnlocked) {
    return {
      title: '当前目标：深入异界与三环钟表匠巷',
      detail: '积累异常情报至 25 或攻克二层异界/三环街区，开启「组织总部与大渗透」轮回体系。',
      targetTab: 'expedition',
    };
  }
  return {
    title: '组织运转正常：继续扩张防线或收束轮回',
    detail: '一边用联合演武室传承强力技能与词条，一边在推不动更深异界时随时前往总部迎接大渗透。',
    targetTab: 'hq',
  };
}

export function appendTicker(ticker: string[], message: string): string[] {
  const next = [message, ...ticker];
  return next.slice(0, 8);
}
