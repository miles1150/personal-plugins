'use strict';
(() => {
  let lang = 'en';
  try { lang = localStorage.getItem('miles-language') || (/^zh/i.test(navigator.language || '') ? 'zh' : 'en'); } catch { /* storage blocked */ }
  const zh = lang === 'zh';
  const T = zh ? {
    loginTitle: '登录后管理分享', loginBody: '分享链接只能由账户主人打开或关闭。', signIn: '使用 Google 登录',
    confirmTitle: '打开这个分享链接？', turnOn: '打开链接', notNow: '暂不打开',
    roleComment: '拿到链接的人可以查看并留下评论，评论时需要填写名字。', roleView: '拿到链接的人只能查看。',
    scope: (d) => `任何拿到链接的人都能看到这份设计，不需要登录。打开后 ${d} 天内有效，你可以随时关闭。`,
    listTitle: '分享的设计', listBody: '助手发布的设计快照和它们的链接。关闭的链接不能再打开。',
    empty: '还没有分享。让助手用 baoyu-share 发布一份设计。',
    kind: { canvas: '画布', slides: '幻灯片', page: '网页' }, version: (v) => `第 ${v} 版`, comments: (n) => `${n} 条评论`,
    comment: '可评论', view: '仅查看', off: '未打开', on: (d) => `已打开，有效期至 ${d}`, expired: (d) => `已于 ${d} 过期`, revoked: '已关闭',
    open: '打开', copy: '复制链接', copied: '已复制链接。', turnOff: '关闭链接', turnOnRow: '打开链接',
    confirmOff: '关闭后，这个链接将永久失效。确定关闭吗？', reauth: '为了安全，打开链接前请重新登录。',
    done: '链接已打开，可以把它发给别人了。', missing: '没有找到这个链接，或它已经处理过。', failed: '操作没有完成，请再试一次。',
  } : {
    loginTitle: 'Sign in to manage shares', loginBody: 'Only the account owner can turn share links on or off.', signIn: 'Sign in with Google',
    confirmTitle: 'Turn on this share link?', turnOn: 'Turn on link', notNow: 'Not now',
    roleComment: 'People with the link can view the design and leave comments under a name they type.', roleView: 'People with the link can only view the design.',
    scope: (d) => `Anyone with the link can see this design without signing in. It works for ${d} days after you turn it on, and you can turn it off at any time.`,
    listTitle: 'Shared designs', listBody: 'Design snapshots your assistants published, and their links. A link you turn off can’t be turned back on.',
    empty: 'Nothing shared yet. Ask an assistant to publish a design with baoyu-share.',
    kind: { canvas: 'Canvas', slides: 'Slides', page: 'Page' }, version: (v) => `version ${v}`, comments: (n) => `${n} comment${n === 1 ? '' : 's'}`,
    comment: 'Can comment', view: 'View only', off: 'Off', on: (d) => `On until ${d}`, expired: (d) => `Expired ${d}`, revoked: 'Turned off',
    open: 'Open', copy: 'Copy link', copied: 'Link copied.', turnOff: 'Turn off', turnOnRow: 'Turn on',
    confirmOff: 'Turning this link off stops it for good. Turn it off?', reauth: 'For safety, sign in again before turning on a link.',
    done: 'The link is on. You can send it now.', missing: 'This link wasn’t found, or it was already handled.', failed: 'That didn’t finish. Try again.',
  };
  document.documentElement.lang = zh ? 'zh-CN' : 'en';
  const $ = (id) => document.getElementById(id);
  const fmt = (ms) => new Date(ms).toLocaleString(zh ? 'zh-CN' : undefined, { dateStyle: 'medium', timeStyle: 'short' });
  const wanted = new URLSearchParams(location.search).get('link');
  let csrf = '', shares = [];
  for (const [id, key] of [['login-title', 'loginTitle'], ['login-body', 'loginBody'], ['sign-in', 'signIn'], ['confirm-title', 'confirmTitle'], ['turn-on', 'turnOn'], ['not-now', 'notNow'], ['list-title', 'listTitle'], ['list-body', 'listBody'], ['list-empty', 'empty']]) $(id).textContent = T[key];

  async function api(path, body) {
    const r = await fetch(path, body === undefined ? {} : { method: 'POST', headers: { 'content-type': 'application/json', 'x-waypost-csrf': csrf }, body: JSON.stringify(body) });
    const v = await r.json().catch(() => null);
    if (!r.ok || !v || !v.ok) { const e = new Error('request'); e.status = r.status; e.code = v && v.error && v.error.code; throw e; }
    return v.data;
  }
  const say = (text, ok) => { $('notice').textContent = text; $('notice').classList.toggle('ok', !!ok); };
  function signIn() {
    try { sessionStorage.setItem('works.share_return', location.pathname + location.search); } catch { /* ignore */ }
    api('/account/login', {}).then((d) => location.assign(d.authorization_url)).catch(() => say(T.failed));
  }
  function statusText(l) {
    if (l.status === 'active') return T.on(fmt(l.expires_at));
    if (l.status === 'expired') return T.expired(fmt(l.expires_at));
    if (l.status === 'revoked') return T.revoked;
    return T.off;
  }
  function button(label, onClick, primary) {
    const b = document.createElement('button'); b.type = 'button'; b.textContent = label; if (primary) b.className = 'primary';
    b.addEventListener('click', onClick); return b;
  }
  async function act(path, link) {
    try { await api(path, { link_id: link.link_id }); return true; }
    catch (e) {
      if (e.code === 'REAUTHENTICATION_REQUIRED' || e.status === 401) { say(T.reauth); $('login').hidden = false; return false; }
      say(e.status === 404 || e.status === 409 ? T.missing : T.failed); return false;
    }
  }
  function render() {
    const root = $('shares'); root.textContent = '';
    $('list-empty').hidden = shares.length > 0;
    for (const s of shares) {
      const section = document.createElement('section'); section.className = 'share';
      const h = document.createElement('h2'); h.textContent = s.title;
      const meta = document.createElement('p'); meta.className = 'share-meta';
      meta.textContent = [T.kind[s.kind] || s.kind, T.version(s.version), T.comments(s.comments)].join(' · ');
      const ul = document.createElement('ul'); ul.className = 'links';
      for (const l of s.links) {
        const li = document.createElement('li'); li.className = 'link-row';
        const what = document.createElement('p'); what.className = 'what';
        const role = document.createElement('strong'); role.textContent = l.role === 'comment' ? T.comment : T.view;
        const st = document.createElement('span'); st.textContent = statusText(l); if (l.status === 'active') st.className = 'status-on';
        what.append(role, st); li.append(what);
        if (l.status === 'pending') li.append(button(T.turnOnRow, () => focusLink(l.link_id), true));
        if (l.status === 'active') {
          li.append(button(T.copy, async () => { try { await navigator.clipboard.writeText(l.url); say(T.copied, true); } catch { prompt(T.copy, l.url); } }));
          const open = document.createElement('a'); open.href = l.url; open.target = '_blank'; open.rel = 'noopener noreferrer'; open.textContent = T.open; open.className = 'button';
          li.append(open);
          li.append(button(T.turnOff, async () => { if (confirm(T.confirmOff) && await act('/account/shares/links/revoke', l)) load(); }));
        }
        ul.append(li);
      }
      section.append(h, meta, ul); root.append(section);
    }
  }
  function focusLink(id) {
    const share = shares.find((s) => s.links.some((l) => l.link_id === id));
    const link = share && share.links.find((l) => l.link_id === id);
    if (!share || !link || link.status !== 'pending') { $('confirm').hidden = true; if (id === wanted) say(T.missing); return; }
    $('confirm-what').textContent = `${share.title} · ${T.version(share.version)} · ${link.role === 'comment' ? T.roleComment : T.roleView}`;
    $('confirm-scope').textContent = T.scope(link.days);
    $('preview').title = share.title;
    $('preview').src = '/account/shares/preview/' + encodeURIComponent(share.share_id);
    $('turn-on').onclick = async () => {
      $('turn-on').disabled = true;
      try { if (await act('/account/shares/links/activate', link)) { $('confirm').hidden = true; await load(); say(T.done, true); history.replaceState(null, '', location.pathname); } }
      finally { $('turn-on').disabled = false; }
    };
    $('not-now').onclick = () => { $('confirm').hidden = true; history.replaceState(null, '', location.pathname); };
    $('confirm').hidden = false; $('turn-on').focus();
  }
  async function load() { shares = (await api('/account/shares/data')).shares; render(); $('list').hidden = false; }
  $('sign-in').addEventListener('click', signIn);
  (async () => {
    try { csrf = (await api('/account/session')).csrf_token; }
    catch (e) { if (e.status === 401) { $('login').hidden = false; return; } say(T.failed); return; }
    try { await load(); if (wanted) focusLink(wanted); } catch { say(T.failed); }
  })();
})();
