 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { PromptTemplateConfig } from '@/shared/types/prompt-template';

export class PromptTemplateAPI {
   
  async getConfig(): Promise<PromptTemplateConfig> {
    try {
      return await api.invoke('get_prompt_template_config');
    } catch (error) {
      throw createTauriCommandError('get_prompt_template_config', error);
    }
  }

   
  async saveConfig(config: PromptTemplateConfig): Promise<void> {
    try {
      await api.invoke('save_prompt_template_config', { config });
    } catch (error) {
      throw createTauriCommandError('save_prompt_template_config', error, { config });
    }
  }

   
  async exportTemplates(): Promise<string> {
    try {
      return await api.invoke('export_prompt_templates');
    } catch (error) {
      throw createTauriCommandError('export_prompt_templates', error);
    }
  }

   
  async importTemplates(json: string): Promise<void> {
    try {
      await api.invoke('import_prompt_templates', { json });
    } catch (error) {
      throw createTauriCommandError('import_prompt_templates', error, { json });
    }
  }

   
  async resetTemplates(): Promise<void> {
    try {
      await api.invoke('reset_prompt_templates');
    } catch (error) {
      throw createTauriCommandError('reset_prompt_templates', error);
    }
  }
}


export const promptTemplateAPI = new PromptTemplateAPI();

