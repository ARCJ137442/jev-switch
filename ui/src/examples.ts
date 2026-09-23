/**
 * 4 个示例场景 — 与 docs/06-MVP-实现计划.md D6-D8 对齐。
 * 每个示例含: model 选路 + state + questions。
 *
 * 命名约定：
 * - id: machine-readable slug
 * - label: 显示在 chip 上
 * - description: 鼠标 hover title
 * - payload: JevRequest (model + state + questions)
 * 题型仅契约入站三变体 choice/score/noul（contracts/01 §1 — 无 boolean）。
 */

import type { JevRequest } from './api';

export interface ExamplePayload {
  id: string;
  label: string;
  description: string;
  payload: JevRequest;
}

const DEFAULT_MODEL_LAYA = 'laya-english';

export const EXAMPLES: ExamplePayload[] = [
  {
    id: 'support-routing',
    label: 'Support Routing',
    description: 'Route a support ticket to the right team (choice question)',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        ticket: {
          subject: 'Cannot reset password',
          body: "I've tried resetting my password three times and the email never arrives. Need this resolved ASAP for a customer demo tomorrow.",
          priority: 'high',
        },
        customer: { tier: 'enterprise', region: 'us-west' },
      },
      questions: {
        routing: {
          type: 'choice',
          instructions: 'Which team should handle this support ticket?',
          criteria: {
            billing: 'billing & payments',
            auth: 'authentication & account access',
            bug: 'product bug or defect',
            sales: 'sales / pre-sales',
          },
        },
      },
    },
  },
  {
    id: 'refund-verification',
    label: 'Refund Verification',
    description: 'Is this refund request legitimate? (noul question)',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        order: { id: 'ord_8821', amount: 129.99, days_since_purchase: 14 },
        customer_history: { returns_last_year: 0, account_age_days: 420 },
        reason: 'Item arrived damaged',
      },
      questions: {
        should_refund: {
          type: 'noul',
          instructions: 'Should we approve this refund request?',
          criteria: { true: 'approve', false: 'deny' },
        },
      },
    },
  },
  {
    id: 'incident-urgency',
    label: 'Incident Urgency',
    description: 'Score incident urgency from 0 (cosmetic) to 1 (outage)',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        alert: 'p99 latency on /api/checkout spiked to 4.2s (normal: 180ms)',
        affected_regions: ['us-east', 'eu-west'],
        error_rate_delta: '+0.8%',
        started_at: '2026-09-23T08:42:00Z',
      },
      questions: {
        urgency: {
          type: 'score',
          instructions: 'How urgent is this incident?',
          criteria: [
            'cosmetic — no user impact',
            'minor — slow for some users',
            'degraded — partial functionality broken',
            'major — wide user impact',
            'outage — full service down',
          ],
        },
      },
    },
  },
  {
    id: 'agent-next-action',
    label: 'Agent Next Action',
    description: 'Pick the next best action for a support agent (choice)',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        agent: { id: 'agt_41', tier: 'L2', shift_remaining_min: 45 },
        ticket: { kind: 'refund', status: 'pending_evidence', customer_tier: 'pro' },
        signals: { customer_replies: 2, sla_remaining_min: 18 },
      },
      questions: {
        next_action: {
          type: 'choice',
          instructions: 'What should the agent do next?',
          criteria: {
            ask_evidence: 'ask customer for evidence',
            escalate: 'escalate to L3',
            process_refund: 'process refund immediately',
            send_template: 'send templated follow-up',
            close: 'close as resolved',
          },
        },
      },
    },
  },
];
