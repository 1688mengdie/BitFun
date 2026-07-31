/**
 * WorkshopDAG — 工坊界面组件
 *
 * 展示四大工坊（天机/金算/丹青/留影）+ 选中工坊的 DAG 工作流。
 * 参考 beautiful-skill-tree 节点串联 + agent-town 区域概念。
 */

import React, { useState } from 'react';
import { MOCK_WORKSHOPS, MOCK_WORKSHOP_DAG } from './lvpa-mock-data';
import type { WorkshopDef, DagNode } from './lvpa-types';
import './WorkshopDAG.scss';

const STATUS_LABEL: Record<string, string> = {
  running: '运转中',
  paused: '暂停',
  idle: '空闲',
};

const NODE_STATUS_LABEL: Record<string, string> = {
  pending: '待处理',
  running: '进行中',
  done: '已完成',
  error: '异常',
};

interface DagNodeBoxProps {
  node: DagNode;
}

const DagNodeBox: React.FC<DagNodeBoxProps> = React.memo(({ node }) => (
  <div className={`workshop-dag__dag-node-box workshop-dag__dag-node-box--${node.status} workshop-dag__dag-node-box--${node.type}`}>
    <div>{node.label}</div>
    <div className="workshop-dag__dag-node-progress">{NODE_STATUS_LABEL[node.status]}</div>
  </div>
));

DagNodeBox.displayName = 'DagNodeBox';

export const WorkshopDAG: React.FC = () => {
  const [selectedId, setSelectedId] = useState<WorkshopDef['id']>('tianji');

  const selected = MOCK_WORKSHOPS.find((w) => w.id === selectedId);
  const dag = selectedId ? MOCK_WORKSHOP_DAG[selectedId] : undefined;

  return (
    <div className="workshop-dag">
      <div className="workshop-dag__header">
        <h2 className="workshop-dag__title">工坊</h2>
        <p className="workshop-dag__subtitle">固定工作流体系。点击工坊卡片查看工作流 DAG</p>
      </div>

      {/* 工坊卡片列表 */}
      <div className="workshop-dag__workshops">
        {MOCK_WORKSHOPS.map((ws) => (
          <div
            key={ws.id}
            className={`workshop-dag__card workshop-dag__card--${ws.status} ${
              selectedId === ws.id ? 'workshop-dag__card--selected' : ''
            }`}
            onClick={() => setSelectedId(ws.id)}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => e.key === 'Enter' && setSelectedId(ws.id)}
          >
            <div className="workshop-dag__card-top">
              <span className="workshop-dag__card-icon">{ws.icon}</span>
              <div className="workshop-dag__card-info">
                <div>
                  <span className="workshop-dag__card-name">{ws.nameCN}</span>
                  <span className="workshop-dag__card-namecn">{ws.name}</span>
                </div>
                <div className="workshop-dag__card-desc">{ws.description}</div>
              </div>
            </div>
            <div className="workshop-dag__card-meta">
              <span>成员 {ws.memberCount}人</span>
              <span className={`workshop-dag__card-status workshop-dag__card-status--${ws.status}`}>
                {STATUS_LABEL[ws.status]}
              </span>
            </div>
            {ws.currentProject && ws.progressPct !== undefined && (
              <div className="workshop-dag__card-progress">
                <div className="workshop-dag__progress-bar">
                  <div
                    className="workshop-dag__progress-fill"
                    style={{ width: `${ws.progressPct}%` }}
                  />
                </div>
                <div className="workshop-dag__progress-label">
                  {ws.currentProject} · {ws.progressPct}%
                </div>
              </div>
            )}
          </div>
        ))}
      </div>

      {/* DAG 流程图 */}
      <div className="workshop-dag__dag">
        <h3 className="workshop-dag__dag-title">
          {selected?.nameCN ?? selectedId} — 工作流
        </h3>

        {dag ? (
          <div className="workshop-dag__dag-flow">
            {dag.nodes.map((node, idx) => (
              <React.Fragment key={node.id}>
                {idx > 0 && (
                  <div className="workshop-dag__dag-arrow">→</div>
                )}
                <DagNodeBox node={node} />
              </React.Fragment>
            ))}
          </div>
        ) : (
          <div className="workshop-dag__dag-empty">
            该工坊暂无工作流定义
          </div>
        )}
      </div>
    </div>
  );
};

export default WorkshopDAG;
