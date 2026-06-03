import type { Session } from '../types/flow-chat';

const ACP_AGENT_TYPE_PREFIX = 'acp:';

export function acpClientIdFromAgentType(agentType: string | null | undefined): string | null {
  const value = agentType?.trim();
  if (!value?.startsWith(ACP_AGENT_TYPE_PREFIX)) return null;

  const clientId = value.slice(ACP_AGENT_TYPE_PREFIX.length).trim();
  return clientId || null;
}

export function isAcpAgentType(agentType: string | null | undefined): boolean {
  return acpClientIdFromAgentType(agentType) !== null;
}

export function isAcpFlowSession(
  session: Pick<Session, 'config' | 'mode'> | null | undefined,
): boolean {
  return Boolean(
    isAcpAgentType(session?.config?.agentType) ||
    isAcpAgentType(session?.mode),
  );
}

export interface AcpSessionRef {
  sessionId: string;
  clientId: string;
  workspacePath?: string;
  remoteConnectionId?: string;
  remoteSshHost?: string;
}

export function acpSessionRef(
  session:
    | Pick<
        Session,
        'sessionId' | 'config' | 'mode' | 'workspacePath' | 'remoteConnectionId' | 'remoteSshHost'
      >
    | null
    | undefined,
): AcpSessionRef | null {
  if (!session?.sessionId) return null;

  const clientId =
    acpClientIdFromAgentType(session.config?.agentType) ??
    acpClientIdFromAgentType(session.mode);
  if (!clientId) return null;

  return {
    sessionId: session.sessionId,
    clientId,
    workspacePath: session.workspacePath ?? session.config?.workspacePath,
    remoteConnectionId: session.remoteConnectionId ?? session.config?.remoteConnectionId,
    remoteSshHost: session.remoteSshHost,
  };
}

export function acpSlashCommandText(name: string): string {
  return `/${name} `;
}
