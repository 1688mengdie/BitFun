import type { SessionMetadata } from '@/shared/types/session-history';
import type { Session } from '../types/flow-chat';

export interface SessionTitleDescriptor {
  source: 'text' | 'i18n';
  text: string;
  key?: string;
  params?: Record<string, unknown>;
}

type TranslateFn = (key: string, options?: Record<string, unknown>) => string;

const DEFAULT_SESSION_TITLE_KEY = 'flow-chat:session.new';

function normalizeTitleText(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() ? value.trim() : undefined;
}

function normalizeTitleParams(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

export function createTextSessionTitleDescriptor(text: string): SessionTitleDescriptor {
  return {
    source: 'text',
    text: normalizeTitleText(text) || '',
  };
}

export function createI18nSessionTitleDescriptor(
  key: string,
  translate: TranslateFn,
  params?: Record<string, unknown>,
): SessionTitleDescriptor {
  return {
    source: 'i18n',
    text: translate(key, params),
    key,
    params,
  };
}

export function createDefaultSessionTitleDescriptor(
  mode: 'code' | 'cowork' | 'claw',
  count: number,
  translate: TranslateFn,
): SessionTitleDescriptor {
  const key =
    mode === 'cowork'
      ? 'flow-chat:session.newCoworkWithIndex'
      : mode === 'claw'
        ? 'flow-chat:session.newClawWithIndex'
        : 'flow-chat:session.newCodeWithIndex';
  return createI18nSessionTitleDescriptor(key, translate, { count });
}

export function deriveSessionTitleState(
  descriptor?: SessionTitleDescriptor,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  if (descriptor?.source === 'i18n' && normalizeTitleText(descriptor.key)) {
    return {
      title: normalizeTitleText(descriptor.text),
      titleSource: 'i18n',
      titleI18nKey: descriptor.key,
      titleI18nParams: descriptor.params,
    };
  }

  return {
    title: normalizeTitleText(descriptor?.text),
    titleSource: 'text',
    titleI18nKey: undefined,
    titleI18nParams: undefined,
  };
}

export function freezeSessionTitleState(
  title: string,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  return {
    title: normalizeTitleText(title),
    titleSource: 'text',
    titleI18nKey: undefined,
    titleI18nParams: undefined,
  };
}

export function deriveSessionTitleStateFromMetadata(
  metadata?: Pick<SessionMetadata, 'sessionName' | 'customMetadata' | 'turnCount'> | null,
): Pick<Session, 'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'> {
  const titleKey = normalizeTitleText(metadata?.customMetadata?.titleKey);
  const useDynamicTitle =
    metadata?.customMetadata?.titleSource === 'i18n' &&
    Boolean(titleKey) &&
    (metadata?.turnCount ?? 0) === 0;

  return {
    title: normalizeTitleText(metadata?.sessionName),
    titleSource: useDynamicTitle ? 'i18n' : 'text',
    titleI18nKey: useDynamicTitle ? titleKey : undefined,
    titleI18nParams: useDynamicTitle
      ? normalizeTitleParams(metadata?.customMetadata?.titleParams)
      : undefined,
  };
}

export function resolveSessionTitle(
  session: Pick<
    Session,
    'title' | 'titleSource' | 'titleI18nKey' | 'titleI18nParams'
  > | null | undefined,
  translate: TranslateFn,
  fallbackKey: string = DEFAULT_SESSION_TITLE_KEY,
): string {
  if (session?.titleSource === 'i18n' && normalizeTitleText(session.titleI18nKey)) {
    return translate(session.titleI18nKey!, session.titleI18nParams);
  }

  return normalizeTitleText(session?.title) || translate(fallbackKey);
}

export function resolvePersistedSessionTitle(
  metadata: Pick<SessionMetadata, 'sessionName' | 'customMetadata' | 'turnCount'> | null | undefined,
  translate: TranslateFn,
  fallbackKey: string = DEFAULT_SESSION_TITLE_KEY,
): string {
  const titleKey = normalizeTitleText(metadata?.customMetadata?.titleKey);
  if (
    metadata?.customMetadata?.titleSource === 'i18n' &&
    titleKey &&
    (metadata?.turnCount ?? 0) === 0
  ) {
    return translate(titleKey, normalizeTitleParams(metadata?.customMetadata?.titleParams));
  }

  return normalizeTitleText(metadata?.sessionName) || translate(fallbackKey);
}
