import type { Session } from '../types/flow-chat';
import type { SessionMetadata } from '@/shared/types/session-history';
import { normalizeOrphanKind } from './sessionMetadata';
import { isSamePath, normalizeRemoteWorkspacePath } from '@/shared/utils/pathUtils';

/** Extract `host` from SSH connection IDs, with or without the legacy port suffix. */
function hostFromSshConnectionId(connectionId: string): string | null {
  const t = connectionId.trim();
  const m = t.match(/^ssh-[^@]+@(.+?)(?::\d+)?$/);
  return m ? m[1].trim().toLowerCase() : null;
}

/** Row-level SSH host: prefer workspace metadata, else parse from `connectionId` (sidebar may lack `sshHost`). */
function effectiveWorkspaceSshHost(
  remoteSshHost?: string | null,
  remoteConnectionId?: string | null
): string {
  const h = remoteSshHost?.trim().toLowerCase() ?? '';
  if (h) return h;
  return hostFromSshConnectionId(remoteConnectionId?.trim() ?? '') ?? '';
}

/**
 * Whether a persisted session belongs to a nav row for this workspace.
 * Remote workspaces are scoped by **SSH host + normalized remote root** (and connection id when present).
 * We must never treat "same host" as sufficient: two tabs to the same server at `/a` vs `/b` are distinct.
 */
export function sessionBelongsToWorkspaceNavRow(
  session: Pick<
    Session,
    'workspacePath' | 'projectWorkspacePath' | 'remoteConnectionId' | 'remoteSshHost' | 'workspaceHostname'
  >,
  workspacePath: string,
  remoteConnectionId?: string | null,
  remoteSshHost?: string | null
): boolean {
  const sessionRoot = session.workspacePath?.trim();
  const projectRoot = session.projectWorkspacePath?.trim();
  const pathsMatch =
    Boolean(
      sessionRoot &&
      (
        isSamePath(sessionRoot, workspacePath) ||
        normalizeRemoteWorkspacePath(sessionRoot) === normalizeRemoteWorkspacePath(workspacePath)
      )
    ) ||
    Boolean(
      projectRoot &&
      (
        isSamePath(projectRoot, workspacePath) ||
        normalizeRemoteWorkspacePath(projectRoot) === normalizeRemoteWorkspacePath(workspacePath)
      )
    );

  const wsConn = remoteConnectionId?.trim() ?? '';
  const sessConn = session.remoteConnectionId?.trim() ?? '';
  const wsHostEff = effectiveWorkspaceSshHost(remoteSshHost, remoteConnectionId);
  const sessHost =
    session.remoteSshHost?.trim().toLowerCase() ||
    session.workspaceHostname?.trim().toLowerCase() ||
    '';
  const sessConnHost = hostFromSshConnectionId(sessConn);
  const wsConnHost = hostFromSshConnectionId(wsConn);

  if (wsHostEff.length > 0) {
    // Host match alone is insufficient (same server, different remote folders).
    if (sessHost === wsHostEff && pathsMatch) {
      return true;
    }
    if (sessConnHost === wsHostEff && pathsMatch) {
      return true;
    }
    if (sessConnHost && wsConnHost && sessConnHost === wsConnHost) {
      return pathsMatch;
    }
  }

  if (!pathsMatch) return false;

  if (wsConn.length > 0 || sessConn.length > 0) {
    return sessConn === wsConn;
  }
  return true;
}

export function getSessionSortTimestamp(session: Pick<Session, 'createdAt' | 'lastFinishedAt'>): number {
  return session.lastFinishedAt ?? session.createdAt;
}

export function compareSessionsForDisplay(
  a: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>,
  b: Pick<Session, 'sessionId' | 'createdAt' | 'lastFinishedAt'>
): number {
  const timestampDiff = getSessionSortTimestamp(b) - getSessionSortTimestamp(a);
  if (timestampDiff !== 0) {
    return timestampDiff;
  }

  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return a.sessionId.localeCompare(b.sessionId);
}

export function getSessionMetadataSortTimestamp(
  session: Pick<SessionMetadata, 'createdAt' | 'lastFinishedAt' | 'customMetadata'>
): number {
  const lastFinishedAt = session.lastFinishedAt ?? session.customMetadata?.lastFinishedAt;
  return typeof lastFinishedAt === 'number' ? lastFinishedAt : session.createdAt;
}

export function compareSessionMetadataForDisplay(
  a: Pick<SessionMetadata, 'sessionId' | 'createdAt' | 'lastFinishedAt' | 'customMetadata'>,
  b: Pick<SessionMetadata, 'sessionId' | 'createdAt' | 'lastFinishedAt' | 'customMetadata'>
): number {
  const timestampDiff = getSessionMetadataSortTimestamp(b) - getSessionMetadataSortTimestamp(a);
  if (timestampDiff !== 0) {
    return timestampDiff;
  }

  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return a.sessionId.localeCompare(b.sessionId);
}

