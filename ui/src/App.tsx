import { Shell, useHashRoute, type Route } from './app/Shell';
import { I18nProvider } from './i18n';
import { DashboardPage } from './pages/DashboardPage';
import { ProvidersPage } from './pages/ProvidersPage';
import { EndpointsPage } from './pages/EndpointsPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';
import { AuthProvider, useAuth } from './auth/AuthContext';

/**
 * 路由分发：Dashboard / Providers / Entries / Routing DAG / Playground。
 * `/#/` 或 `/#/dashboard`（默认）· `/#/providers` · `/#/endpoints` · `/#/routing` · `/#/playground`
 */
export default function App() {
  const route = useHashRoute();
  return (
    <I18nProvider>
      <AuthProvider>
        <AppContent route={route} />
      </AuthProvider>
    </I18nProvider>
  );
}

function AppContent({ route }: { route: Route }) {
  const { isReadOnly } = useAuth();
  const visibleRoute = isReadOnly && route !== 'dashboard' && route !== 'home' && route !== 'playground'
    ? 'dashboard'
    : route;
  return (
    <Shell route={visibleRoute}>
        {visibleRoute === 'home' || visibleRoute === 'dashboard' ? (
          <DashboardPage />
        ) : visibleRoute === 'providers' ? (
          <ProvidersPage />
        ) : visibleRoute === 'endpoints' ? (
          <EndpointsPage />
        ) : visibleRoute === 'routing' ? (
          <RoutingPage />
        ) : (
          <PlaygroundPage />
        )}
    </Shell>
  );
}
