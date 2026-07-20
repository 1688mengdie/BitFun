import type {
  PermissionRequestEvent,
  PermissionV2Request,
} from '@/infrastructure/api/service-api/AgentAPI';

export function permissionRequestBelongsToSession(
  request: PermissionV2Request,
  sessionId?: string,
): boolean {
  if (!sessionId) return false;
  return request.sessionId === sessionId || request.delegation?.parentSessionId === sessionId;
}

export function selectPermissionRequestsForSession(
  requests: readonly PermissionV2Request[],
  sessionId?: string,
): PermissionV2Request[] {
  return sortPermissionRequests(
    requests.filter((request) => permissionRequestBelongsToSession(request, sessionId)),
  );
}

export interface PermissionRequestBatch {
  sessionId: string;
  roundId: string;
  requests: PermissionV2Request[];
}

export function selectActivePermissionBatch(
  requests: readonly PermissionV2Request[],
  sessionId?: string,
): PermissionRequestBatch | undefined {
  const routed = selectPermissionRequestsForSession(requests, sessionId);
  const first = routed[0];
  if (!first) return undefined;

  const batchRequests = routed.filter(
    (request) => request.sessionId === first.sessionId && request.roundId === first.roundId,
  );
  return {
    sessionId: first.sessionId,
    roundId: first.roundId,
    requests: batchRequests,
  };
}

/**
 * Keep permission requests in arrival order across rounds, while preserving
 * the model-provided order inside each round. The first-seen batch position is
 * used because round IDs are opaque (usually UUIDs) and are not chronological.
 */
export function sortPermissionRequests(
  requests: readonly PermissionV2Request[],
): PermissionV2Request[] {
  const firstBatchIndex = new Map<string, number>();
  requests.forEach((request, index) => {
    const batchId = `${request.sessionId}\u0000${request.roundId}`;
    if (!firstBatchIndex.has(batchId)) firstBatchIndex.set(batchId, index);
  });

  return [...requests].sort((left, right) => {
    const leftBatchId = `${left.sessionId}\u0000${left.roundId}`;
    const rightBatchId = `${right.sessionId}\u0000${right.roundId}`;
    const batchOrder =
      (firstBatchIndex.get(leftBatchId) ?? 0) - (firstBatchIndex.get(rightBatchId) ?? 0);
    if (batchOrder !== 0) return batchOrder;

    return left.order - right.order || left.requestId.localeCompare(right.requestId);
  });
}

export function pendingPermissionToolCallIdsForSession(
  requests: readonly PermissionV2Request[],
  sessionId?: string,
): ReadonlySet<string> {
  const toolCallIds = new Set<string>();
  if (!sessionId) return toolCallIds;

  for (const request of requests) {
    if (!permissionRequestBelongsToSession(request, sessionId)) continue;

    const toolCallId = request.sessionId === sessionId
      ? request.toolCallId
      : request.delegation?.parentToolCallId;
    if (toolCallId) toolCallIds.add(toolCallId);
  }

  return toolCallIds;
}

export function applyPermissionRequestEvent(
  requests: readonly PermissionV2Request[],
  event: PermissionRequestEvent,
): PermissionV2Request[] {
  if (event.event !== 'asked') {
    return requests.filter((request) => request.requestId !== event.requestId);
  }

  const existingIndex = requests.findIndex(
    (request) => request.requestId === event.request.requestId,
  );
  if (existingIndex < 0) return [...requests, event.request];

  const next = [...requests];
  next[existingIndex] = event.request;
  return next;
}

export function reconcilePermissionRequestSnapshot(
  current: readonly PermissionV2Request[],
  pending: readonly PermissionV2Request[],
  resolvedIds: ReadonlySet<string>,
): PermissionV2Request[] {
  const currentById = new Map(current.map((request) => [request.requestId, request]));
  const pendingIds = new Set<string>();
  const reconciled: PermissionV2Request[] = [];

  for (const request of pending) {
    if (resolvedIds.has(request.requestId)) continue;
    pendingIds.add(request.requestId);
    reconciled.push(currentById.get(request.requestId) ?? request);
  }

  for (const request of current) {
    if (!resolvedIds.has(request.requestId) && !pendingIds.has(request.requestId)) {
      reconciled.push(request);
    }
  }

  return reconciled;
}
