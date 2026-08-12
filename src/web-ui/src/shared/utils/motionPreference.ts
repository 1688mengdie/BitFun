const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

export type MotionAwareScrollBehavior = 'auto' | 'smooth';

export function isReducedMotionPreferred(): boolean {
  return globalThis.matchMedia?.(REDUCED_MOTION_QUERY).matches ?? false;
}

/**
 * Preserve occasional spatial navigation while respecting reduced motion.
 * Automatic, keyboard-repeated, and other high-frequency paths should pass
 * `auto` directly instead of opting into smooth scrolling.
 */
export function getMotionAwareScrollBehavior(
  preferredBehavior: MotionAwareScrollBehavior,
): MotionAwareScrollBehavior {
  if (preferredBehavior !== 'smooth') {
    return preferredBehavior;
  }

  return isReducedMotionPreferred()
    ? 'auto'
    : preferredBehavior;
}