/**
 * Left-nav session list order: newest-created first, stable while switching sessions
 * (does not use `lastActiveAt`, so rows do not jump to the top on click).
 * R-AD-08: sessions flagged as orphaned sort after normal top-level sessions so
 * they collect under the orphan section instead of mingling with active work.
 */
export function compareSessionsForNavStable(
  a: Pick<Session, 'sessionId' | 'createdAt' | 'orphaned' | 'orphanKind'>,
  b: Pick<Session, 'sessionId' | 'createdAt' | 'orphaned' | 'orphanKind'>
): number {
  const orphanRankDiff = orphanSessionSortRank(a) - orphanSessionSortRank(b);
  if (orphanRankDiff !== 0) {
    return orphanRankDiff;
  }

  const createdAtDiff = b.createdAt - a.createdAt;
  if (createdAtDiff !== 0) {
    return createdAtDiff;
  }

  return a.sessionId.localeCompare(b.sessionId);
}

/**
 * Orphan-sort rank for a session. Non-orphans share rank 0 and keep the
 * existing ordering among themselves; orphans rank after them (1) so a
 * dedicated orphan section can render as a trailing group. The orphan kind
 * does not affect ordering — it only labels the row.
 */
export function orphanSessionSortRank(
  session: Pick<Session, 'orphaned' | 'orphanKind'>
): number {
  return session?.orphaned === true ? 1 : 0;
}

export function isOrphanSession(
  session: Pick<Session, 'orphaned' | 'orphanKind'>
): boolean {
  return session?.orphaned === true;
}

export function resolveSessionOrphanKind(
  session: Pick<Session, 'orphaned' | 'orphanKind'>
): Session['orphanKind'] {
  return isOrphanSession(session) ? normalizeOrphanKind(session?.orphanKind) : undefined;
}

/** Metadata-page variant used when building orphan stats before sessions hydrate. */
export function isOrphanMetadata(
  metadata: Pick<SessionMetadata, 'orphaned' | 'orphanKind'>
): boolean {
  return metadata?.orphaned === true;
}

/**
 * R-WF-12: whether a session is a group chat (group chat = ordinary session
 * with `isGroupChat` set, v3 decision). Used to partition nav rows: group
 * chats render only in the dedicated group-chats section, never mixed into
 * the plain assistant session list.
 */
export function sessionIsGroupChat(
  session: Pick<Session, 'isGroupChat'> | null | undefined
): boolean {
  return session?.isGroupChat === true;
}

/**
 * R-WF-12: whether a session is a workflow member Claw (a Claw spawned as a
 * member of a group/workflow; backend marks it via `customMetadata.legionNodeId`,
 * legion_control_tool.rs). The UI mirrors that marker into `Session.workflowMember`
 * during metadata restore so the Claw list can hide these workflow-owned Claws
 * (they belong to the group, not to the user's Claw assistant list).
 */
export function sessionIsWorkflowMember(
  session: Pick<Session, 'workflowMember'> | null | undefined
): boolean {
  return session?.workflowMember === true;
}

/**
 * R-8 group chat tree (GROUP P5): whether a session belongs to the group chat
 * tree view.
 *
 * A group chat = a special conversation hung on a tree UI: the group session
 * (isGroupChat) is the top-level node and its member sessions render as subtree
 * children. Member identity = consuming the member parent chain: the J1 case-B
 * revised backend dual-writes lineage for every member in
 * create_group_from_preset - relationship.parent_session_id = group_id +
 * customMetadata.parentSessionId = group_id (group_room_tools.rs). The frontend
 * deriveSessionRelationshipFromMetadata (sessionMetadata.ts:184-228) restores
 * session.parentSessionId = group_id. So a member whose parent chain lands in
 * `groupIds` (all group session ids in this scope) belongs to the group chat tree.
 *
 * NOTE identity: only the parent chain = group id counts; never mix in
 * `workflowMember` (customMetadata.legionNodeId, legion_control_tool.rs:1323) -
 * that is a legion workflow member marker, a different source from group tree
 * members (preset group members do not write legionNodeId, only parentSessionId).
 * Mixing misclassifies group tree members as orphaned -> drops child nodes.
 *
 * @param session session to test
 * @param groupIds all group session ids in this scope
 */
export function isGroupChatTreeNode(
  session: Session | null | undefined,
  groupIds: ReadonlySet<string>
): boolean {
  if (!session) {
    return false;
  }
  if (sessionIsGroupChat(session)) {
    return true;
  }
  // Group tree member parent chain = group_id. Member sessionKind = 'normal', so
  // the parent lives in session.parentSessionId (FlowChatStore restores it via
  // deriveSessionRelationshipFromMetadata reading customMetadata.parentSessionId /
  // relationship.parent_session_id, :7296).
  // NOTE: never re-derive via resolveSessionRelationship(session): that normalizes
  // via normalizeSessionRelationship which strips parentSessionId for
  // sessionKind='normal' (:104-113) -> parent chain reads empty -> member judged
  // non-tree-node -> child nodes dropped.
  const parentSessionId = session.parentSessionId ?? session.btwOrigin?.parentSessionId;
  return Boolean(parentSessionId && groupIds.has(parentSessionId));
}

