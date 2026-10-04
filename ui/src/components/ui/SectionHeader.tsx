import type { ReactNode } from 'react';

interface SectionHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  aside?: ReactNode;
  level?: 'h2' | 'h3';
  className?: string;
}

/** Small local pattern for dense page sections; it owns no data or behavior. */
export function SectionHeader({ title, description, aside, level = 'h2', className = '' }: SectionHeaderProps) {
  const Heading = level;
  return (
    <header className={`ui-section-header ${className}`.trim()}>
      <div className="min-w-0">
        <Heading className="ui-section-header__title">{title}</Heading>
        {description && <p className="ui-section-header__description">{description}</p>}
      </div>
      {aside && <div className="shrink-0">{aside}</div>}
    </header>
  );
}

interface MetricStripProps {
  children: ReactNode;
  className?: string;
}

export function MetricStrip({ children, className = '' }: MetricStripProps) {
  return <div className={`ui-metric-strip ${className}`.trim()}>{children}</div>;
}
