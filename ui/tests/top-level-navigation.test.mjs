import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const {
  TOP_LEVEL_ROUTES,
  cycleTopLevelRoute,
  isEditableShortcutTarget,
  isTopLevelShortcut,
} = await loadTs('../src/app/topLevelNavigation.ts');

const key = (overrides = {}) => ({
  key: 'Tab', ctrlKey: true, metaKey: false, altKey: false,
  isComposing: false, defaultPrevented: false, ...overrides,
});

test('cycles every top-level page forward and backward like browser tabs', () => {
  assert.equal(cycleTopLevelRoute('dashboard'), 'providers');
  assert.equal(cycleTopLevelRoute('settings'), 'dashboard');
  assert.equal(cycleTopLevelRoute('dashboard', true), 'settings');
  assert.equal(cycleTopLevelRoute('providers', true), 'dashboard');
  assert.equal(cycleTopLevelRoute('home'), 'providers');
  assert.equal(TOP_LEVEL_ROUTES.length, 7);
});

test('only Tauri Ctrl or Meta plus Tab claims the shortcut', () => {
  assert.equal(isTopLevelShortcut(key(), true), true);
  assert.equal(isTopLevelShortcut(key({ shiftKey: true }), true), true);
  assert.equal(isTopLevelShortcut(key(), false), false, 'browser tabs retain Ctrl+Tab');
  assert.equal(isTopLevelShortcut(key({ altKey: true }), true), false);
  assert.equal(isTopLevelShortcut(key({ defaultPrevented: true }), true), false);
  assert.equal(isTopLevelShortcut(key({ key: 'Enter' }), true), false);
});

test('editable controls keep native focus and text behavior', () => {
  assert.equal(isEditableShortcutTarget({ tagName: 'INPUT' }), true);
  assert.equal(isEditableShortcutTarget({ tagName: 'TEXTAREA' }), true);
  assert.equal(isEditableShortcutTarget({ tagName: 'SELECT' }), true);
  assert.equal(isEditableShortcutTarget({ isContentEditable: true }), true);
  assert.equal(isEditableShortcutTarget({ closest: () => ({}) }), true);
  assert.equal(isEditableShortcutTarget({ tagName: 'BUTTON', closest: () => null }), false);
});
