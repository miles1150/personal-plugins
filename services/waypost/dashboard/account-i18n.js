'use strict';
// Translate only explicit interface strings. Names, IDs, URLs and API metadata
// remain values; they are never scanned or treated as translation keys.
(() => {
  const chinese = {
    'Account · Miles Works': '账户 · Miles Works',
    'Your assistants, connected.': '让你的助手彼此连接。',
    'Account navigation': '账户导航',
    'Overview & Spaces': '概览与空间',
    'Assistants': '助手',
    'Assistant': '助手',
    'Connections': '连接',
    'Security & recovery': '安全与恢复',
    'Messages & materials': '消息与资料',
    'MILES WORKS · ACCOUNT': 'MILES WORKS · 账户',
    'Your workspace': '你的工作区',
    'Manage the people and devices that work with your assistants.': '管理与你的助手协作的人员和设备。',
    'Language': '语言',
    'Interface language': '界面语言',
    'Get ready to connect': '准备连接',
    'Continue': '继续',
    'Back to account': '返回账户',
    'Change workspace or assistant': '更换空间或助手',
    'Welcome to Miles Works': '欢迎使用 Miles Works',
    'Sign in, review access, then return to your assistant.': '登录并确认访问权限，然后返回你的助手。',
    'Review access before connecting your assistant.': '连接助手前，请确认访问权限。',
    'Review access': '确认访问权限',
    'After allowing access, return to {client}.': '允许访问后，请返回 {client}。',
    'Messages: send and receive messages, and read or change shared materials in this workspace. Account management is not included.': '消息：在此工作区中收发消息，以及读取或修改共享资料。不包含账户管理权限。',
    'Sign in to see your messages, keys and connections.': '登录以查看你的消息、密钥和连接。',
    'Continue to set up your personal workspace and an assistant. You will review permissions before connecting.': '继续设置个人工作区和助手。连接前，你可以确认访问权限。',
    'Continue with your existing workspace. No new workspace will be created.': '继续使用已有工作区，不会创建新工作区。',
    'Choose the workspace for this connection below.': '请在下方选择此连接使用的工作区。',
    'Your assistants could not be loaded. Retry before creating anything.': '无法加载你的助手。请先重试，再创建新助手。',
    'Continue to create an assistant for this connection. This does not grant access yet.': '继续为此连接创建助手。此步骤尚不会授予访问权限。',
    'Choose an existing assistant below. Reuse the same identity when reconnecting.': '请在下方选择已有助手。重新连接时，请沿用同一个身份。',
    'Connecting as {assistant} in {workspace}.': '以 {assistant} 的身份连接到 {workspace}。',
    'Complete setup above, then review the permissions below.': '请完成上方的设置，再确认下方的访问权限。',
    'Selected assistant': '所选助手',
    'This connection request expired or belongs to another browser. Restart Connect in your assistant and finish in the same browser.': '此连接请求已过期或属于另一个浏览器。请在助手中重新发起连接，并在同一浏览器中完成。',
    'Could not review this connection. Sign in again, then retry. No access was granted.': '无法查看此连接。请重新登录后重试。尚未授予访问权限。',
    'Sign in with Google': '使用 Google 登录',
    'Sign out': '退出登录',
    'Refresh': '刷新',
    'Your Spaces': '你的空间',
    'A Space is the boundary for messages and shared materials. Select one to manage its assistants and connections.': '每个空间分别管理消息和共享资料。选择空间后，即可管理其中的助手和连接。',
    'Sign in and select a Space to continue.': '请登录并选择一个空间以继续。',
    'Open messages & materials': '打开消息与资料',
    'Set up a personal Space': '设置个人空间',
    'This prepares your first personal Space. It does not create an additional project Space if one already exists.': '这会设置你的第一个个人空间。如果已有个人空间，则不会额外创建项目空间。',
    'Personal Space name': '个人空间名称',
    'Personal Space': '个人空间',
    'Set up personal Space': '设置个人空间',
    'Assistant identity': '助手身份',
    'Keep one identity when reconnecting the same assistant. Each device connects as that identity; creating an identity does not start an assistant or install a receiver.': '重新连接同一个助手时，请沿用原有身份。每台设备都以该身份连接；创建身份不会启动助手或安装接收器。',
    'Use an existing assistant': '使用现有助手',
    'Select a Space first': '请先选择空间',
    'Connection identity': '连接身份',
    'Choose an assistant first': '请先选择助手',
    'Create another assistant': '创建另一个助手',
    'Assistant name': '助手名称',
    'For example: Codex · research': '例如：Codex · 研究',
    'Create assistant identity': '创建助手身份',
    'Technical identity details': '身份技术详情',
    'Actor and Consumer IDs stay stable. Use these fields for an existing identity not yet loaded in the list.': 'Actor 和 Consumer ID 保持不变。若列表中尚未加载某个已有身份，可在此输入其 ID。',
    'Actor ID': 'Actor ID',
    'Consumer ID': 'Consumer ID',
    'A primary assistant is your preferred assistant. Choosing one does not grant extra permissions or change existing connections’ default recipients.': '首选助手是你优先使用的助手。设置首选助手不会授予额外权限，也不会改变已有连接的默认接收者。',
    'Make selected assistant primary': '将所选助手设为首选',
    'Clear primary assistant': '清除首选助手',
    'Connect your assistant': '连接你的助手',
    'Continue only if you started this connection. Client names are unverified.': '仅当此连接由你发起时才继续。客户端名称未经验证。',
    'This connects the selected identity to messages and shared materials in your Space. Account management stays private.': '这会让所选身份访问空间中的消息和共享资料。账户管理权限仍仅限你本人。',
    'Connection details': '连接详情',
    'Keys access': '密钥访问',
    'Permission presets': '权限预设',
    'Trusted assistant': '信任的助手',
    'Choose specific entries': '选择特定条目',
    'Choose an access option, then review and allow.': '选择访问方式，确认后允许连接。',
    'Allow this assistant to': '允许此助手执行',
    'Advanced permissions': '高级权限',
    'All-entry permissions': '所有条目权限',
    'Read all secret values': '读取所有机密值',
    'Change values and remove fields': '修改值和删除字段',
    'Create new entries': '创建新条目',
    'Entry permissions': '条目权限',
    'Entry': '条目',
    'Fields': '字段',
    'Read': '读取',
    'Change': '修改',
    'No entries yet. You can still choose creation or all-entry permissions above.': '暂无条目。你仍可在上方选择创建权限或所有条目权限。',
    'Entry access covers every field. All-entry permissions also include unchecked entries. Create access allows access to entries created by this connection, within the requested scope.': '条目访问权限涵盖所有字段。所有条目权限也包含未勾选的条目。创建权限允许在请求的范围内访问由此连接创建的条目。',
    'Secret values go to the assistant’s host and may enter model context. Revoking access cannot recall values already returned.': '机密值将发送到助手的运行主机，并可能进入模型上下文。撤销访问无法收回已经返回的值。',
    'I allow this access to all existing and future entries, including entries left unchecked.': '我允许访问所有现有和未来的条目，包括未勾选的条目。',
    'Review request': '查看请求',
    'Allow': '允许',
    'Deny': '拒绝',
    'Dismiss locally': '在此关闭',
    'Connect a device': '连接设备',
    'Choose an assistant above, then enter the code shown by your own client. Approve only a connection you started. This grants Space member access to messages and shared materials.': '先在上方选择助手，再输入你自己的客户端显示的代码。仅允许你发起的连接。这将授予空间成员级别的消息和共享资料访问权限。',
    'Device pairing': '设备配对',
    'Device code': '设备代码',
    'Review device': '查看设备',
    'Allow reviewed device': '允许已查看的设备',
    'OAuth device authorization': 'OAuth 设备授权',
    'For clients using the standard OAuth device flow. Use the code displayed by your own client and review the actual client details.': '适用于使用标准 OAuth 设备流程的客户端。请输入你自己的客户端显示的代码，并核对实际客户端详情。',
    'OAuth user code': 'OAuth 用户代码',
    'Review OAuth device': '查看 OAuth 设备',
    'Allow reviewed OAuth device': '允许已查看的 OAuth 设备',
    'Deny reviewed OAuth device': '拒绝已查看的 OAuth 设备',
    'Manage access': '管理访问权限',
    'Sign in within the last 15 minutes to manage access. Revoking prevents future calls; data already returned to a client cannot be recalled. Saved messages and materials remain.': '管理访问权限需要在最近 15 分钟内登录。撤销后将阻止后续调用，但无法收回已返回给客户端的数据。已保存的消息和资料会保留。',
    'Load paired devices': '加载已配对设备',
    'Connections and OAuth grants': '连接与 OAuth 授权',
    'Identifiers are shown without credentials. Review every connection when a device is lost.': '此处仅显示标识符，不显示凭据。设备丢失时，请检查每个连接。',
    'Load existing connections': '加载已有连接',
    'More connections': '更多连接',
    'Load OAuth grants': '加载 OAuth 授权',
    'More OAuth grants': '更多 OAuth 授权',
    'Revoking keys access preserves message access. Previously returned values cannot be recalled.': '撤销密钥访问不会影响消息访问权限。无法收回之前已返回的值。',
    'Load keys connections': '加载密钥连接',
    'Assistant and connection identities': '助手与连接身份',
    'Default owner identities are protected. Choose a different primary before disabling the current primary assistant.': '默认所有者身份受到保护。停用当前首选助手前，请先选择其他首选助手。',
    'Load assistant identities': '加载助手身份',
    'More assistant identities': '更多助手身份',
    'Load connection identities': '加载连接身份',
    'More connection identities': '更多连接身份',
    'Browser sign-in, assistant connections and your local receiver (bridge) are separate. Signing in does not install a receiver. Signing out ends only this browser session.': '浏览器登录、助手连接和本地接收器（桥接器）彼此独立。登录不会安装接收器。退出登录只会结束此浏览器会话。',
    'Claim or unlink an existing Space': '关联或取消关联已有空间',
    'Both a recent account sign-in and the existing owner key are required. Claiming keeps existing connections and the owner key. Unlinking revokes account-paired device connections; legacy connections remain.': '需要近期账户登录和现有所有者密钥。关联会保留已有连接和所有者密钥。取消关联会撤销通过账户配对的设备连接；旧版连接会保留。',
    'Space ID': '空间 ID',
    'Existing owner key': '现有所有者密钥',
    'Claim Space': '关联空间',
    'Unlink Space': '取消关联空间',
    'Emergency detach using owner key': '使用所有者密钥紧急解除关联',
    'Emergency detach uses the owner key without account sign-in. Keep your recovery key private.': '紧急解除关联仅需所有者密钥，无需账户登录。请妥善保管恢复密钥。',
    'Request timed out. Check your connection and try again.': '请求超时。请检查网络连接后重试。',
    'OAuth request failed. Sign in again if your session is older than 15 minutes, then review the request.': 'OAuth 请求失败。如果登录已超过 15 分钟，请重新登录后查看请求。',
    'Dynamic registration (client-supplied metadata)': '动态注册（客户端提供的元数据）',
    'URL metadata document (client-supplied metadata)': 'URL 元数据文档（客户端提供的元数据）',
    'Migrated / administrator-configured client': '迁移或管理员配置的客户端',
    'Client name (unverified): {name}': '客户端名称（未经验证）：{name}',
    'Client ID: {id}': '客户端 ID：{id}',
    'Client source: {source}': '客户端来源：{source}',
    'Approval identity: {actor} / {consumer}': '授权身份：{actor} / {consumer}',
    'Actual redirect URI: {uri}': '实际重定向 URI：{uri}',
    'Resource: {resource}': '资源：{resource}',
    'Scope: {scope}': '权限范围：{scope}',
    'Selected Space: {id}': '所选空间：{id}',
    'Not supplied': '未提供',
    'Unknown — not verified': '未知 — 未经验证',
    'Choose an assistant above': '请在上方选择助手',
    'Choose a connection identity above': '请在上方选择连接身份',
    'Cannot review the actual redirect destination. Restart authorization.': '无法查看实际重定向目标。请重新发起授权。',
    'Invalid authorization redirect destination.': '授权重定向目标无效。',
    '{client} will connect to {space}. Returns to {host}{local}.': '{client}将连接到{space}。授权后返回{host}{local}。',
    'This client': '此客户端',
    'the selected Space': '所选空间',
    ' on this computer': '（在此计算机上）',
    'Loopback callback.': '本机回调。',
    'Device authorization: no browser callback. Verify the code came from your own client.': '设备授权：没有浏览器回调。请确认代码来自你自己的客户端。',
    'None': '无',
    'none': '无',
    'Approval grants access to the selected Space, not account management.': '授权仅授予所选空间的访问权限，不包含账户管理权限。',
    'Request failed': '请求失败',
    'Sign in with Google to continue.': '请使用 Google 登录以继续。',
    'Opening Google…': '正在打开 Google…',
    'Opening Google sign-in…': '正在打开 Google 登录…',
    'Could not start sign-in. Please try again.': '无法发起登录。请重试。',
    'Could not start sign-in. Check your connection and try again.': '无法发起登录。请检查网络连接后重试。',
    'This identity will be used for approval. Review the client or device again after changing identities.': '将使用此身份授权。更改身份后，请重新查看客户端或设备。',
    'Choose both an assistant and a connection identity.': '请选择助手和连接身份。',
    'Choose an assistant': '选择助手',
    'Choose a connection identity': '选择连接身份',
    'Primary assistant: {name}': '首选助手：{name}',
    'Primary assistant: none': '首选助手：无',
    'Create an assistant identity to connect your first client.': '创建助手身份以连接你的第一个客户端。',
    'Load and choose an existing assistant first.': '请先加载并选择已有助手。',
    'Primary assistant updated. Existing connection recipients and permissions are unchanged.': '首选助手已更新。已有连接的接收者和权限保持不变。',
    'Sign in and select a Space above before opening its dashboard.': '请先登录并在上方选择空间，再打开其控制台。',
    'Signed in · {name}': '已登录 · {name}',
    'Signed in. Select a Space or set up your personal Space to continue.': '已登录。请选择空间或设置个人空间以继续。',
    'Open dashboard: {name} ({id})': '打开控制台：{name}（{id}）',
    'Open selected Space dashboard': '打开所选空间的控制台',
    'Selected Space: {name} ({id})': '所选空间：{name}（{id}）',
    'Select a Space above before opening its dashboard.': '请先在上方选择空间，再打开其控制台。',
    'Selected': '已选择',
    'Select': '选择',
    '{action}: {name} ({id})': '{action}：{name}（{id}）',
    'Selected: {name} ({id})': '已选择：{name}（{id}）',
    'Select: {name} ({id})': '选择：{name}（{id}）',
    'Selecting Space…': '正在选择空间…',
    'Identity list unavailable. Sign in again to load your existing assistants, or use technical identity details.': '暂时无法加载身份列表。请重新登录以加载已有助手，或使用身份技术详情。',
    'Enter a name for the assistant.': '请输入助手名称。',
    'Identity created. Its stable IDs are available in technical identity details; refresh to reload the list.': '身份已创建。可在身份技术详情中查看其固定 ID；刷新可重新加载列表。',
    'Assistant identity created and selected. Review your client request or device code to connect it.': '助手身份已创建并选中。请查看客户端请求或设备代码以连接。',
    'Inspect the device first.': '请先查看设备。',
    'Choose an assistant and connection identity first.': '请先选择助手和连接身份。',
    'Approved. Return to your device to finish pairing.': '已允许。请返回设备完成配对。',
    '{name} ({id}) {state} ': '{name}（{id}）{state} ',
    'Revoke': '撤销',
    'Review the request first.': '请先查看请求。',
    'Invalid authorization redirect.': '授权重定向无效。',
    'Inspect the OAuth device first.': '请先查看 OAuth 设备。',
    'OAuth device approved. Return to your client.': '已允许 OAuth 设备。请返回客户端。',
    'OAuth device denied.': '已拒绝 OAuth 设备。',
    'Disabled': '已停用',
    'Revoked': '已撤销',
    'Active': '有效',
    'Disabled{kind}': '已停用{kind}',
    'Revoked{kind}': '已撤销{kind}',
    'Active{kind}': '有效{kind}',
    'Technical details': '技术详情',
    'Disable identity': '停用身份',
    'Revoke access': '撤销访问',
    'Refresh the selected Space before changing access.': '请刷新所选空间后再更改访问权限。',
    'Identity disabled.': '身份已停用。',
    'Access revoked.': '访问权限已撤销。',
    'Use for approval': '用于授权',
    'Make primary': '设为首选',
    'Refresh Actors first.': '请先刷新助手身份。',
    'Load assistant identities first.': '请先加载助手身份。',
    'Read all existing and future entries': '读取所有现有和未来的条目',
    'Change all existing and future entries': '修改所有现有和未来的条目',
    'Create entries': '创建条目',
    'read': '读取',
    'change': '修改',
    ' and ': '和',
    'No keys permissions': '没有密钥权限',
    'Revoke keys access': '撤销密钥访问',
    'Keys access revoked. Message access is preserved.': '密钥访问已撤销。消息访问权限保留。',
    'No clients have keys access.': '没有客户端拥有密钥访问权限。',
    ' (disabled)': '（已停用）',
    'Read {name}': '读取{name}',
    'Change {name}': '修改{name}',
    'Read all secret values.': '读取所有机密值。',
    'Change values and remove fields.': '修改值和删除字段。',
    'Create new entries.': '创建新条目。',
    'read every field': '读取所有字段',
    'change every field': '修改所有字段',
    '{name}: {permissions}.': '{name}：{permissions}。',
    '{name}: {permissions}': '{name}：{permissions}',
    'No keys access selected.': '尚未选择密钥访问权限。',
    'Confirm access to all existing and future entries before allowing this connection.': '允许此连接前，请确认授予所有现有和未来条目的访问权限。',
  };
  const storageKey = 'miles-works.language';
  let savedLanguage;
  try { savedLanguage = localStorage.getItem(storageKey); } catch (_) {}
  let language = savedLanguage === 'en' || savedLanguage === 'zh'
    ? savedLanguage : /^zh/i.test(navigator.language || '') ? 'zh' : 'en';
  const bindings = new Map();
  const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
  function t(source, vars = {}) {
    const template = language === 'zh' && own(chinese, source) ? chinese[source] : source;
    return String(template).replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (match, key) => own(vars, key) ? String(vars[key]) : match);
  }
  function remember(element, attribute, source, vars) {
    let records = bindings.get(element);
    if (!records) bindings.set(element, records = new Map());
    const translated = t(source, vars);
    records.set(attribute, {source, vars: {...vars}, rendered: translated});
    if (attribute === 'textContent') element.textContent = translated;
    else element.setAttribute(attribute, translated);
    return element;
  }
  function text(element, source, vars = {}) { return remember(element, 'textContent', source, vars); }
  function bind(root = document) {
    const selector = '[data-i18n],[data-i18n-placeholder],[data-i18n-label]';
    const nodes = [...(root.matches?.(selector) ? [root] : []), ...root.querySelectorAll(selector)];
    for (const element of nodes) {
      for (const [data, attribute] of [['data-i18n','textContent'],['data-i18n-placeholder','placeholder'],['data-i18n-label','aria-label']]) {
        if (!element.hasAttribute(data)) continue;
        if (bindings.get(element)?.has(attribute)) continue;
        const source = element.getAttribute(data) || (attribute === 'textContent' ? element.textContent : element.getAttribute(attribute)) || '';
        remember(element, attribute, source, {});
      }
    }
  }
  function render() {
    document.documentElement.lang = language === 'zh' ? 'zh-CN' : 'en';
    document.title = t('Account · Miles Works');
    for (const [element, records] of bindings) {
      if (!element.isConnected) { bindings.delete(element); continue; }
      for (const [attribute, record] of records) {
        // Discard registrations superseded by unrelated application writes.
        const current = attribute === 'textContent' ? element.textContent : element.getAttribute(attribute);
        if (record.rendered !== undefined && current !== record.rendered) { records.delete(attribute); continue; }
        const translated = t(record.source, record.vars);
        if (attribute === 'textContent') element.textContent = translated;
        else element.setAttribute(attribute, translated);
        record.rendered = translated;
      }
    }
    const picker = document.getElementById('account-language');
    if (picker) picker.value = language;
  }
  function setLanguage(next) {
    if (next !== 'en' && next !== 'zh') return;
    const changed = language !== next;
    language = next;
    try { localStorage.setItem(storageKey, language); } catch (_) {}
    render();
    if (changed) window.dispatchEvent(new CustomEvent('miles-language-change', {detail: {language}}));
  }
  window.MilesAccountI18n = {t, text, bind, render, setLanguage, get language() { return language; }};
  function start() {
    bind();
    const picker = document.getElementById('account-language');
    if (picker) picker.addEventListener('change', () => setLanguage(picker.value));
    render();
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, {once: true});
  else start();
})();
