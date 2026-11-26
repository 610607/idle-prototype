import type { TagId } from '../types/combat.ts';

export interface TagMeta {
  id: TagId;
  label: string;
  color: string;
  bgColor: string;
  maxStacks: number;
  description: string;
}

export const TAG_META: Record<TagId, TagMeta> = {
  shocked: {
    id: 'shocked',
    label: '[震荡]',
    color: '#fbbf24',
    bgColor: 'rgba(251, 191, 36, 0.16)',
    maxStacks: 3,
    description: '动作失衡：受到伤害 +10%/层，可触发友方追击步枪与协频打击。',
  },
  marked: {
    id: 'marked',
    label: '[标记]',
    color: '#60a5fa',
    bgColor: 'rgba(96, 165, 250, 0.16)',
    maxStacks: 2,
    description: '光学锁定：被狙击/追击命中时附加额外暴击系数。',
  },
  overclock: {
    id: 'overclock',
    label: '[过载]',
    color: '#f97316',
    bgColor: 'rgba(249, 115, 22, 0.18)',
    maxStacks: 3,
    description: '神经超频：主动攻击伤害 +15%/层，但每层额外承受 8% 伤害；配合冷凝站触发真伤与回血。',
  },
  erosion: {
    id: 'erosion',
    label: '[侵蚀]',
    color: '#c084fc',
    bgColor: 'rgba(192, 132, 252, 0.18)',
    maxStacks: 6,
    description: '异界浸染：回合初造成每层持续伤害，叠满或配合解析阵列可放大结晶收益。',
  },
  shielded: {
    id: 'shielded',
    label: '[护盾]',
    color: '#34d399',
    bgColor: 'rgba(52, 211, 153, 0.16)',
    maxStacks: 1,
    description: '折跃护盾：优先吸收伤害。',
  },
  fortified: {
    id: 'fortified',
    label: '[固守]',
    color: '#94a3b8',
    bgColor: 'rgba(148, 163, 184, 0.2)',
    maxStacks: 2,
    description: '战术掩护：减伤 25%。',
  },
};
