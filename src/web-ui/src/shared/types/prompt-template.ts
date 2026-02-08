/**
 * Prompt template types.
 */
export interface PromptTemplate {
  id: string;                    
  name: string;                  
  description?: string;          
  content: string;               
  category?: string;             
  shortcut?: string;             
  isFavorite: boolean;           
  order: number;                 
  createdAt: number;             
  updatedAt: number;             
  usageCount: number;            
}

 
export interface PlaceholderInfo {
  name: string;                  
  defaultValue?: string;         
  description?: string;          
  startIndex: number;            
  endIndex: number;              
  displayText: string;           
}

 
export interface PromptTemplateConfig {
  templates: PromptTemplate[];
  globalShortcut: string;        
  enableAutoComplete: boolean;   
  recentTemplates: string[];     
  lastSyncTime?: number;         
}

 
export interface TemplateCategory {
  id: string;
  name: string;
  icon?: string;
  order: number;
}

 
export interface TemplateSearchResult {
  template: PromptTemplate;
  matchScore: number;            
  matchedFields: string[];       
}

 
export interface TemplateInsertEvent {
  template: PromptTemplate;
  placeholders: PlaceholderInfo[];
}

 
export interface PlaceholderFillState {
  currentIndex: number;          
  placeholders: PlaceholderInfo[];
  filledValues: Record<string, string>;  
  isActive: boolean;             
}

 
export interface ShortcutConfig {
  key: string;                   
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  meta?: boolean;
}

 
export type PresetTemplateType = 
  | 'code-refactor'
  | 'bug-fix'
  | 'doc-generation'
  | 'test-case'
  | 'code-review';

 
export interface PresetTemplate extends Omit<PromptTemplate, 'id' | 'createdAt' | 'updatedAt' | 'usageCount'> {
  type: PresetTemplateType;
}
