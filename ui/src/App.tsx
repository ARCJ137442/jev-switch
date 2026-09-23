import { Shell, useHashRoute } from './app/Shell';
import { ProvidersPage } from './pages/ProvidersPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';

/**
 * 路由分发（design/01 §5 三页 IA，hash 路由手写）：
 * `/#/providers` · `/#/routing` · `/#/playground`（默认）。
 * 壳（导航 / daemon 灯 / 冲突横幅 / footer）统一在 Shell。
 */
export default function App() {
  const route = useHashRoute();
  return (
    <Shell route={route}>
      {route === 'providers' ? (
        <ProvidersPage />
      ) : route === 'routing' ? (
        <RoutingPage />
      ) : (
        <PlaygroundPage />
      )}
    </Shell>
  );
}
