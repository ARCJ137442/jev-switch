import type { JevRequest, JevResponse } from '../api';
import { adminRequest } from './admin';

/** Direct-provider calls stay behind the daemon and use the admin session. */
export function invokeProviderModel(
  providerConfigId: string,
  request: JevRequest,
  signal?: AbortSignal,
): Promise<JevResponse> {
  return adminRequest<JevResponse>(
    `/v1/admin/providers/${encodeURIComponent(providerConfigId)}/invoke`,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(request),
      signal,
    },
  );
}
