import test from 'node:test';
import assert from 'node:assert/strict';
import { loadTs } from './load-ts.mjs';

const { formKeyAction } = await loadTs('../src/app/formKeyboard.ts');

const key = (overrides = {}) => ({
  key: 'Enter', ctrlKey: false, metaKey: false, altKey: false, shiftKey: false,
  isComposing: false, targetTag: 'INPUT', targetType: 'text', ...overrides,
});

test('Ctrl or Command plus Enter submits from any editable control', () => {
  assert.equal(formKeyAction(key({ ctrlKey: true })), 'submit');
  assert.equal(formKeyAction(key({ metaKey: true, targetTag: 'TEXTAREA' })), 'submit');
});

test('plain Enter advances single-line inputs and selects', () => {
  assert.equal(formKeyAction(key()), 'next');
  assert.equal(formKeyAction(key({ targetTag: 'SELECT', targetType: undefined })), 'next');
});

test('textarea Enter remains a newline and Tab remains native focus traversal', () => {
  assert.equal(formKeyAction(key({ targetTag: 'TEXTAREA' })), 'native');
  assert.equal(formKeyAction(key({ key: 'Tab' })), 'native');
  assert.equal(formKeyAction(key({ isComposing: true })), 'native');
});

test('plain Enter on action controls keeps native activation behavior', () => {
  assert.equal(formKeyAction(key({ targetType: 'checkbox' })), 'native');
  assert.equal(formKeyAction(key({ targetType: 'submit' })), 'native');
});
