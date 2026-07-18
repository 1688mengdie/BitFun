import { useState } from 'react';
import { Check, ShieldAlert, X } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type {
  PermissionReplyKind,
  PermissionV2Request,
} from '@/infrastructure/api/service-api/AgentAPI';
import './PermissionRequestPanel.scss';

interface PermissionRequestPanelProps {
  request: PermissionV2Request;
  onRespond: (reply: PermissionReplyKind, feedback?: string) => Promise<void>;
}

export function PermissionRequestPanel({ request, onRespond }: PermissionRequestPanelProps) {
  const { t } = useTranslation('flow-chat');
  const [feedback, setFeedback] = useState('');
  const [responding, setResponding] = useState(false);
  const [error, setError] = useState(false);
  const risk = [request.displayMetadata?.riskDescription, request.displayMetadata?.risk].find(
    (value): value is string => typeof value === 'string' && value.trim().length > 0,
  );

  const respond = async (reply: PermissionReplyKind) => {
    setResponding(true);
    setError(false);
    try {
      await onRespond(reply, reply === 'reject' ? feedback : undefined);
    } catch {
      setError(true);
    } finally {
      setResponding(false);
    }
  };

  return (
    <section className="permission-request-panel" aria-label={t('permissionV2.title')}>
      <div className="permission-request-panel__heading">
        <ShieldAlert size={18} aria-hidden="true" />
        <div>
          <h2>{t('permissionV2.title')}</h2>
          <p>
            {request.action} · {request.source.identity}
          </p>
        </div>
      </div>
      <div className="permission-request-panel__resources">
        {request.resources.map((resource, index) => (
          <code key={`${request.requestId}:${index}`}>{resource}</code>
        ))}
      </div>
      {risk && <p className="permission-request-panel__risk">{risk}</p>}
      <p className="permission-request-panel__scope">
        {request.saveResources?.length
          ? t('permissionV2.scope', { project: request.projectId })
          : t('permissionV2.scopeOnce')}
      </p>
      {error && <p role="alert">{t('permissionV2.responseFailed')}</p>}
      <textarea
        value={feedback}
        onChange={(event) => setFeedback(event.target.value)}
        placeholder={t('permissionV2.feedbackPlaceholder')}
        aria-label={t('permissionV2.feedbackLabel')}
        disabled={responding}
        rows={2}
      />
      <div className="permission-request-panel__actions">
        <button type="button" onClick={() => void respond('once')} disabled={responding}>
          <Check size={15} aria-hidden="true" /> {t('permissionV2.allowOnce')}
        </button>
        <button type="button" onClick={() => void respond('always')} disabled={responding}>
          <Check size={15} aria-hidden="true" /> {t('permissionV2.allowAlways')}
        </button>
        <button
          type="button"
          className="permission-request-panel__reject"
          onClick={() => void respond('reject')}
          disabled={responding}
        >
          <X size={15} aria-hidden="true" /> {t('permissionV2.reject')}
        </button>
      </div>
    </section>
  );
}
