import { Shell, useHashRoute } from './app/Shell';
import { I18nProvider } from './i18n';
import { ProvidersPage } from './pages/ProvidersPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';

/**
 * 路由分发（design/01 §5 三页 IA，hash 路由手写）：
 * `/#/providers` · `/#/routing` · `/#/playground`（默认）。
 * 壳（导航 / daemon 灯 / 冲突横幅 / footer）统一在 Shell；i18n Provider 包最外。
 */
export default function App() {
  const route = useHashRoute();
  return (
    <I18nProvider>
      <Shell route={route}>
        {route === 'providers' ? (
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
