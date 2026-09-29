export const PROVIDER_KINDS = [
  {
    value: 'vercel',
    labelKey: 'providers.kindVercel',
    descriptionKey: 'providers.kindVercelHint',
    defaultBase: 'https://ai-gateway.vercel.sh/typesafe/v1/systemone',
    defaultModels: [],
  },
  {
    value: 'laya',
    labelKey: 'providers.kindLaya',
    descriptionKey: 'providers.kindLayaHint',
    defaultBase: 'http://127.0.0.1:18765/v1/systemone',
    defaultModels: [],
  },
  {
    value: 'typesafe',
    labelKey: 'providers.kindTypeSafe',
    descriptionKey: 'providers.kindTypeSafeHint',
    defaultBase: 'https://api.typesafe.ai/v1/systemone',
    defaultModels: ['jev-latest'],
  },
  {
    value: 'openrouter',
    labelKey: 'providers.kindOpenRouter',
    descriptionKey: 'providers.kindOpenRouterHint',
    defaultBase: 'https://openrouter.ai/api/v1/chat/completions',
    defaultModels: ['openrouter/auto'],
  },
] as const;

export type ProviderKind = (typeof PROVIDER_KINDS)[number]['value'];

export const PROVIDER_KIND_VALUES = PROVIDER_KINDS.map(({ value }) => value) as ProviderKind[];

export function isProviderKind(value: string): value is ProviderKind {
  return PROVIDER_KIND_VALUES.includes(value as ProviderKind);
}

export function getProviderKindPreset(value: string) {
  return PROVIDER_KINDS.find((kind) => kind.value === value);
}

export function applyProviderKindPreset(
  previousKind: string,
  nextKind: string,
  currentBase: string,
  currentModels: readonly string[],
): { base: string; models: string[] } {
  const previous = getProviderKindPreset(previousKind);
  const next = getProviderKindPreset(nextKind);
  if (!next) return { base: currentBase, models: [...currentModels] };

  const baseIsPreset = !currentBase.trim() || currentBase.trim() === previous?.defaultBase;
  const modelsArePreset =
    currentModels.length === 0 ||
    (previous !== undefined &&
      currentModels.length === previous.defaultModels.length &&
      currentModels.every((model, index) => model === previous.defaultModels[index]));

  return {
    base: baseIsPreset ? next.defaultBase : currentBase,
    models: modelsArePreset ? [...next.defaultModels] : [...currentModels],
  };
}
