import { adminRequest } from './admin';
import type { CreateEndpointRequest } from '../generated/CreateEndpointRequest';
import type { UpdateEndpointRequest } from '../generated/UpdateEndpointRequest';
import type { EndpointsResponse } from '../generated/EndpointsResponse';
import type { ServiceEndpointView } from '../generated/ServiceEndpointView';
import type { StrategyConfig } from '../generated/StrategyConfig';
import type { DeleteEndpointResponse } from '../generated/DeleteEndpointResponse';
import type { DefaultStrategyBody } from '../generated/DefaultStrategyBody';

export type { CreateEndpointRequest, UpdateEndpointRequest, EndpointsResponse, ServiceEndpointView, StrategyConfig };

export function listEndpoints(signal?: AbortSignal): Promise<EndpointsResponse> {
  return adminRequest('/v1/admin/endpoints', { signal });
}

export function createEndpoint(body: CreateEndpointRequest): Promise<ServiceEndpointView> {
  return adminRequest('/v1/admin/endpoints', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  });
}

export async function updateEndpoint(id: string, body: Partial<UpdateEndpointRequest>): Promise<void> {
  await adminRequest(`/v1/admin/endpoints/${encodeURIComponent(id)}`, {
    method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
  });
}

export function deleteEndpoint(id: string): Promise<DeleteEndpointResponse> {
  return adminRequest(`/v1/admin/endpoints/${encodeURIComponent(id)}`, { method: 'DELETE' });
}

export function updateDefaultStrategy(default_strategy: string): Promise<DefaultStrategyBody> {
  return adminRequest('/v1/admin/config/default_strategy', {
    method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ default_strategy }),
  });
}
