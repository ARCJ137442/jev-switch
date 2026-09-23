/**
 * 示例场景 — 题型 × 单/多题 × 中/英 多样化（验收反馈 UI-2/UI-4）。
 * 每个示例含: 建议 model + state + questions。
 *
 * 命名约定：
 * - id: machine-readable slug
 * - label: 显示在 chip 上
 * - description: 鼠标 hover title
 * - payload: JevRequest (model + state + questions)
 * 题型仅契约入站三变体 choice/score/noul（contracts/01 §1 — 无 boolean）。
 *
 * 注：chip 点击**不再切换**用户的 model 选择（UI-1）——payload.model 仅作初始默认值记录。
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
  /* ---------- 英文 · 单题 ---------- */
  {
    id: 'support-routing',
    label: 'Support Routing',
    description: 'Route a support ticket to the right team (choice)',
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
    description: 'Is this refund request legitimate? (noul)',
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
    description: 'Score incident urgency from cosmetic to outage (score)',
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

  /* ---------- 中文 · 单题 ---------- */
  {
    id: 'zh-ticket-category',
    label: '中文·工单分类',
    description: '中文 choice 题：把客服工单分到正确队列',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        工单: {
          标题: '快递显示已签收但我没收到货',
          正文: '物流三天前显示本人签收，但我本人从未收到。包裹价值 399 元，希望尽快处理。',
          会员等级: '黄金',
        },
        渠道: 'App-在线客服',
      },
      questions: {
        分类: {
          type: 'choice',
          instructions: '这条工单应该分给哪个处理队列？',
          criteria: {
            物流: '查件、签收异常、配送延误',
            退款: '退款申请、拒收退款、仅退款',
            账号: '登录异常、账号安全、实名问题',
            商品: '商品质量、描述不符、维修换新',
          },
        },
      },
    },
  },
  {
    id: 'zh-sentiment',
    label: '中文·情感判断',
    description: '中文 noul 题：判断评论是否为负面反馈',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        评论: '整体还行，就是客服回得太慢了，问题拖了两天才解决，体验一般。',
        星级: 3,
        下单到评价间隔天数: 5,
      },
      questions: {
        是否负面: {
          type: 'noul',
          instructions: '这条评论是否应被标记为需要人工跟进的负面反馈？',
          criteria: { true: '是，负面且需跟进', false: '否，正常反馈' },
        },
      },
    },
  },
  {
    id: 'zh-risk-score',
    label: '中文·风险等级',
    description: '中文 score 题：给登录行为打风险分',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        登录: { 地区: '异国 IP（与常用地跨 8000km）', 设备: '新设备指纹', 时间: '凌晨 03:12' },
        账户: { 近 24 小时失败登录次数: 6, 是否开启二次验证: false, 余额: '较高' },
      },
      questions: {
        风险等级: {
          type: 'score',
          instructions: '请给这次登录行为评定风险等级（从低到高）。',
          criteria: ['低风险——常规登录', '中风险——轻微异常', '较高风险——多项异常', '高风险——疑似盗号', '极高风险——立即拦截'],
        },
      },
    },
  },

  /* ---------- 多题（真实多问题输入） ---------- */
  {
    id: 'en-churn-triage',
    label: 'Multi·Churn Risk',
    description: '英文多题：choice + noul + score 一次提交（3 questions）',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        account: { id: 'acc_9032', plan: 'pro', mrr: 49, tenure_months: 14 },
        signals: {
          logins_last_14d: 1,
          support_tickets_30d: 3,
          feature_usage_delta: '-62%',
          invoice_dispute: true,
        },
        note: 'User asked about export options twice this week.',
      },
      questions: {
        churn_driver: {
          type: 'choice',
          instructions: 'What is the most likely churn driver for this account?',
          criteria: {
            price: 'price / value for money',
            missing_feature: 'missing capability (e.g. export)',
            reliability: 'reliability or bugs',
            support: 'support experience',
          },
        },
        high_risk: {
          type: 'noul',
          instructions: 'Is this account at high risk of churning within 30 days?',
          criteria: { true: 'high risk — intervene now', false: 'low / moderate risk' },
        },
        priority: {
          type: 'score',
          instructions: 'What priority level should the retention outreach get?',
          criteria: [
            'no action needed',
            'low — automated email',
            'medium — CSM reaches out',
            'high — founder/exec call',
          ],
        },
      },
    },
  },
  {
    id: 'zh-content-moderation',
    label: '中文·内容审核',
    description: '中文多题：分类 + 是否违规 + 处置力度（3 questions）',
    payload: {
      model: DEFAULT_MODEL_LAYA,
      state: {
        帖子: {
          标题: '某明星演唱会门票转让，私聊有优惠',
          正文: '手里有两张内场票，原价转让，加微信 xxx 详聊，绝对真票。',
          账号注册天数: 3,
          历史违规次数: 1,
        },
      },
      questions: {
        内容类别: {
          type: 'choice',
          instructions: '这条帖子属于哪个内容类别？',
          criteria: {
            交易: '票务/商品交易',
            广告: '营销推广',
            正常: '普通讨论',
            违规: '欺诈或违禁信息',
          },
        },
        是否违规: {
          type: 'noul',
          instructions: '这条帖子是否构成需要处理的违规内容？',
          criteria: { true: '违规，需要处置', false: '不违规' },
        },
        处置力度: {
          type: 'score',
          instructions: '若需处置，给出处置力度（从轻到重）。',
          criteria: ['不处置', '仅记录观察', '限制展示', '下架并警告', '封号并上报'],
        },
      },
    },
  },
];
