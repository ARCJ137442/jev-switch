import { Circle } from 'lucide-react';

type Status = 'healthy' | 'degraded' | 'failed' | 'idle';

const statusColors: Record<Status, string> = {
  healthy: 'var(--success)',
  degraded: 'var(--warning)',
  failed: 'var(--danger)',
  idle: 'var(--text-subtle)',
};

interface StatusDotProps {
  status: Status;
  size?: number;
  fill?: boolean;
  className?: string;
}

/**
 * 状态圆点组件（替代 Unicode ●/◉/○）
 * 用于健康状态指示、provider 状态等
 */
export function StatusDot({ status, size = 8, fill = true, className = '' }: StatusDotProps) {
  return (
    <Circle
      size={size}
      className={className}
      style={{ color: statusColors[status] }}
      fill={fill ? 'currentColor' : 'none'}
      strokeWidth={fill ? 0 : 2}
    />
  );
}
