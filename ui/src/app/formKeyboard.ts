type FormKeyInput = Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'isComposing'> & {
  targetTag: string;
  targetType?: string;
};

export function formKeyAction(input: FormKeyInput): 'submit' | 'next' | 'native' {
  if (input.isComposing) return 'native';
  if ((input.ctrlKey || input.metaKey) && input.key === 'Enter') return 'submit';
  if (input.key !== 'Enter' || input.ctrlKey || input.metaKey || input.altKey || input.shiftKey) return 'native';
  if (input.targetTag === 'TEXTAREA') return 'native';
  if (input.targetTag === 'SELECT') return 'next';
  if (input.targetTag !== 'INPUT') return 'native';
  if (['button', 'checkbox', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit'].includes(input.targetType ?? 'text')) return 'native';
  return 'next';
}

/** Shared keyboard behavior for editable forms in the console shell. */
export function handleFormKeyDown(event: KeyboardEvent): void {
  if (event.isComposing || !(event.target instanceof HTMLElement)) return;

  const target = event.target;
  const form = target.closest('form');
  if (!(form instanceof HTMLFormElement)) return;

  const action = formKeyAction({
    key: event.key,
    ctrlKey: event.ctrlKey,
    metaKey: event.metaKey,
    altKey: event.altKey,
    shiftKey: event.shiftKey,
    isComposing: event.isComposing,
    targetTag: target.tagName,
    targetType: target instanceof HTMLInputElement ? target.type : undefined,
  });

  if (action === 'submit') {
    event.preventDefault();
    form.requestSubmit();
    return;
  }
  if (action !== 'next') return;

  // Tab remains the browser's normal focus traversal key for every control.
  const controls = Array.from(form.elements).filter((element): element is HTMLElement => {
    if (!(element instanceof HTMLElement) || element.hasAttribute('disabled') || element.hidden) return false;
    if (element instanceof HTMLInputElement && element.type === 'hidden') return false;
    return element instanceof HTMLInputElement || element instanceof HTMLSelectElement || element instanceof HTMLTextAreaElement || element instanceof HTMLButtonElement;
  });
  const index = controls.indexOf(target);
  if (index < 0) return;

  event.preventDefault();
  controls[index + 1]?.focus();
}
