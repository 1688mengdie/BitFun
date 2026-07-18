import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  agentAPI,
  type PermissionReplyKind,
  type PermissionRequestEvent,
  type PermissionV2Request,
} from '@/infrastructure/api/service-api/AgentAPI';

export function usePermissionRequests(sessionId?: string) {
  const [requests, setRequests] = useState<PermissionV2Request[]>([]);
  const resolvedIds = useRef(new Set<string>());

  useEffect(() => {
    let disposed = false;
    const unlisten = agentAPI.onPermissionRequestEvent((event: PermissionRequestEvent) => {
      if (disposed) return;
      setRequests((current) => {
        if (event.event === 'asked') {
          resolvedIds.current.delete(event.request.requestId);
          return current.some((request) => request.requestId === event.request.requestId)
            ? current.map((request) =>
                request.requestId === event.request.requestId ? event.request : request,
              )
            : [...current, event.request];
        }
        resolvedIds.current.add(event.requestId);
        return current.filter((request) => request.requestId !== event.requestId);
      });
    });

    void (async () => {
      try {
        await agentAPI.subscribePermissionRequests();
        const pending = await agentAPI.listPendingPermissionRequests();
        if (!disposed) {
          setRequests(pending.filter((request) => !resolvedIds.current.has(request.requestId)));
        }
      } catch {
        if (!disposed) setRequests([]);
      }
    })();

    return () => {
      disposed = true;
      unlisten();
    };
  }, []);

  const respond = useCallback(
    async (requestId: string, reply: PermissionReplyKind, feedback?: string) => {
      await agentAPI.respondPermission(requestId, reply, feedback);
      resolvedIds.current.add(requestId);
      setRequests((current) => current.filter((request) => request.requestId !== requestId));
    },
    [],
  );

  const sessionRequests = useMemo(
    () => requests.filter((request) => !sessionId || request.sessionId === sessionId),
    [requests, sessionId],
  );

  return { requests: sessionRequests, respond };
}
