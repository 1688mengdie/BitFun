 

import { PlaceholderInfo } from '../types/prompt-template';

 
const PLACEHOLDER_REGEX = /\{\{([^}:|\n]+)(?::([^}|\n]+))?(?:\|([^}\n]+))?\}\}/g;

 
export function parseTemplate(content: string): PlaceholderInfo[] {
  const placeholders: PlaceholderInfo[] = [];
  let match: RegExpExecArray | null;

  
  PLACEHOLDER_REGEX.lastIndex = 0;

  while ((match = PLACEHOLDER_REGEX.exec(content)) !== null) {
    const [fullMatch, name, defaultValue, description] = match;
    
    placeholders.push({
      name: name.trim(),
      defaultValue: defaultValue?.trim(),
      description: description?.trim(),
      startIndex: match.index,
      endIndex: match.index + fullMatch.length,
      displayText: fullMatch
    });
  }

  return placeholders;
}

 
export function fillTemplate(
  content: string, 
  values: Record<string, string>
): string {
  let result = content;
  const placeholders = parseTemplate(content);

  
  for (let i = placeholders.length - 1; i >= 0; i--) {
    const placeholder = placeholders[i];
    const value = values[placeholder.name] ?? placeholder.defaultValue ?? '';
    
    result = 
      result.slice(0, placeholder.startIndex) + 
      value + 
      result.slice(placeholder.endIndex);
  }

  return result;
}

 
export function validatePlaceholder(placeholder: string): boolean {
  const regex = /^\{\{[^}:|\n]+(?::[^}|\n]+)?(?:\|[^}\n]+)?\}\}$/;
  return regex.test(placeholder);
}

 
export function createPlaceholder(
  name: string,
  defaultValue?: string,
  description?: string
): string {
  let result = `{{${name}`;
  
  if (defaultValue) {
    result += `:${defaultValue}`;
  }
  
  if (description) {
    result += `|${description}`;
  }
  
  result += '}}';
  return result;
}

 
export function getUniquePlaceholderNames(content: string): string[] {
  const placeholders = parseTemplate(content);
  const names = placeholders.map(p => p.name);
  return [...new Set(names)];
}

 
export function hasPlaceholders(content: string): boolean {
  return PLACEHOLDER_REGEX.test(content);
}

 
export function replacePlaceholder(
  content: string,
  placeholderName: string,
  newValue: string
): string {
  const placeholders = parseTemplate(content);
  let result = content;
  let offset = 0;

  for (const placeholder of placeholders) {
    if (placeholder.name === placeholderName) {
      const start = placeholder.startIndex + offset;
      const end = placeholder.endIndex + offset;
      
      result = result.slice(0, start) + newValue + result.slice(end);
      offset += newValue.length - (placeholder.endIndex - placeholder.startIndex);
    }
  }

  return result;
}

 
export function escapePlaceholder(text: string): string {
  return text.replace(/\{\{/g, '\\{\\{').replace(/\}\}/g, '\\}\\}');
}

 
export function unescapePlaceholder(text: string): string {
  return text.replace(/\\{\\{/g, '{{').replace(/\\}\\}/g, '}}');
}

