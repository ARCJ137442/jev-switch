import { useEffect, useMemo, useRef, useState } from 'react';
import { Command, Search } from 'lucide-react';
import { useAuth } from '../../auth/AuthContext';
import { useI18n } from '../../i18n';
import { COMMANDS, filterCommands } from '../../commands/registry';
import './command-palette.css';

export function CommandPalette() {
  const { t } = useI18n();
  const auth = useAuth();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const commands = useMemo(() => filterCommands(COMMANDS.filter((command) => !command.adminOnly || auth.canManage), query, t), [auth.canManage, query, t]);

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
  const run = (href: string) => {
    setOpen(false);
    window.location.hash = href.slice(1);
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
              if (event.key === 'Enter' && commands[selected]) { event.preventDefault(); run(commands[selected].href); }
            }}
            placeholder={t('command.placeholder')} aria-label={t('command.placeholder')} />
          <kbd>Esc</kbd>
        </div>
        <h2 id="command-palette-title" className="sr-only">{t('command.title')}</h2>
        <div className="command-palette__list" role="listbox" aria-label={t('command.title')}>
          {commands.map((command, index) => (
            <button type="button" role="option" aria-selected={index === selected} key={command.id}
              className="command-palette__item" onMouseEnter={() => setSelected(index)} onClick={() => run(command.href)}>
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
