import { invoke } from '@tauri-apps/api/core';
import { isAndroidTauriRuntime } from './androidDebug';

/** Android WebView does not promise that Blob anchors reach Downloads. */
export async function saveJsonFile(name: string, content: string): Promise<'downloads' | 'shared' | 'browser'> {
  if (isAndroidTauriRuntime()) {
    return invoke<'downloads' | 'shared'>('android_export_json_file', { name, content });
  }
  const url = URL.createObjectURL(new Blob([content], { type: 'application/json' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  return 'browser';
}

export async function copyText(text: string): Promise<void> {
  if (isAndroidTauriRuntime()) {
    await invoke('android_write_clipboard', { text });
  } else {
    await navigator.clipboard.writeText(text);
  }
}
