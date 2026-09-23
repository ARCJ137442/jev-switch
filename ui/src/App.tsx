import { Shell, useHashRoute } from './app/Shell';
import { I18nProvider } from './i18n';
import { HomePage } from './pages/HomePage';
import { ProvidersPage } from './pages/ProvidersPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';

/**
 * 路由分发（块 5 起四页 IA）：
 * `/#/home`（默认）· `/#/providers` · `/#/routing` · `/#/playground`。
 * 壳（导航 / daemon 灯 / 冲突横幅 / footer）统一在 Shell；i18n Provider 包最外。
 */
export default function App() {
  const route = useHashRoute();
  return (
    <I18nProvider>
      <Shell route={route}>
        {route === 'home' ? (
          <HomePage />
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
