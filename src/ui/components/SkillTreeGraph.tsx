import type { SkillTreeNodeDef } from '../../types/game.ts';
import { GameTooltip } from './GameTooltip.tsx';
import { SkillTreeNodeTooltipCard } from './TooltipCards.tsx';

interface SkillTreeGraphProps {
  title: string;
  nodes: SkillTreeNodeDef[];
  unlockedIds: string[];
  availableSp: number;
  origin?: { name: string; icon: string; x: number; y: number };
  viewBoxWidth: number;
  viewBoxHeight: number;
  onUnlockNode: (nodeId: string) => void;
}

export function SkillTreeGraph({
  title,
  nodes,
  unlockedIds,
  availableSp,
  origin,
  viewBoxWidth,
  viewBoxHeight,
  onUnlockNode,
}: SkillTreeGraphProps) {
  const unlockedSet = new Set(unlockedIds);
  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  const edges: Array<{
    key: string;
    x1: number;
    y1: number;
    x2: number;
    y2: number;
    state: 'unlocked' | 'available' | 'locked';
  }> = [];

  for (const node of nodes) {
    if (node.prereqNodeIds.length === 0 && origin) {
      const state = unlockedSet.has(node.id)
        ? 'unlocked'
        : availableSp >= node.costSp
          ? 'available'
          : 'locked';
      edges.push({
        key: `origin_${node.id}`,
        x1: origin.x,
        y1: origin.y,
        x2: node.x,
        y2: node.y,
        state,
      });
    } else {
      for (const pid of node.prereqNodeIds) {
        const parent = nodeMap.get(pid);
        if (!parent) continue;
        const parentUnlocked = unlockedSet.has(parent.id);
        const childUnlocked = unlockedSet.has(node.id);
        const state =
          parentUnlocked && childUnlocked
            ? 'unlocked'
            : parentUnlocked
              ? 'available'
              : 'locked';
        edges.push({
          key: `${parent.id}_${node.id}`,
          x1: parent.x,
          y1: parent.y,
          x2: node.x,
          y2: node.y,
          state,
        });
      }
    }
  }

  return (
    <div className="skill-tree-panel">
      <div className="skill-tree-header">
        <strong>{title}</strong>
        <span className="pill cyan mono">
          已点亮 {nodes.filter((n) => unlockedSet.has(n.id)).length}/{nodes.length} · 可用 SP {availableSp}
        </span>
      </div>

      <div
        className="skill-tree-canvas-wrap"
        style={{ aspectRatio: `${viewBoxWidth} / ${viewBoxHeight}` }}
      >
        <svg
          className="skill-tree-svg"
          viewBox={`0 0 ${viewBoxWidth} ${viewBoxHeight}`}
          preserveAspectRatio="xMidYMid meet"
        >
          <defs>
            <pattern
              id={`grid_${viewBoxWidth}_${viewBoxHeight}`}
              width="32"
              height="32"
              patternUnits="userSpaceOnUse"
            >
              <path
                d="M 32 0 L 0 0 0 32"
                fill="none"
                stroke="rgba(56, 189, 248, 0.06)"
                strokeWidth="1"
              />
            </pattern>
          </defs>
          <rect
            width={viewBoxWidth}
            height={viewBoxHeight}
            fill={`url(#grid_${viewBoxWidth}_${viewBoxHeight})`}
          />

          <g className="edges">
            {edges.map((e) => {
              const mx = (e.x1 + e.x2) / 2;
              const d = `M ${e.x1} ${e.y1} C ${mx} ${e.y1}, ${mx} ${e.y2}, ${e.x2} ${e.y2}`;
              return (
                <path key={e.key} d={d} className={`tree-edge ${e.state}`} />
              );
            })}
          </g>
        </svg>

        {origin && (
          <div
            className="tree-node-anchor"
            style={{
              left: `${(origin.x / viewBoxWidth) * 100}%`,
              top: `${(origin.y / viewBoxHeight) * 100}%`,
            }}
          >
            <div className="medallion origin unlocked">
              <span className="medallion-icon">{origin.icon}</span>
            </div>
            <div className="medallion-label">{origin.name}</div>
          </div>
        )}

        {nodes.map((node) => {
          const isUnlocked = unlockedSet.has(node.id);
          const prereqMet = node.prereqNodeIds.every((pid) => unlockedSet.has(pid));
          const hasSp = availableSp >= node.costSp;

          const visualState = isUnlocked
            ? 'unlocked'
            : prereqMet && hasSp
              ? 'available'
              : prereqMet
                ? 'no_sp'
                : 'locked';

          const prereqNames = node.prereqNodeIds.map(
            (pid) => nodeMap.get(pid)?.name ?? pid,
          );

          return (
            <div
              key={node.id}
              className="tree-node-anchor"
              style={{
                left: `${(node.x / viewBoxWidth) * 100}%`,
                top: `${(node.y / viewBoxHeight) * 100}%`,
              }}
            >
              <GameTooltip
                as="div"
                content={
                  <SkillTreeNodeTooltipCard
                    node={node}
                    stateLabel={visualState}
                    prereqNames={prereqNames}
                  />
                }
              >
                <button
                  type="button"
                  className={`medallion ${node.isKeystone ? 'keystone' : 'standard'} ${
                    isUnlocked
                      ? 'unlocked'
                      : prereqMet && hasSp
                        ? 'available'
                        : prereqMet
                          ? 'prereq-met'
                          : 'locked'
                  }`}
                  onClick={() => {
                    if (visualState === 'available') {
                      onUnlockNode(node.id);
                    }
                  }}
                >
                  <span className="medallion-icon">{node.icon}</span>
                  <span className="medallion-cost mono">
                    {isUnlocked ? '✓' : `${node.costSp}`}
                  </span>
                </button>
              </GameTooltip>
              <div className={`medallion-label ${isUnlocked ? 'unlocked' : ''}`}>
                {node.name}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
