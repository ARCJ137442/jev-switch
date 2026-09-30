import { useState } from 'react';
import { Shell, useHashRoute, type Route } from './app/Shell';
import { I18nProvider } from './i18n';
import { DashboardPage } from './pages/DashboardPage';
import { ProvidersPage } from './pages/ProvidersPage';
import { EndpointsPage } from './pages/EndpointsPage';
import { RoutingPage } from './pages/RoutingPage';
import { PlaygroundPage } from './pages/PlaygroundPage';
import { StatisticsPage } from './pages/StatisticsPage';
import { SettingsPage } from './pages/SettingsPage';
import { MobileConnectPage } from './pages/MobileConnectPage';
import { readApiBasePreference } from './api/base';
import { isAndroidTauri } from './api/androidGateway';
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
  const androidMobile = import.meta.env.VITE_JEV_MOBILE && isAndroidTauri();
  const [mobileConnected, setMobileConnected] = useState(() => !androidMobile || Boolean(readApiBasePreference()));
  if (androidMobile && !mobileConnected) {
    return <MobileConnectPage onConnected={() => setMobileConnected(true)} onUseLocalGateway={() => setMobileConnected(true)} />;
  }
  const visibleRoute = isReadOnly && route !== 'dashboard' && route !== 'home' && route !== 'playground' && route !== 'stats' && route !== 'settings'
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
        ) : visibleRoute === 'stats' ? (
          <StatisticsPage />
        ) : visibleRoute === 'settings' ? (
          <SettingsPage />
        ) : (
          <PlaygroundPage />
        )}
    </Shell>
  );
}
