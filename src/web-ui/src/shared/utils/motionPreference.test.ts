import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getMotionAwareScrollBehavior,
  isReducedMotionPreferred,
} from './motionPreference';

describe('getMotionAwareScrollBehavior', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('keeps smooth scrolling when reduced motion is not requested', () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: false })));

    expect(getMotionAwareScrollBehavior('smooth')).toBe('smooth');
  });

  it('uses immediate scrolling when reduced motion is requested', () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true })));

    expect(getMotionAwareScrollBehavior('smooth')).toBe('auto');
  });

  it('does not query motion preferences for an immediate scroll', () => {
    const matchMedia = vi.fn(() => ({ matches: true }));
    vi.stubGlobal('matchMedia', matchMedia);

    expect(getMotionAwareScrollBehavior('auto')).toBe('auto');
    expect(matchMedia).not.toHaveBeenCalled();
  });

  it('reports the current reduced-motion preference', () => {
    vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: true })));

    expect(isReducedMotionPreferred()).toBe(true);
  });

  it('defaults to full motion when matchMedia is unavailable', () => {
    vi.stubGlobal('matchMedia', undefined);

    expect(isReducedMotionPreferred()).toBe(false);
  });
});
