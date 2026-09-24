import { useState } from 'react';
import { Copy, Check } from 'lucide-react';

interface CopyButtonProps {
  text: string;
  className?: string;
  size?: number;
  onCopy?: () => void;
}

/**
 * 复制按钮组件（带成功动画）
 * 替代 Unicode ✓ 和按钮文字
 */
export function CopyButton({ text, className = '', size = 16, onCopy }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      onCopy?.();
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  return (
    <button
      onClick={handleCopy}
      className={className}
      title={copied ? 'Copied!' : 'Copy to clipboard'}
      aria-label={copied ? 'Copied!' : 'Copy to clipboard'}
      type="button"
    >
      {copied ? (
        <Check size={size} className="text-green-600" strokeWidth={2.5} />
      ) : (
        <Copy size={size} strokeWidth={2} />
      )}
    </button>
  );
}
