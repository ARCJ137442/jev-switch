import { createContext, useCallback, useContext, useMemo, useSyncExternalStore, type ReactNode } from 'react';
import {
  clearCallerSession,
  getAuthIdentity,
  markAdminSession,
  setCallerSession,
  subscribeAuth,
  type AuthIdentity,
} from './callerSession';
import { getCallerIdentity } from '../api/access';
import { clearAdminSession } from '../api/admin';
import type { CallerRole } from './callerSession';

interface AuthContextValue {
  identity: AuthIdentity;
  role: 'admin' | 'readonly' | null;
  isReadOnly: boolean;
  canManage: boolean;
  loginCaller: (token: string) => Promise<CallerRole>;
  logoutCaller: () => void;
  markAdmin: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const identity = useSyncExternalStore(subscribeAuth, getAuthIdentity, getAuthIdentity);
  const role = identity?.role ?? null;
  const loginCaller = useCallback(async (token: string) => {
    const normalized = token.trim();
    if (!normalized) throw new Error('Caller token is required');
    const caller = await getCallerIdentity(normalized);
    // Never silently fall back to a previous, more privileged browser session.
    clearAdminSession();
    setCallerSession(normalized, caller);
    return caller.role;
  }, []);
  const logoutCaller = useCallback(() => clearCallerSession(), []);
  const markAdmin = useCallback(() => markAdminSession(), []);
  const value = useMemo<AuthContextValue>(() => ({
    identity,
    role,
    isReadOnly: role === 'readonly',
    canManage: role !== 'readonly',
    loginCaller,
    logoutCaller,
    markAdmin,
  }), [identity, role, loginCaller, logoutCaller, markAdmin]);
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const value = useContext(AuthContext);
  if (!value) throw new Error('useAuth must be used inside AuthProvider');
  return value;
}
