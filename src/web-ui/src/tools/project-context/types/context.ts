/**
 * Project context panel types for categories and documents.
 * Supports a VS Code-style collapsible group layout.
 *
 * Note: built-in document definitions moved to
 * crates/core/src/service/project_context/builtin_documents.rs.
 */

import { LucideIcon } from 'lucide-react';

export type CategoryId = 'general' | 'coding' | 'design' | 'review' | string;

export type DocumentPriority = 'high' | 'medium' | 'low';

export interface ContextCategory {
  /** Unique category ID */
  id: CategoryId;
  /** Display name (i18n key) */
  name: string;
  /** Icon name (lucide-react) */
  icon: string;
  /** Description (i18n key) */
  description: string;
  /** Sort order */
  order: number;
  /** Built-in category flag */
  isBuiltin?: boolean;
  /** Expanded state in UI */
  isExpanded?: boolean;
}

export interface ContextDocument {
  /** Unique document ID */
  id: string;
  /** File name */
  name: string;
  /** Category ID */
  categoryId: CategoryId;
  /** File path (null when not created) */
  filePath: string | null;
  /** File exists */
  exists: boolean;
  /** Enabled for context injection */
  enabled: boolean;
  /** AI generation support */
  canGenerate: boolean;
  /** Display priority */
  priority: DocumentPriority;
  /** Token count estimate */
  tokenCount?: number;
  /** User-defined (added on frontend) */
  isCustom?: boolean;
  /** Runtime description for UI */
  description?: string;
}

export interface CategoryDocuments {
  /** Existing documents */
  existing: ContextDocument[];
  /** Missing documents (listed under "view more") */
  missing: ContextDocument[];
}

export interface ProjectContextPanelConfig {
  /** Categories */
  categories: ContextCategory[];
  /** Expanded category state */
  expandedCategories: Set<CategoryId>;
  /** Enabled document IDs */
  enabledDocuments: Set<string>;
  /** Show low priority docs by default */
  showLowPriorityByDefault: boolean;
}

export interface ProjectContextConfig {
  /** Document enabled map: doc_id -> enabled */
  enabledDocuments: Record<string, boolean>;
}

/**
 * Built-in categories for UI icon mapping and default expansion.
 * Name and description are i18n keys resolved by the translator.
 */
export const BUILTIN_CATEGORIES: ContextCategory[] = [
  {
    id: 'general',
    name: 'builtinCategories.general.name',
    icon: 'FileText',
    isBuiltin: true,
    isExpanded: true,
    order: 0,
    description: 'builtinCategories.general.description'
  },
  {
    id: 'coding',
    name: 'builtinCategories.coding.name',
    icon: 'Code',
    isBuiltin: true,
    isExpanded: true,
    order: 1,
    description: 'builtinCategories.coding.description'
  },
  {
    id: 'design',
    name: 'builtinCategories.design.name',
    icon: 'Boxes',
    isBuiltin: true,
    isExpanded: false,
    order: 2,
    description: 'builtinCategories.design.description'
  },
  {
    id: 'review',
    name: 'builtinCategories.review.name',
    icon: 'GitPullRequest',
    isBuiltin: true,
    isExpanded: false,
    order: 3,
    description: 'builtinCategories.review.description'
  }
];

/**
 * Returns the category icon component.
 */
export function getCategoryIcon(_iconName: string): LucideIcon | null {
  // Dynamic import is handled at runtime.
  return null;
}
