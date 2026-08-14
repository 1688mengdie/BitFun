export class ThinkingPresentationPolicy {
  static isRunning(status: string): boolean {
    const normalized = (status || '').toLowerCase();
    return normalized === 'active' || normalized === 'running';
  }

  static shouldRender(status: string, text: string): boolean {
    if (ThinkingPresentationPolicy.isRunning(status)) {
      return true;
    }
    return text.trim().length > 0;
  }

  static defaultExpanded(isRunning: boolean, keepExpandedWhenDone: boolean): boolean {
    return isRunning || keepExpandedWhenDone;
  }
}
