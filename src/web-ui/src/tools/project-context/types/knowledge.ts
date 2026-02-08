/**
 * Knowledge base types.
 * Supports Skill and RAG variants.
 */

/**
 * Knowledge base types:
 * - skill: predefined skills/rules
 * - rag: retrieves knowledge through a query endpoint
 */
export type KnowledgeBaseType = 'skill' | 'rag';

export type KnowledgeBaseStatus = 'active' | 'inactive' | 'error' | 'syncing';

export interface SkillKnowledgeConfig {
  /** Skill file path (local file) */
  filePath?: string;
  /** Inline skill content */
  content?: string;
  /** Content format */
  format: 'markdown' | 'json' | 'yaml' | 'text';
  /** Auto-sync on file changes */
  autoSync?: boolean;
}

export interface RAGKnowledgeConfig {
  /** Query endpoint URL */
  endpoint: string;
  /** API key (optional) */
  apiKey?: string;
  /** Request headers (optional) */
  headers?: Record<string, string>;
  /** Query template */
  queryTemplate?: string;
  /** Response JSON path */
  responsePath?: string;
  /** Max results */
  maxResults?: number;
  /** Similarity threshold */
  similarityThreshold?: number;
}

export interface KnowledgeBase {
  /** Unique ID */
  id: string;
  /** Name */
  name: string;
  /** Description */
  description: string;
  /** Type */
  type: KnowledgeBaseType;
  /** Icon */
  icon: string;
  /** Enabled */
  enabled: boolean;
  /** Status */
  status: KnowledgeBaseStatus;
  /** Tags */
  tags: string[];
  /** Token estimate */
  tokenEstimate: number;
  /** Created at */
  createdAt: string;
  /** Updated at */
  updatedAt: string;
  /** Last sync time */
  lastSyncAt?: string;
  /** Error message */
  errorMessage?: string;
}

export interface SkillKnowledgeBase extends KnowledgeBase {
  type: 'skill';
  /** Skill config */
  config: SkillKnowledgeConfig;
}

export interface RAGKnowledgeBase extends KnowledgeBase {
  type: 'rag';
  /** RAG config */
  config: RAGKnowledgeConfig;
}

export type KnowledgeBaseItem = SkillKnowledgeBase | RAGKnowledgeBase;

export interface AddKnowledgeFormData {
  name: string;
  description: string;
  type: KnowledgeBaseType;
  icon: string;
  tags: string[];
  /** Skill config */
  skillConfig?: Partial<SkillKnowledgeConfig>;
  /** RAG config */
  ragConfig?: Partial<RAGKnowledgeConfig>;
}

/**
 * Preset knowledge base icons.
 */
export const KNOWLEDGE_ICONS = [
  'BookOpen',
  'Brain',
  'Database',
  'FileText',
  'Lightbulb',
  'Library',
  'Compass',
  'Code',
  'Terminal',
  'Cpu',
  'Globe',
  'Search',
  'Sparkles',
  'Zap'
] as const;
