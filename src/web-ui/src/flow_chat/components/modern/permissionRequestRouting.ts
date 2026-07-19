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
  return requests.filter((request) => permissionRequestBelongsToSession(request, sessionId));
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
