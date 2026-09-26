import type { JevRequest } from '../../api';

export interface ComparisonSnapshot {
  state: JevRequest['state'];
  questions: JevRequest['questions'];
  stateJson: string;
  questionsJson: string;
  fingerprint: string;
}

export function getSnapshot(stateJson: string, questionsJson: string): ComparisonSnapshot {
  let state: JevRequest['state'];
  let questions: JevRequest['questions'];
  try { state = JSON.parse(stateJson); } catch { throw new Error('state'); }
  try { questions = JSON.parse(questionsJson); } catch { throw new Error('questions'); }
  if (questions === null || typeof questions !== 'object' || Array.isArray(questions) || Object.keys(questions).length === 0) {
    throw new Error('questions');
  }
  return { state, questions, stateJson, questionsJson, fingerprint: JSON.stringify([state, questions]) };
}
