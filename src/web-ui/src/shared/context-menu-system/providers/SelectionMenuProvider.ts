 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext, ContextType, SelectionContext } from '../types/context.types';
import { commandExecutor } from '../commands/CommandExecutor';
import { systemAPI } from '../../../infrastructure/api';
import { i18nService } from '@/infrastructure/i18n';
import { createLogger } from '@/shared/utils/logger';

const log = createLogger('SelectionMenuProvider');

export class SelectionMenuProvider implements IMenuProvider {
  readonly id = 'selection';
  readonly name = i18nService.t('common:contextMenu.selectionMenu.name');
  readonly description = i18nService.t('common:contextMenu.selectionMenu.description');
  readonly priority = 100;

  matches(context: MenuContext): boolean {
    return context.type === ContextType.SELECTION;
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const selectionContext = context as SelectionContext;
    const items: MenuItem[] = [];

    
    items.push({
      id: 'selection-copy',
      label: i18nService.t('common:actions.copy'),
      icon: 'Copy',
      shortcut: 'Ctrl+C',
      command: 'copy',
      onClick: async (ctx) => {
        await commandExecutor.execute('copy', ctx);
      }
    });

    
    if (selectionContext.isEditable) {
      items.push({
        id: 'selection-cut',
        label: i18nService.t('common:actions.cut'),
        icon: 'Scissors',
        shortcut: 'Ctrl+X',
        command: 'cut',
        onClick: async (ctx) => {
          await commandExecutor.execute('cut', ctx);
        }
      });
    }

    
    if (selectionContext.selectedText && selectionContext.selectedText.length < 100) {
      items.push({
        id: 'selection-separator-1',
        label: '',
        separator: true
      });

      items.push({
        id: 'selection-search',
        label: `${i18nService.t('common:actions.search')} "${this.truncateText(selectionContext.selectedText, 20)}"`,
        icon: 'Search',
        onClick: async (ctx) => {
          const text = (ctx as SelectionContext).selectedText;
          const searchUrl = `https://www.google.com/search?q=${encodeURIComponent(text)}`;
          try {
            await systemAPI.openExternal(searchUrl);
          } catch (error) {
            log.error('Failed to open search URL', error as Error);
          }
        }
      });
    }

    return items;
  }

  isEnabled(): boolean {
    return true;
  }

  private truncateText(text: string, maxLength: number): string {
    if (text.length <= maxLength) {
      return text;
    }
    return text.substring(0, maxLength) + '...';
  }
}

