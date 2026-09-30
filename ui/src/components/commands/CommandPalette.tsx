import { useEffect, useMemo, useRef, useState } from 'react';
import { Command, Search } from 'lucide-react';
import { isTauri } from '@tauri-apps/api/core';
import { useAuth } from '../../auth/AuthContext';
import { useI18n, type MessageKey } from '../../i18n';
import { COMMANDS, filterCommands, type CommandDefinition } from '../../commands/registry';
import { readRecentCommands, updateRecentCommands, writeRecentCommands } from '../../commands/recent';
import { getGatewayServiceStatus, setGatewayServiceRunning } from '../../api/gatewayControl';
import { useToast } from '../../app/feedback';
import './command-palette.css';

export function CommandPalette() {
  const { t } = useI18n();
  const auth = useAuth();
  const { toast } = useToast();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [recentIds, setRecentIds] = useState(readRecentCommands);
  const input = useRef<HTMLInputElement>(null);
  const available = useMemo(() => COMMANDS.filter((command) => (!command.adminOnly || auth.canManage) && (!command.action || isTauri())), [auth.canManage]);
  const recent = useMemo(() => recentIds.map((id) => available.find((command) => command.id === id)).filter((command): command is CommandDefinition => command !== undefined), [available, recentIds]);
  const matched = useMemo(() => filterCommands(available, query, t), [available, query, t]);
  const commands = useMemo(() => {
    if (query.trim()) return matched;
    const recentSet = new Set(recent.map((command) => command.id));
    return [...recent, ...available.filter((command) => !recentSet.has(command.id))];
  }, [available, matched, query, recent]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('input,textarea,select,[contenteditable="true"]')) return;
      if ((event.ctrlKey || event.metaKey) && ['k', 'p'].includes(event.key.toLocaleLowerCase())) {
        event.preventDefault();
        setOpen(true);
        setQuery('');
        setSelected(0);
      } else if (event.key === 'Escape' && open) {
        event.preventDefault();
        setOpen(false);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open]);

  useEffect(() => { if (open) window.setTimeout(() => input.current?.focus(), 0); }, [open]);
  useEffect(() => { setSelected((value) => Math.min(value, Math.max(0, commands.length - 1))); }, [commands.length]);

  if (!open) return null;
  const run = async (command: CommandDefinition) => {
    setOpen(false);
    try {
      if (command.action === 'toggle_gateway') {
        const current = await getGatewayServiceStatus();
        if (!current) throw new Error(t('gateway.unavailable' as MessageKey));
        const next = await setGatewayServiceRunning(!current.running);
        toast('ok', t(next.running ? 'gateway.running' as MessageKey : 'gateway.stopped' as MessageKey));
      } else {
        window.location.hash = command.href.slice(1);
      }
      const nextRecent = updateRecentCommands(recentIds, command.id);
      setRecentIds(nextRecent);
      writeRecentCommands(nextRecent);
    } catch (cause) {
      toast('danger', cause instanceof Error ? cause.message : String(cause));
    }
  };
  return (
    <div className="command-palette-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setOpen(false); }}>
      <section className="command-palette" role="dialog" aria-modal="true" aria-labelledby="command-palette-title">
        <div className="command-palette__search">
          <Search size={17} aria-hidden />
          <input ref={input} value={query} onChange={(event) => { setQuery(event.target.value); setSelected(0); }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') { event.preventDefault(); setSelected((value) => Math.min(value + 1, commands.length - 1)); }
              if (event.key === 'ArrowUp') { event.preventDefault(); setSelected((value) => Math.max(value - 1, 0)); }
              if (event.key === 'Enter' && commands[selected]) { event.preventDefault(); void run(commands[selected]); }
            }}
            placeholder={t('command.placeholder')} aria-label={t('command.placeholder')} />
          <kbd>Esc</kbd>
        </div>
        <h2 id="command-palette-title" className="sr-only">{t('command.title')}</h2>
        <div className="command-palette__list" role="listbox" aria-label={t('command.title')}>
          {!query.trim() && recent.length > 0 && <div className="command-palette__section" role="presentation">{t('command.recent' as MessageKey)}</div>}
          {commands.map((command, index) => (
            <button type="button" role="option" aria-selected={index === selected} key={command.id}
              className="command-palette__item" onMouseEnter={() => setSelected(index)} onClick={() => void run(command)}>
              <Command size={15} aria-hidden />
              <span>{t(command.labelKey)}</span>
              <span className="command-palette__hint">{command.keywords[0]}</span>
            </button>
          ))}
          {commands.length === 0 && <div className="command-palette__empty">{t('command.empty')}</div>}
        </div>
        <footer className="command-palette__footer">{t('command.footer')}</footer>
      </section>
    </div>
  );
}
