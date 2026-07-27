import React, { useCallback } from 'react';
import type { SelectOption } from '@/component-library';
import type { AIModelConfig } from '../types';

export interface ModelSelectOption extends SelectOption {
  model?: AIModelConfig;
}

/**
 * Hook 返回模型选择所需的选项构建和渲染函数
 * 用于 Select 组件的 options/renderOption/renderValue
 */
export function useModelSelectPresentation() {
  const buildModelOption = useCallback((model: AIModelConfig): ModelSelectOption => {
    return {
      label: model.name || model.model_name || model.id || '',
      value: model.id || model.model_name,
      model,
    };
  }, []);

  /** renderOption: Select 组件接受 (option: SelectOption) => ReactNode */
  const renderModelOption = useCallback((option: SelectOption): React.ReactNode => {
    return <span className="model-select-presentation__option">{option.label}</span>;
  }, []);

  /** renderValue: Select 组件接受 (option?: SelectOption | SelectOption[]) => ReactNode */
  const renderModelValue = useCallback((option?: SelectOption | SelectOption[]): React.ReactNode => {
    if (Array.isArray(option)) {
      return option.length > 0
        ? <span className="model-select-presentation__value">{option[0]?.label}</span>
        : null;
    }
    if (!option) return null;
    return <span className="model-select-presentation__value">{option.label}</span>;
  }, []);

  return { buildModelOption, renderModelOption, renderModelValue };
}
