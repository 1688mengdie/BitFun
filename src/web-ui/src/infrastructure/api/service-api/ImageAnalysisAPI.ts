 

import { api } from './ApiClient';
import { createTauriCommandError } from '../errors/TauriCommandError';
import type { ImageContext } from '@/shared/types/context';



export interface ImageContextData {
  id: string;
  image_path?: string;
  data_url?: string;
  mime_type: string;
  metadata?: Record<string, any>;
}

export interface ImageAnalysisResult {
  image_id: string;
  summary: string;
  detailed_description: string;
  detected_elements: string[];
  confidence: number;
  analysis_time_ms: number;
}

export interface AnalyzeImagesRequest {
  images: ImageContextData[];
  user_message?: string;
  session_id: string;
}

export interface SendEnhancedMessageRequest {
  original_message: string;
  image_analyses: ImageAnalysisResult[];
  other_contexts: any[];
  session_id: string;
  dialog_turn_id: string;
}



export class ImageAnalysisAPI {
   
  async analyzeImages(request: AnalyzeImagesRequest): Promise<ImageAnalysisResult[]> {
    try {
      return await api.invoke<ImageAnalysisResult[]>('analyze_images', { 
        request 
      });
    } catch (error) {
      throw createTauriCommandError('analyze_images', error, request);
    }
  }
  
   
  async sendEnhancedMessage(request: SendEnhancedMessageRequest): Promise<void> {
    try {
      await api.invoke<void>('send_enhanced_message', { 
        request 
      });
    } catch (error) {
      throw createTauriCommandError('send_enhanced_message', error, request);
    }
  }
  
   
  convertContextToData(context: ImageContext): ImageContextData {
    return {
      id: context.id,
      image_path: context.isLocal ? context.imagePath : undefined,
      data_url: !context.isLocal ? context.dataUrl : undefined,
      mime_type: context.mimeType,
      metadata: {
        name: context.imageName,
        width: context.width,
        height: context.height,
        file_size: context.fileSize,
        source: context.source,
      }
    };
  }
  
   
  convertContextsToData(contexts: ImageContext[]): ImageContextData[] {
    return contexts.map(ctx => this.convertContextToData(ctx));
  }
}


export const imageAnalysisAPI = new ImageAnalysisAPI();

