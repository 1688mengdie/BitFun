 

import { api } from './ApiClient';
import { i18nService } from '@/infrastructure/i18n';



 
export enum RuleApplyType {
   
  AlwaysApply = 'always_apply',
   
  ApplyIntelligently = 'apply_intelligently',
   
  ApplyToSpecificFiles = 'apply_to_specific_files',
   
  ApplyManually = 'apply_manually',
}

 
export enum RuleLevel {
   
  User = 'user',
   
  Project = 'project',
   
  All = 'all',
}

 
export interface AIRule {
   
  name: string;
   
  level: RuleLevel;
   
  apply_type: RuleApplyType;
   
  description?: string;
   
  globs?: string;
   
  content: string;
   
  file_path: string;
   
  enabled: boolean;
}

 
export interface CreateRuleRequest {
   
  name: string;
   
  apply_type: RuleApplyType;
   
  description?: string;
   
  globs?: string;
   
  content: string;
   
  enabled?: boolean;
}

 
export interface UpdateRuleRequest {
   
  name?: string;
   
  apply_type?: RuleApplyType;
   
  description?: string;
   
  globs?: string;
   
  content?: string;
   
  enabled?: boolean;
}

 
export interface RuleStats {
   
  total_rules: number;
   
  enabled_rules: number;
   
  disabled_rules: number;
   
  by_apply_type: Record<string, number>;
}



export class AIRulesAPI {
   
  static async getRules(level: RuleLevel): Promise<AIRule[]> {
    return api.invoke<AIRule[]>('get_ai_rules', {
      request: {
        level,
      },
    });
  }

   
  static async getRule(level: RuleLevel, name: string): Promise<AIRule | null> {
    return api.invoke<AIRule | null>('get_ai_rule', {
      request: {
        level,
        name,
      },
    });
  }

   
  static async createRule(level: RuleLevel, rule: CreateRuleRequest): Promise<AIRule> {
    if (level === RuleLevel.All) {
      throw new Error('Cannot create rule with "all" level. Please specify "user" or "project".');
    }

    return api.invoke<AIRule>('create_ai_rule', {
      request: {
        level,
        rule,
      },
    });
  }

   
  static async updateRule(
    level: RuleLevel,
    name: string,
    rule: UpdateRuleRequest
  ): Promise<AIRule> {
    if (level === RuleLevel.All) {
      throw new Error('Cannot update rule with "all" level. Please specify "user" or "project".');
    }

    return api.invoke<AIRule>('update_ai_rule', {
      request: {
        level,
        name,
        rule,
      },
    });
  }

   
  static async deleteRule(level: RuleLevel, name: string): Promise<boolean> {
    if (level === RuleLevel.All) {
      throw new Error('Cannot delete rule with "all" level. Please specify "user" or "project".');
    }

    return api.invoke<boolean>('delete_ai_rule', {
      request: {
        level,
        name,
      },
    });
  }

   
  static async getRulesStats(level: RuleLevel): Promise<RuleStats> {
    return api.invoke<RuleStats>('get_ai_rules_stats', {
      request: {
        level,
      },
    });
  }

   
  static async buildSystemPrompt(): Promise<string> {
    return api.invoke<string>('build_ai_rules_system_prompt', {});
  }

   
  static async reloadRules(level: RuleLevel): Promise<void> {
    return api.invoke<void>('reload_ai_rules', {
      level,
    });
  }

   
  static async toggleRule(level: RuleLevel, name: string): Promise<AIRule> {
    if (level === RuleLevel.All) {
      throw new Error('Cannot toggle rule with "all" level. Please specify "user" or "project".');
    }

    return api.invoke<AIRule>('toggle_ai_rule', {
      request: {
        level,
        name,
      },
    });
  }

  

   
  static getApplyTypeLabel(type: RuleApplyType): string {
    const labels: Record<RuleApplyType, string> = {
      [RuleApplyType.AlwaysApply]: i18nService.t('settings/ai-rules:form.fields.applyTypes.alwaysApply'),
      [RuleApplyType.ApplyIntelligently]: i18nService.t('settings/ai-rules:form.fields.applyTypes.applyIntelligently'),
      [RuleApplyType.ApplyToSpecificFiles]: i18nService.t('settings/ai-rules:form.fields.applyTypes.applyToSpecificFiles'),
      [RuleApplyType.ApplyManually]: i18nService.t('settings/ai-rules:form.fields.applyTypes.applyManually'),
    };
    return labels[type] || type;
  }

   
  static getApplyTypeLabelEn(type: RuleApplyType): string {
    const labels: Record<RuleApplyType, string> = {
      [RuleApplyType.AlwaysApply]: 'Always Apply',
      [RuleApplyType.ApplyIntelligently]: 'Apply Intelligently',
      [RuleApplyType.ApplyToSpecificFiles]: 'Apply to Specific Files',
      [RuleApplyType.ApplyManually]: 'Apply Manually',
    };
    return labels[type] || type;
  }

   
  static getRuleLevelLabel(level: RuleLevel): string {
    const labels: Record<RuleLevel, string> = {
      [RuleLevel.User]: i18nService.t('settings/ai-rules:filters.user'),
      [RuleLevel.Project]: i18nService.t('settings/ai-rules:filters.project'),
      [RuleLevel.All]: i18nService.t('settings/ai-rules:filters.all'),
    };
    return labels[level] || level;
  }
}

export default AIRulesAPI;
