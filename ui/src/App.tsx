import { Shell, useHashRoute } from './app/Shell';
import { I18nProvider } from './i18n';
import { DashboardPage } from './pages/DashboardPage';
import { ProvidersPage } from './pages/ProvidersPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';

/**
 * 路由分发（v2.0 重构：Dashboard 首页 + Providers/Routing/Playground）
 * `/#/` 或 `/#/dashboard`（默认）· `/#/providers` · `/#/routing` · `/#/playground`
 */
export default function App() {
  const route = useHashRoute();
  return (
    <I18nProvider>
      <Shell route={route}>
        {route === 'home' || route === 'dashboard' ? (
          <DashboardPage />
        ) : route === 'providers' ? (
          <ProvidersPage />
        ) : route === 'routing' ? (
          <RoutingPage />
        ) : (
          <PlaygroundPage />
        )}
      </Shell>
    </I18nProvider>
  );
}
