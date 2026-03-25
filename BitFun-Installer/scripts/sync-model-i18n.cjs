const fs = require('fs');
const path = require('path');

const INSTALLER_ROOT = path.resolve(__dirname, '..');
const PROJECT_ROOT = path.resolve(INSTALLER_ROOT, '..');

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function writeJson(filePath, data) {
  fs.writeFileSync(filePath, `${JSON.stringify(data, null, 2)}\n`, 'utf8');
}

function get(obj, keyPath, fallback) {
  const segments = keyPath.split('.');
  let current = obj;
  for (const seg of segments) {
    if (!current || typeof current !== 'object' || !(seg in current)) {
      return fallback;
    }
    current = current[seg];
  }
  return current ?? fallback;
}

function mergeDeep(target, source) {
  const result = { ...(target || {}) };
  for (const [key, value] of Object.entries(source || {})) {
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      result[key] = mergeDeep(result[key], value);
    } else {
      result[key] = value;
    }
  }
  return result;
}

function buildProviderPatch(settingsAiModel) {
  const providers = get(settingsAiModel, 'providers', {});
  const providerPatch = {};

  for (const [providerId, provider] of Object.entries(providers)) {
    providerPatch[providerId] = {
      name: get(provider, 'name', providerId),
      description: get(provider, 'description', ''),
    };

    if (provider && provider.urlOptions && typeof provider.urlOptions === 'object') {
      providerPatch[providerId].urlOptions = { ...provider.urlOptions };
    }
  }

  return providerPatch;
}

function buildModelPatch(settingsAiModel, languageTag) {
  const isZh = languageTag === 'zh';
  return {
    description: get(
      settingsAiModel,
      'subtitle',
      'Configure AI model provider, API key, and advanced parameters.'
    ),
    providerLabel: get(settingsAiModel, 'providerSelection.title', 'Model Provider'),
    selectProvider: get(settingsAiModel, 'providerSelection.orSelectProvider', 'Select a provider...'),
    customProvider: get(settingsAiModel, 'providerSelection.customTitle', 'Custom'),
    getApiKey: get(settingsAiModel, 'providerSelection.getApiKey', 'How to get an API Key?'),
    modelNamePlaceholder: get(
      settingsAiModel,
      'providerSelection.inputModelName',
      get(settingsAiModel, 'form.modelName', 'Enter model name...')
    ),
    modelNameSelectPlaceholder: get(settingsAiModel, 'providerSelection.selectModel', 'Select a model...'),
    modelSearchPlaceholder: get(
      settingsAiModel,
      'providerSelection.searchOrInputModel',
      'Search or enter a custom model name...'
    ),
    modelNoResults: isZh ? '没有匹配的模型' : 'No matching models',
    customModel: get(settingsAiModel, 'providerSelection.useCustomModel', 'Use custom model name'),
    baseUrlPlaceholder: isZh
      ? '示例：https://open.bigmodel.cn/api/paas/v4/chat/completions'
      : 'e.g., https://open.bigmodel.cn/api/paas/v4/chat/completions',
    customRequestBodyPlaceholder: get(
      settingsAiModel,
      'advancedSettings.customRequestBody.placeholder',
      '{\n  "temperature": 0.8,\n  "top_p": 0.9\n}'
    ),
    jsonValid: get(settingsAiModel, 'advancedSettings.customRequestBody.validJson', 'Valid JSON format'),
    jsonInvalid: get(
      settingsAiModel,
      'advancedSettings.customRequestBody.invalidJson',
      'Invalid JSON format'
    ),
    skipSslVerify: get(
      settingsAiModel,
      'advancedSettings.skipSslVerify.label',
      'Skip SSL Certificate Verification'
    ),
    customHeadersModeMerge: get(
      settingsAiModel,
      'advancedSettings.customHeaders.modeMerge',
      'Merge Override'
    ),
    customHeadersModeReplace: get(
      settingsAiModel,
      'advancedSettings.customHeaders.modeReplace',
      'Replace All'
    ),
    addHeader: get(settingsAiModel, 'advancedSettings.customHeaders.addHeader', 'Add Field'),
    headerKey: get(settingsAiModel, 'advancedSettings.customHeaders.keyPlaceholder', 'key'),
    headerValue: get(settingsAiModel, 'advancedSettings.customHeaders.valuePlaceholder', 'value'),
    testConnection: get(settingsAiModel, 'actions.test', 'Test Connection'),
    testing: isZh ? '测试中...' : 'Testing...',
    testSuccess: get(settingsAiModel, 'messages.testSuccess', 'Connection successful'),
    testFailed: get(settingsAiModel, 'messages.testFailed', 'Connection failed'),
    advancedShow: 'Show advanced settings',
    advancedHide: 'Hide advanced settings',
    providers: buildProviderPatch(settingsAiModel),
  };
}

function syncOne(languageTag) {
  const localeDir = languageTag === 'zh' ? 'zh-CN' : 'en-US';
  const installerLocale = languageTag === 'zh' ? 'zh.json' : 'en.json';

  const sourceAiModelPath = path.join(
    PROJECT_ROOT,
    'src',
    'web-ui',
    'src',
    'locales',
    localeDir,
    'settings',
    'ai-model.json'
  );

  const targetPath = path.join(INSTALLER_ROOT, 'src', 'i18n', 'locales', installerLocale);

  const settingsAiModel = readJson(sourceAiModelPath);
  const target = readJson(targetPath);

  const patch = buildModelPatch(settingsAiModel, languageTag);
  target.model = mergeDeep(target.model || {}, patch);

  writeJson(targetPath, target);
}

function main() {
  syncOne('en');
  syncOne('zh');
  console.log('[sync-model-i18n] Synced installer model i18n from web-ui locales.');
}

main();
