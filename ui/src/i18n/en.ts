/**
 * i18n 英文词典（默认语言 — CDP 断言依赖此处英文字面，改动前先对 e2e 脚本）。
 * 键 = 点分命名空间；值可用 {name} 占位符插值。
 * 品牌/HTTP 术语不译：Jev-Switch / Run Jev / POST / GET / qid / key / UNSAVED 机读位见 zh 对应键。
 */
export const en = {
  /* common */
  'common.reload': 'Reload',
  'common.overwrite': 'Overwrite',
  'common.retry': 'Retry',
  'common.cancel': 'Cancel',
  'common.close': 'Close',
  'common.save': 'Save',
  'common.saving': 'Saving…',
  'common.delete': 'Delete',
  'common.remove': 'Remove',
  'common.refresh': 'Refresh',
  'common.required': 'required',
  'common.loadFailed': 'Failed to load: ',
  'common.addRow': '+ Add row',

  /* shell */
  'shell.navProviders': 'Providers',
  'shell.navRouting': 'Routing',
  'shell.navPlayground': 'Playground',
  'shell.daemonOk': 'daemon ok',
  'shell.daemonUnreachable': 'daemon unreachable',
  'shell.connecting': 'connecting…',
  'shell.themeToDark': 'Switch to dark theme (currently: light)',
  'shell.themeToLight': 'Switch to light theme (currently: dark)',
  'shell.langToZh': '切换到中文',
  'shell.langToEn': 'Switch to English',

  /* conflict banner */
  'conflict.text': 'Config was modified externally',

  /* providers page */
  'prov.title': 'Providers',
  'prov.add': '+ Add provider',
  'prov.pasteToml': 'Paste toml snippet',
  'prov.enabledCount': '{n} / {total} enabled',
  'prov.mockMode': 'mock-first · dev',
  'prov.liveMode': 'live daemon',
  'prov.empty': 'No providers yet.',
  'prov.emptyHint':
    'Paste a [providers.*] toml snippet, or see rs/providers.example.toml to add your first upstream.',
  'prov.idExists': 'id already exists: {id}',
  'prov.added': 'Added {id}',
  'prov.deleted': 'Deleted {id}',
  'prov.saveFailed': 'Save failed, rolled back',
  'prov.deleteFailed': 'Delete failed, rolled back',
  'prov.keySaved': 'Key saved — masked display only',
  'prov.keySaveFailed': 'Failed to save key',

  /* provider card */
  'card.untested': 'Untested',
  'card.disabled': 'Disabled',
  'card.probeFailed': 'probe failed',
  'card.degraded': 'Degraded (slow)',
  'card.healthy': 'Healthy',
  'card.lastUntested': 'last — · untested',
  'card.replaceKey': 'Replace key',
  'card.typeIdConfirm': 'type id to confirm',

  /* probe button */
  'probe.btn': 'Probe',
  'probe.loading': 'Probing…',

  /* key form */
  'key.newKey': 'new key',
  'key.maskedHint': 'masked input · no plaintext echo',

  /* add provider panel */
  'add.tabForm': 'Form',
  'add.idRequired': 'id is required',
  'add.kindRequired': 'kind is required',
  'add.baseHttp': 'base must start with http(s)://',
  'add.noSections': 'No [providers.<id>] sections found',
  'add.pasteHint': 'Paste a [providers.<id>] snippet',
  'add.parseHint': 'Parse errors fully shown · never half-imported',
  'add.parseAdd': 'Parse & Add',
  'add.submit': 'Add provider',
  'add.apiKeyHint': 'password only · no Show',

  /* admin login */
  'login.title': 'Admin login',
  'login.desc': 'Cloud-mode admin sign-in (session missing or expired)',
  'login.submit': 'Sign in',

  /* routing page */
  'routing.edges': '{n} edges',
  'routing.cycle': 'Route cycle detected — write rejected',
  'routing.noPair': 'No available left×right pair',
  'routing.imported': 'Imported example routes (UNSAVED → auto-save)',
  'routing.empty': 'No routes yet.',
  'routing.importExample': 'Import example',
  'routing.saveFailed': 'Save failed, rolled back',

  /* route table */
  'rt.title': 'Route table · all routes (keyboard path)',
  'rt.empty': 'No routes — click Add row or import the example',
  'rt.del': 'Del',
  'rt.footer': 'Changes PUT /v1/admin/routes after 400ms debounce · cycle → reject in red',

  /* edge inspector */
  'edge.hintKeep': 'empty = inherit',
  'edge.backspaceHint': 'Backspace / Delete also works',

  /* bipartite canvas */
  'canvas.dragTitle': 'Drop on a model port to pin; drop on card body = same-name pin',
  'canvas.noEndpoint': 'no endpoint — drag here to create a same-name endpoint',
  'canvas.pinTitle': 'Pinned endpoint: ({id}) {model} — address × model × token fixed',
  'canvas.passTitle': 'Passthrough endpoint: model = caller input (local/* → local/qwen…)',

  /* playground page */
  'pg.heroLead':
    'Route a single Jev SystemOne call across heterogeneous upstream models — Vercel AI Gateway, local Laya daemon, or any future provider. Switch the model field; the router handles protocol translation.',
  'pg.quickStart': 'Quick start',
  'pg.examples': 'Examples',

  /* test panel */
  'tp.form': 'Form',
  'tp.json': 'JSON',
  'tp.structured': 'structured',
  'tp.rawJson': 'raw JSON',
  'tp.input': 'Input',
  'tp.output': 'Output',
  'tp.questionsCount': '{n} question(s)',
  'tp.questionsUnknown': '? questions',
  'tp.running': 'Running…',
  'tp.waiting': '// [ · ]\n// waiting for input + Run Jev',
  'tp.estimate': 'est',
  'tp.stateParseFailed': 'state JSON parse failed: ',
  'tp.questionsParseFailed': 'questions JSON parse failed: ',
  'tp.errCapability': 'capability mismatch (422)',
  'tp.errRateLimit': 'upstream rate-limited (retryable) → returned 503',

  /* question form editor */
  'qf.typeChoice': 'Pick one option; returns probabilities for each',
  'qf.typeScore': 'Score along ordered levels',
  'qf.typeNoul': 'Yes/no — returns a 0–1 probability (true/false descriptions)',
  'qf.notObject': 'questions must be an object { qid: {...} }',
  'qf.qidNotObject': 'question "{qid}" is not an object',
  'qf.typeUnsupported':
    'question "{qid}" has unsupported type="{type}" (only choice/score/noul) — edit in JSON view',
  'qf.empty': 'questions is empty — at least 1 question required',
  'qf.qidEmpty': 'qid cannot be empty',
  'qf.qidDup': 'duplicate qid: "{id}" — rename it',
  'qf.jsonInvalid': 'Invalid JSON — showing last valid form, fix it in JSON view: ',
  'qf.notWrittenBack': '{err} (not written back yet)',
  'qf.keepOne': 'Keep at least 1 question',
  'qf.deleteQuestion': 'Delete this question',
  'qf.placeholderQuestion': 'The decision you want Jev to make.',
  'qf.trueMeaning': 'meaning when true',
  'qf.falseMeaning': 'meaning when false',
  'qf.choiceLabel': 'option description',
  'qf.levelLabel': 'level description',
  'qf.choicesHint': 'Choices · key = probability key / label = display text',
  'qf.levelsHint': 'Levels · ordered (low → high)',
  'qf.addQuestion': '+ Add question',
  'qf.addChoice': '+ Add choice',
  'qf.addChoiceMax': '+ Add choice (max 10)',
  'qf.addLevel': '+ Add level',
  'qf.loadingForm': 'loading form…',

  /* example chips */
  'ex.titleQuestions': '{desc} · {n} questions',

  /* api/admin parse errors */
  'api.missingKind': '[{id}] missing kind',
  'api.missingBase': '[{id}] missing base',
  'api.badSection': 'line {n}: unsupported section [{name}] (expected [providers.<id>])',
  'api.parseLine': 'line {n}: cannot parse "{line}"',
  'api.kvOutside': 'line {n}: key/value outside a [providers.<id>] section',
  'api.valueType': 'line {n}: {key} only supports quoted string or boolean',

  /* home（块 5） */
  'shell.navHome': 'Home',
  'home.systemCard': 'System',
  'home.runCard': 'Request',
  'home.uptime': 'uptime',
  'home.mode': 'mode',
  'home.bind': 'bind',
  'home.version': 'version',
  'home.envOverride': 'env will override file mode',
  'home.pwSet': 'admin password set',
  'home.pwUnset': 'admin password not set',
  'home.mockStatus': 'status: mock (live endpoint pending)',
  'home.routesSummary': '{models} models · {edges} edges · {prefix} prefix',
  'home.moreRows': '+{n} more',
  'home.modeSection': 'Mode switch',
  'home.modeHint': 'local: loopback, no token · cloud: Bearer token + admin password',
  'home.switchTo': 'Switch to {mode}?',
  'home.confirm': 'Confirm',
  'home.rebindOk': 'mode → {mode} · rebind {from} → {to} ok',
  'home.rebindSkipped': 'mode → {mode} · rebind skipped: {reason}',
  'home.modeFailed': 'mode switch failed: {msg}',
  'home.setPassword': 'Set admin password',
  'home.setPasswordHint':
    'Required once to activate cloud. Stored in config file (0600), never displayed.',
  'home.pwLabel': 'admin password',
  'home.activate': 'Activate',
  'home.pwFailed': 'activation failed: {msg}',
  'home.listenSection': 'Listen address',
  'home.listenEdit': 'Edit',
  'home.listenAuto': 'auto',
  'home.listenHint': 'ip:port, or auto to restore the mode default',
  'home.listenSaved': 'listen → {addr}',
  'home.listenFailed': 'listen change failed — old listener kept: {msg}',
  'home.appearanceSection': 'Appearance & language',
  'home.appearanceHint': 'Same toggles as the top bar.',
  'home.quickSection': 'Quick links',
  'home.tileRun': 'Run Jev ↗',
  'home.uptimeD': '{d}d {h}h {m}m',
  'home.uptimeH': '{h}h {m}m {s}s',
  'home.uptimeM': '{m}m {s}s',
  'home.uptimeS': '{s}s',
} as const;

export type MessageKey = keyof typeof en;
