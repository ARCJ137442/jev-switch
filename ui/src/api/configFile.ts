import { adminRequest } from './admin';
import type { Route } from './admin';

export interface ConfigFileStatus {
  authority: 'sqlite';
  toml_path: string;
  toml_drifted: boolean;
  source_fingerprint: string | null;
  current_fingerprint: string | null;
}

export const getConfigFileStatus = () => adminRequest<ConfigFileStatus>('/v1/admin/config/storage');
export const importConfigFile = () => adminRequest<{ imported: boolean }>('/v1/admin/config/import-toml', {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ confirm: true }),
});
export const exportConfigFile = () => adminRequest<{ exported: boolean }>('/v1/admin/config/export-toml', { method: 'POST' });

export interface BackupProvider {
  kind: string;
  base: string;
  name?: string | null;
  account?: string | null;
  models: string[];
  api_key?: string | null;
  api_key_env?: string | null;
  enabled: boolean;
}

export interface RuntimeConfigBackup {
  schema_version: 1;
  providers: Record<string, BackupProvider>;
  routes: Route[];
}

export const exportRuntimeConfigJson = () => adminRequest<RuntimeConfigBackup>('/v1/admin/config/export-json', {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ confirm: true }),
});

export const importRuntimeConfigJson = (backup: RuntimeConfigBackup) => adminRequest<{ imported: boolean; schema_version: number }>('/v1/admin/config/import-json', {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ confirm: true, backup }),
});
