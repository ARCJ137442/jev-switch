import type { MessageKey } from '../i18n';

export interface CommandDefinition {
  id: string;
  labelKey: MessageKey;
  keywords: string[];
  href: string;
  adminOnly?: boolean;
}

export const COMMANDS: readonly CommandDefinition[] = [
  { id: 'dashboard', labelKey: 'command.dashboard', keywords: ['home', 'overview', '仪表盘', '首页'], href: '#/dashboard' },
  { id: 'providers', labelKey: 'command.providers', keywords: ['upstream', 'provider', '提供商', '上游'], href: '#/providers' },
  { id: 'endpoints', labelKey: 'command.endpoints', keywords: ['entry', 'model entry', '入口', '模型入口'], href: '#/endpoints' },
  { id: 'routing', labelKey: 'command.routing', keywords: ['dag', 'route', '路由', '路由图'], href: '#/routing' },
  { id: 'playground', labelKey: 'command.playground', keywords: ['test', 'compare', '演练场', '测试', '对比'], href: '#/playground' },
];

export function filterCommands(commands: readonly CommandDefinition[], query: string, translate: (key: MessageKey) => string) {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return [...commands];
  return commands.filter((command) => `${translate(command.labelKey)} ${command.keywords.join(' ')}`.toLocaleLowerCase().includes(normalized));
}
