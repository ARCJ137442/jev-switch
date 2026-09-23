import type { ProvidersResponse } from '../api/admin';

/**
 * Providers mock fixture — 形状严格按 contracts/05 §2 GET /v1/admin/providers 字面。
 * 仅密文（api_key_masked），不存在任何明文 key 字段。
 */
export const PROVIDERS_FIXTURE: ProvidersResponse = {
  providers: [
    {
      id: 'vercel',
      kind: 'vercel-gateway',
      base: 'https://ai-gateway.vercel.sh/v4/ai/evaluation-model',
      enabled: true,
      api_key_masked: 'sk-****a1b2',
      api_key_set: true,
    },
    {
      id: 'laya',
      kind: 'laya',
      base: 'http://127.0.0.1:18765/v1/systemone',
      enabled: true,
      api_key_masked: 'sk-****a1b2',
      api_key_set: true,
    },
  ],
};
