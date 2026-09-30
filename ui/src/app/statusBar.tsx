import { createContext, useContext, useEffect, type ReactNode } from 'react';

export interface StatusBarItem {
  id: string;
  label: string;
  title?: string;
  href?: string;
  tone?: 'default' | 'good' | 'warning' | 'danger';
}

type RegisterStatusItems = (items: readonly StatusBarItem[]) => void;
const StatusBarContext = createContext<RegisterStatusItems>(() => undefined);

export function StatusBarContributionProvider({ children, register }: { children: ReactNode; register: RegisterStatusItems }) {
  return <StatusBarContext.Provider value={register}>{children}</StatusBarContext.Provider>;
}

/** Page-owned status items are cleared when the page unmounts. */
export function useStatusBarItems(items: readonly StatusBarItem[]): void {
  const register = useContext(StatusBarContext);
  useEffect(() => {
    register(items);
    return () => register([]);
  }, [items, register]);
}
