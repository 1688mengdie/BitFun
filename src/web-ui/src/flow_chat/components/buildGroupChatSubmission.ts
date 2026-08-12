/**
 * Task A: normalize a ChatInputSubmission into the group chat message body and
 * mention targets. Group-member mentions arrive as session-reference contexts
 * carrying `metadata.groupChatMention` (R-GC-15 `@@`); their display capsules
 * become readable `@name` mentions in the body.
 *
 * Kept in its own file (not exported from GroupChatPane.tsx) so the component
 * module only exports components — Vite/React fast-refresh requires that.
 */

import type { GroupChatActor } from '../types/flow-chat';
import type { ChatInputSubmission } from './chatInputRegistration';
import type { SessionReferenceContext } from '@/shared/types/context';

export function buildGroupChatSubmission(
  submission: ChatInputSubmission,
  pendingTargets: GroupChatActor[] = [],
): { text: string; mentionTargets: GroupChatActor[] } {
  const displayText = (submission.displayText ?? submission.text).trim();
  const text = displayText.replace(/\[Session reference:\s*(.+?)\]/g, '@$1');
  const membersFromContexts = (submission.contexts ?? [])
    .filter((context): context is SessionReferenceContext =>
      context.type === 'session-reference' && context.metadata?.groupChatMention !== undefined)
    .map((context) => context.metadata?.groupChatMention as GroupChatActor)
    .filter((target): target is GroupChatActor => target !== undefined);
  // Dedupe by identity: @all is a single fixed target; members key on sessionId.
  const byKey = new Map<string, GroupChatActor>();
  for (const target of [...pendingTargets, ...membersFromContexts]) {
    byKey.set(target.kind === 'claw' ? `claw:${target.sessionId}` : target.kind, target);
  }
  return { text, mentionTargets: [...byKey.values()] };
}
