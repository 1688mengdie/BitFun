/**
 * Async local image loader.
 * After Markdown render, resolves images marked with `data-local-image`.
 */

import { getMimeType } from './rehype-local-images';
import { createLogger } from '@/shared/utils/logger';
import { i18nService } from '@/infrastructure/i18n';

const log = createLogger('loadLocalImages');

/**
 * Load all images marked as local images inside the container.
 * @param container Container holding image elements.
 */
export async function loadLocalImages(container: HTMLElement): Promise<void> {
  const localImages = container.querySelectorAll<HTMLImageElement>('img[data-local-image="true"]');
  
  if (localImages.length === 0) {
    return;
  }
  
  const loadPromises = Array.from(localImages).map(async (img) => {
    const localPath = img.getAttribute('data-local-path');
    
    if (!localPath) {
      log.warn('Image missing data-local-path attribute');
      return;
    }
    
    try {
      const { workspaceAPI } = await import('@/infrastructure/api');
      
      const base64Content = await workspaceAPI.readFileContent(localPath);
      
      const mimeType = getMimeType(localPath);
      
      const dataUrl = `data:${mimeType};base64,${base64Content}`;
      
      img.src = dataUrl;
      
      img.classList.remove('local-image-loading');
      img.classList.add('local-image-loaded');
      
      img.removeAttribute('data-local-image');
      img.removeAttribute('data-local-path');
    } catch (error) {
      log.error('Failed to load local image', { path: localPath, error });
      
      img.classList.remove('local-image-loading');
      img.classList.add('local-image-error');
      
      const originalSrc = img.getAttribute('data-original-src') || localPath;
      img.alt = i18nService.t('tools:editor.meditor.localImageAltLoadFailedWithSrc', { src: originalSrc });
      img.title = i18nService.t('tools:editor.meditor.localImageTitleUnableToLoadWithPath', { path: localPath });
    }
  });
  
  await Promise.allSettled(loadPromises);
}

