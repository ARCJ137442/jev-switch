import * as LucideIcons from 'lucide-react';

interface IconProps {
  name: keyof typeof LucideIcons;
  size?: number;
  className?: string;
  color?: string;
  strokeWidth?: number;
}

/**
 * 统一图标封装组件（lucide-react）
 * 替代项目中的 Unicode 字符与 emoji
 */
export function Icon({ name, size = 16, className, color, strokeWidth = 2 }: IconProps) {
  const LucideIcon = LucideIcons[name];
  if (!LucideIcon) {
    console.warn(`Icon "${name}" not found in lucide-react`);
    return null;
  }
  return <LucideIcon size={size} className={className} color={color} strokeWidth={strokeWidth} />;
}

/** 预设尺寸 */
export const IconSize = {
  xs: 12,
  sm: 14,
  md: 16,
  lg: 20,
  xl: 24,
} as const;
