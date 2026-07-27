import type { SubscriptionProvider } from '../types';

/** 订阅登录操作 */
export interface SubscriptionLoginOperation {
  id: string;
  provider: SubscriptionProvider;
  sessionId: string;
  cancelled: boolean;
  /** 标记 startSubscriptionLogin 是否已经结算（成功或失败） */
  startSettled?: boolean;
}

/**
 * 订阅登录协调器
 * 管理模型提供商订阅账户的登录流程，确保同一时间只有一个登录流程
 */
export class SubscriptionLoginCoordinator {
  private currentOperation: SubscriptionLoginOperation | null = null;
  private operationCounter = 0;

  /** 开始一个新的登录操作。如果已有进行中的操作则返回 null */
  begin(provider: SubscriptionProvider): SubscriptionLoginOperation | null {
    if (this.currentOperation && !this.currentOperation.cancelled) {
      return null;
    }
    this.operationCounter += 1;
    this.currentOperation = {
      id: `login-${this.operationCounter}-${Date.now()}`,
      provider,
      sessionId: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2)}`,
      cancelled: false,
    };
    return this.currentOperation;
  }

  /** 检查当前操作是否仍然是传入的 operation */
  isCurrent(operation: SubscriptionLoginOperation): boolean {
    return this.currentOperation?.id === operation.id && !this.currentOperation.cancelled;
  }

  /** 返回当前操作 */
  current(): SubscriptionLoginOperation | null {
    return this.currentOperation;
  }

  /** 请求取消指定 provider 的操作，返回被取消的 operation（如果没有则返回 null） */
  requestCancel(provider: SubscriptionProvider): SubscriptionLoginOperation | null {
    if (this.currentOperation && this.currentOperation.provider === provider && !this.currentOperation.cancelled) {
      this.currentOperation.cancelled = true;
    }
    return this.currentOperation;
  }

  /** 检查是否拥有该 operation */
  owns(operation: SubscriptionLoginOperation): boolean {
    return this.currentOperation?.id === operation.id;
  }

  /** 标记开始阶段已完成 */
  markStartSettled(operation: SubscriptionLoginOperation): void {
    if (this.currentOperation?.id === operation.id) {
      this.currentOperation.startSettled = true;
    }
  }

  /** 标记登录完成，返回 true 表示成功完成 */
  complete(operation: SubscriptionLoginOperation): boolean {
    if (this.currentOperation?.id === operation.id) {
      this.currentOperation = null;
      return true;
    }
    return false;
  }
}

/** 取消登录错误 */
export function subscriptionLoginCancelledError(): Error {
  return new Error('Subscription login cancelled');
}

export interface SettlementResult {
  cleanupError?: Error;
  shouldContinue: boolean;
}

/**
 * 发起订阅登录流程的结算
 * 返回 { cleanupError, shouldContinue }
 */
export async function settleSubscriptionLoginStart(
  coordinator: SubscriptionLoginCoordinator,
  operation: SubscriptionLoginOperation,
  cancelFn: () => Promise<void>,
): Promise<SettlementResult> {
  try {
    if (!coordinator.isCurrent(operation)) {
      await cancelFn();
      return { shouldContinue: false };
    }
    coordinator.markStartSettled(operation);
    return { shouldContinue: true };
  } catch (error) {
    return {
      cleanupError: error instanceof Error ? error : new Error(String(error)),
      shouldContinue: false,
    };
  }
}
