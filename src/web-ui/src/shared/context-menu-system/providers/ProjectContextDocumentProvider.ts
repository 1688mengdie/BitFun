 

import { IMenuProvider } from '../types/provider.types';
import { MenuItem } from '../types/menu.types';
import { MenuContext, ContextType, ProjectContextDocumentContext } from '../types/context.types';
import { globalEventBus } from '../../../infrastructure/event-bus';
import { i18nService } from '../../../infrastructure/i18n';

export class ProjectContextDocumentProvider implements IMenuProvider {
  readonly id = 'project-context-document';
  readonly name = i18nService.t('common:contextMenu.projectContextDocumentMenu.name');
  readonly description = i18nService.t('common:contextMenu.projectContextDocumentMenu.description');
  readonly priority = 85;

  matches(context: MenuContext): boolean {
    return context.type === ContextType.PROJECT_CONTEXT_DOCUMENT;
  }

  async getMenuItems(context: MenuContext): Promise<MenuItem[]> {
    const docContext = context as ProjectContextDocumentContext;
    const items: MenuItem[] = [];

    
    if (docContext.exists) {
      items.push({
        id: 'doc-delete',
        label: i18nService.t('common:document.delete'),
        icon: 'Trash2',
        
        disabled: !docContext.exists,
        onClick: () => {
          
          globalEventBus.emit('project-context:delete-document', {
            docId: docContext.docId,
            docName: docContext.docName,
            filePath: docContext.filePath,
            categoryId: docContext.categoryId
          });
        }
      });
    }

    return items;
  }

  isEnabled(): boolean {
    return true;
  }
}
