import React from 'react';
import { useTranslation } from 'react-i18next';
import { Target } from 'lucide-react';
import { IconButton, Tooltip } from '@/component-library';
import type { ThreadGoalSnapshot } from '../../services/goalService';

export interface ThreadGoalStripButtonProps {
  goal: ThreadGoalSnapshot | null;
  onOpen: () => void;
}

/** Strip icon tone: none = gray, active = yellow, complete = green. */
export type ThreadGoalStripIconTone = 'none' | 'active' | 'complete';

function normalizeThreadGoalStatus(status: string | undefined): string {
  const raw = status?.trim() ?? '';
  if (!raw) {
    return '';
  }
  const camel = raw.charAt(0).toLowerCase() + raw.slice(1);
  if (camel === 'usage_limited') {
    return 'usageLimited';
  }
  if (camel === 'budget_limited') {
    return 'budgetLimited';
  }
  return camel;
}

export function resolveThreadGoalStripIconTone(
  goal: ThreadGoalSnapshot | null,
): ThreadGoalStripIconTone {
  if (!goal) {
    return 'none';
  }
  if (normalizeThreadGoalStatus(goal.status) === 'complete') {
    return 'complete';
  }
  return 'active';
}

export const ThreadGoalStripButton: React.FC<ThreadGoalStripButtonProps> = ({
  goal,
  onOpen,
}) => {
  const { t } = useTranslation('flow-chat');

  const iconTone = resolveThreadGoalStripIconTone(goal);
  const statusKey = goal?.status ?? 'none';
  const tooltip = goal
    ? t('threadGoal.stripTooltipWithGoal', {
        status: t(`threadGoal.status.${statusKey}`, { defaultValue: statusKey }),
        objective: goal.objective,
      })
    : t('threadGoal.stripTooltipEmpty');

  const ariaLabel = goal ? t('threadGoal.stripOpenWithGoal') : t('threadGoal.stripOpenEmpty');

  return (
    <Tooltip content={tooltip}>
      <IconButton
        className={`bitfun-chat-input-workspace-strip__goal-btn bitfun-chat-input-workspace-strip__goal-btn--${iconTone}`}
        variant="ghost"
        size="xs"
        type="button"
        aria-label={ariaLabel}
        data-testid="thread-goal-strip-button"
        onClick={e => {
          e.stopPropagation();
          onOpen();
        }}
      >
        <Target size={14} strokeWidth={2} aria-hidden />
      </IconButton>
    </Tooltip>
  );
};

ThreadGoalStripButton.displayName = 'ThreadGoalStripButton';
