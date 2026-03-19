/**
 * Keyboard shortcut configuration (modifier + key).
 * Used by ShortcutManager for global shortcut registration.
 */
export interface ShortcutConfig {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
}
