import { Circle } from 'lucide-react';

type Status = 'healthy' | 'degraded' | 'failed' | 'idle';

const statusColors: Record<Status, string> = {
  healthy: 'text-green-600',
  degraded: 'text-yellow-600',
  failed: 'text-red-600',
  idle: 'text-gray-400',
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
      className={`${statusColors[status]} ${className}`}
      fill={fill ? 'currentColor' : 'none'}
      strokeWidth={fill ? 0 : 2}
    />
  );
}
