import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  Clock,
  Pause,
  Play,
  Settings,
  SkipForward,
} from 'lucide-react';
import { Button } from '@/component-library';
import type {
  DeepReviewCapacityQueueReason,
  DeepReviewCapacityQueueState,
} from '../../store/deepReviewActionBarStore';
import { formatElapsedTime } from './actionBarFormatting';

interface CapacityQueueNoticeProps {
  capacityQueueState: DeepReviewCapacityQueueState;
  supportsInlineQueueControls: boolean;
  onPauseQueue: () => void | Promise<void>;
  onContinueQueue: () => void | Promise<void>;
  onSkipOptionalQueuedReviewers: () => void | Promise<void>;
  onCancelQueuedReviewers: () => void | Promise<void>;
  onRunSlowerNextTime: () => void | Promise<void>;
  onOpenReviewSettings: () => void | Promise<void>;
}

const CAPACITY_QUEUE_REASON_KEYS: Record<DeepReviewCapacityQueueReason, string> = {
  provider_rate_limit: 'deepReviewActionBar.capacityQueue.reasons.providerRateLimit',
  provider_concurrency_limit: 'deepReviewActionBar.capacityQueue.reasons.providerConcurrencyLimit',
  retry_after: 'deepReviewActionBar.capacityQueue.reasons.retryAfter',
  local_concurrency_cap: 'deepReviewActionBar.capacityQueue.reasons.localConcurrencyCap',
  temporary_overload: 'deepReviewActionBar.capacityQueue.reasons.temporaryOverload',
};

export const CapacityQueueNotice: React.FC<CapacityQueueNoticeProps> = ({
  capacityQueueState,
  supportsInlineQueueControls,
  onPauseQueue,
  onContinueQueue,
  onSkipOptionalQueuedReviewers,
  onCancelQueuedReviewers,
  onRunSlowerNextTime,
  onOpenReviewSettings,
}) => {
  const { t } = useTranslation('flow-chat');
  const capacityQueueReasonLabel = capacityQueueState.reason
    ? t(CAPACITY_QUEUE_REASON_KEYS[capacityQueueState.reason], {
      defaultValue: capacityQueueState.reason.split('_').join(' '),
    })
    : null;
  const capacityQueueElapsedLabel = capacityQueueState.queueElapsedMs !== undefined
    ? formatElapsedTime(capacityQueueState.queueElapsedMs)
    : null;
  const capacityQueueMaxWaitLabel = capacityQueueState.maxQueueWaitSeconds !== undefined
    ? formatElapsedTime(capacityQueueState.maxQueueWaitSeconds * 1000)
    : null;

  return (
    <div className="deep-review-action-bar__capacity-queue" aria-live="polite">
      <div className="deep-review-action-bar__capacity-queue-main">
        <Clock size={14} className="deep-review-action-bar__capacity-queue-icon" />
        <div className="deep-review-action-bar__capacity-queue-copy">
          <span className="deep-review-action-bar__capacity-queue-title">
            {capacityQueueState.status === 'paused_by_user'
              ? t('deepReviewActionBar.capacityQueue.pausedTitle', {
                defaultValue: 'Queue paused',
              })
              : t('deepReviewActionBar.capacityQueue.title', {
                defaultValue: 'Reviewers waiting for capacity',
              })}
          </span>
          <span className="deep-review-action-bar__capacity-queue-detail">
            {t('deepReviewActionBar.capacityQueue.detail', {
              defaultValue: 'Queue wait does not count against reviewer runtime.',
            })}
          </span>
          {(capacityQueueReasonLabel || capacityQueueElapsedLabel) && (
            <span className="deep-review-action-bar__capacity-queue-meta">
              {capacityQueueReasonLabel && (
                <span className="deep-review-action-bar__capacity-queue-chip">
                  {t('deepReviewActionBar.capacityQueue.reason', {
                    reason: capacityQueueReasonLabel,
                    defaultValue: `Reason: ${capacityQueueReasonLabel}`,
                  })}
                </span>
              )}
              {capacityQueueElapsedLabel && (
                <span className="deep-review-action-bar__capacity-queue-chip">
                  {capacityQueueMaxWaitLabel
                    ? t('deepReviewActionBar.capacityQueue.elapsedWithMax', {
                      elapsed: capacityQueueElapsedLabel,
                      max: capacityQueueMaxWaitLabel,
                      defaultValue: `Waited ${capacityQueueElapsedLabel} of ${capacityQueueMaxWaitLabel}`,
                    })
                    : t('deepReviewActionBar.capacityQueue.elapsed', {
                      elapsed: capacityQueueElapsedLabel,
                      defaultValue: `Waited ${capacityQueueElapsedLabel}`,
                    })}
                </span>
              )}
            </span>
          )}
          {capacityQueueState.sessionConcurrencyHigh && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.sessionBusy', {
                defaultValue: 'Your active session is busy. Pause Deep Review or continue later.',
              })}
            </span>
          )}
          {!supportsInlineQueueControls && (
            <span className="deep-review-action-bar__capacity-queue-detail">
              {t('deepReviewActionBar.capacityQueue.stopHint', {
                defaultValue: 'Use Stop to interrupt this review queue.',
              })}
            </span>
          )}
        </div>
      </div>
      <div className="deep-review-action-bar__capacity-queue-actions">
        {supportsInlineQueueControls && (
          <>
            {capacityQueueState.status === 'paused_by_user' ? (
              <Button
                variant="secondary"
                size="small"
                onClick={() => void onContinueQueue()}
              >
                <Play size={13} />
                {t('deepReviewActionBar.capacityQueue.continueQueue', {
                  defaultValue: 'Continue queue',
                })}
              </Button>
            ) : (
              <Button
                variant="secondary"
                size="small"
                onClick={() => void onPauseQueue()}
              >
                <Pause size={13} />
                {t('deepReviewActionBar.capacityQueue.pauseQueue', {
                  defaultValue: 'Pause queue',
                })}
              </Button>
            )}
            {(capacityQueueState.optionalReviewerCount ?? 0) > 0 && (
              <Button
                variant="ghost"
                size="small"
                onClick={() => void onSkipOptionalQueuedReviewers()}
              >
                <SkipForward size={13} />
                {t('deepReviewActionBar.capacityQueue.skipOptionalQueued', {
                  defaultValue: 'Skip optional extras',
                })}
              </Button>
            )}
            <Button
              variant="ghost"
              size="small"
              onClick={() => void onCancelQueuedReviewers()}
            >
              {t('deepReviewActionBar.capacityQueue.cancelQueued', {
                defaultValue: 'Cancel queued reviewers',
              })}
            </Button>
          </>
        )}
        <Button
          variant="ghost"
          size="small"
          onClick={() => void onRunSlowerNextTime()}
        >
          <Settings size={13} />
          {t('deepReviewActionBar.capacityQueue.runSlowerNextTime', {
            defaultValue: 'Run slower next time',
          })}
        </Button>
        <Button
          variant="ghost"
          size="small"
          onClick={() => void onOpenReviewSettings()}
        >
          {t('deepReviewActionBar.capacityQueue.openReviewSettings', {
            defaultValue: 'Open Review settings',
          })}
        </Button>
      </div>
    </div>
  );
};
