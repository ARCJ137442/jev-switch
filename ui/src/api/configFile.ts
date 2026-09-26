import { adminRequest } from './admin';

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
