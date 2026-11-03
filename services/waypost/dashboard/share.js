'use strict';
(() => {
  const zh = /^zh/i.test(navigator.language || '');
  const T = zh ? {
    loading: '正在打开…', comment: '添加评论', comments: '评论', cancel: '取消',
    hint: '点一下你想评论的位置', name: '你的名字', body: '评论', post: '发布', discard: '放弃',
    spotText: (t) => `评论位置：${t}`, spotPlain: '评论位置：画面中你点的地方',
    draft: '新评论', reply: '回复', replyTo: '回复这条评论', empty: '还没有评论。点「添加评论」，再点设计里的位置。',
    viewOnly: '这个链接只能查看。', agent: '助手', failedPost: '没能发布，请再试一次。',
    offTitle: '链接还没打开', offBody: '分享的人还没打开这个链接。请等对方打开后再试。',
    expiredTitle: '链接已过期', expiredBody: (d) => `这个链接在 ${d} 到期。请向分享的人要一个新链接。`,
    revokedTitle: '链接已关闭', revokedBody: '分享的人关闭了这个链接。请向对方要一个新链接。',
    missingTitle: '找不到这个链接', missingBody: '请检查链接是否完整复制，或向分享的人要一个新链接。',
    errorTitle: '暂时打不开', errorBody: '网络或服务出了问题。', retry: '重试',
    until: (d) => `有效期至 ${d}`, version: (v) => `第 ${v} 版`,
  } : {
    loading: 'Opening…', comment: 'Add comment', comments: 'Comments', cancel: 'Cancel',
    hint: 'Click the spot you want to comment on', name: 'Your name', body: 'Comment', post: 'Post', discard: 'Discard',
    spotText: (t) => `On: ${t}`, spotPlain: 'On: the spot you clicked',
    draft: 'New comment', reply: 'Reply', replyTo: 'Reply to this comment', empty: 'No comments yet. Choose Add comment, then click a spot in the design.',
    viewOnly: 'This link is view only.', agent: 'Assistant', failedPost: 'Could not post. Try again.',
    offTitle: 'This link isn’t on yet', offBody: 'The person who shared it hasn’t turned it on. Try again after they do.',
    expiredTitle: 'This link expired', expiredBody: (d) => `It stopped working on ${d}. Ask the person who shared it for a new link.`,
    revokedTitle: 'This link was turned off', revokedBody: 'The person who shared it turned it off. Ask them for a new link.',
    missingTitle: 'Link not found', missingBody: 'Check that the whole link was copied, or ask the person who shared it for a new link.',
    errorTitle: 'Can’t open this right now', errorBody: 'Something went wrong with the network or the service.', retry: 'Retry',
    until: (d) => `Open until ${d}`, version: (v) => `Version ${v}`,
  };
  const $ = (id) => document.getElementById(id);
  const token = location.pathname.split('/')[2] || '';
  const base = '/s/' + encodeURIComponent(token);
  const fmt = (ms) => new Date(ms).toLocaleString(zh ? 'zh-CN' : undefined, { dateStyle: 'medium', timeStyle: 'short' });
  const frame = $('frame');
  let info = null, comments = [], spot = null, replyTo = null, active = null, picking = false;

  for (const [id, key] of [['add', 'comment'], ['cancel', 'cancel'], ['name-label', 'name'], ['body-label', 'body'], ['post', 'post'], ['discard', 'discard'], ['retry', 'retry'], ['panel-title', 'comments']]) $(id).textContent = T[key];
  $('hint').textContent = T.hint;
  try { $('name').value = localStorage.getItem('share.name') || ''; } catch { /* storage may be blocked */ }

  async function api(path, options) {
    const r = await fetch(base + path, options);
    const v = await r.json().catch(() => null);
    if (!r.ok || !v || !v.ok) { const e = new Error('request'); e.status = r.status; e.error = v && v.error; throw e; }
    return v.data;
  }
  function showState(title, body, retry) {
    $('state-title').textContent = title; $('state-body').textContent = body;
    $('retry').hidden = !retry; $('state').hidden = false; frame.hidden = true;
  }
  function failure(e) {
    const code = e.error && e.error.code;
    if (code === 'SHARE_LINK_OFF') return showState(T.offTitle, T.offBody, true);
    if (code === 'SHARE_LINK_EXPIRED') return showState(T.expiredTitle, T.expiredBody(fmt(e.error.details.expired_at)), false);
    if (code === 'SHARE_LINK_REVOKED') return showState(T.revokedTitle, T.revokedBody, false);
    if (e.status === 404) return showState(T.missingTitle, T.missingBody, false);
    showState(T.errorTitle, T.errorBody, true);
  }
  function send(msg) { if (frame.contentWindow) frame.contentWindow.postMessage(msg, '*'); }
  const tops = () => comments.filter((c) => !c.parent_id);
  function pins() {
    const list = tops().filter((c) => c.anchor).map((c) => ({ id: c.comment_id, n: tops().indexOf(c) + 1, x: c.anchor.x, y: c.anchor.y, view: c.anchor.view || '', selector: c.anchor.selector || '', rx: c.anchor.rx || 0, ry: c.anchor.ry || 0, active: c.comment_id === active }));
    if (spot && !$('form').hidden) list.push({ id: 'draft', n: '+', label: T.draft, x: spot.x, y: spot.y, view: spot.view, selector: spot.selector, rx: spot.rx, ry: spot.ry, active: true });
    send({ type: 'share:pins', pins: list });
  }
  function setPicking(on) {
    picking = on; send({ type: 'share:mode', on });
    $('hint').hidden = !on; $('cancel').hidden = !on; $('add').hidden = on || !canComment();
    if (on) $('cancel').focus();
  }
  const canComment = () => info && info.role === 'comment';
  function openPanel(open) {
    $('panel').hidden = !open; $('toggle').setAttribute('aria-expanded', String(open));
    document.body.classList.toggle('with-panel', open);
  }
  function compose(anchor, parent) {
    spot = anchor; replyTo = parent;
    $('spot').textContent = parent ? T.replyTo : anchor && anchor.text ? T.spotText(anchor.text) : T.spotPlain;
    $('form-error').textContent = ''; $('form').hidden = false; $('empty').hidden = true; openPanel(true); pins();
    ($('name').value ? $('body') : $('name')).focus();
  }
  function render() {
    const list = $('threads'); list.textContent = '';
    const all = tops();
    $('toggle').textContent = all.length ? `${T.comments} · ${all.length}` : T.comments;
    $('empty').hidden = all.length > 0 || !$('form').hidden; $('empty').textContent = T.empty;
    all.forEach((c, i) => {
      const li = document.createElement('li'); li.className = 'thread' + (c.comment_id === active ? ' active' : '');
      const head = document.createElement('button'); head.type = 'button'; head.className = 'thread-head';
      const n = document.createElement('span'); n.className = 'pin'; n.textContent = String(i + 1); n.setAttribute('aria-hidden', 'true');
      head.append(n, entry(c)); head.addEventListener('click', () => { active = c.comment_id; pins(); render(); send({ type: 'share:focus', id: c.comment_id }); });
      li.append(head);
      for (const r of comments.filter((x) => x.parent_id === c.comment_id)) { const d = entry(r); d.classList.add('reply'); li.append(d); }
      const reply = document.createElement('button'); reply.type = 'button'; reply.className = 'link'; reply.textContent = T.reply;
      reply.addEventListener('click', () => compose(null, c.comment_id)); li.append(reply);
      list.append(li);
    });
  }
  function entry(c) {
    const d = document.createElement('div'); d.className = 'entry';
    const who = document.createElement('p'); who.className = 'who';
    const name = document.createElement('strong'); name.textContent = c.author_kind === 'agent' ? `${c.author} · ${T.agent}` : c.author;
    const when = document.createElement('time'); when.dateTime = new Date(c.created_at).toISOString(); when.textContent = fmt(c.created_at);
    who.append(name, ' ', when);
    const body = document.createElement('p'); body.className = 'text'; body.textContent = c.body;
    d.append(who, body); return d;
  }
  async function loadComments() {
    if (!canComment()) return;
    comments = (await api('/comments')).comments; render(); pins();
  }
  async function start() {
    showState(T.loading, '', false);
    try { info = await api('/info'); } catch (e) { return failure(e); }
    document.title = info.title; $('title').textContent = info.title; frame.title = info.title; $('bar').hidden = false;
    $('state').hidden = true; frame.hidden = false; frame.src = base + '/content';
    if (canComment()) {
      $('add').hidden = false; $('toggle').hidden = false;
      try { await loadComments(); } catch (e) { failure(e); }
    }
  }
  addEventListener('message', (e) => {
    if (e.source !== frame.contentWindow) return;
    const m = e.data || {};
    if (m.type === 'share:ready') { pins(); if (picking) send({ type: 'share:mode', on: true }); }
    else if (m.type === 'share:point' && picking && typeof m.x === 'number' && typeof m.y === 'number') {
      setPicking(false);
      const unit = (n) => (typeof n === 'number' && n >= 0 && n <= 1 ? n : 0);
      const selector = typeof m.selector === 'string' ? m.selector.slice(0, 500) : '';
      compose({ x: unit(m.x), y: unit(m.y), view: typeof m.view === 'string' ? m.view.slice(0, 64) : '', text: typeof m.text === 'string' ? m.text.slice(0, 120) : '', selector, rx: selector ? unit(m.rx) : 0, ry: selector ? unit(m.ry) : 0 }, null);
    } else if (m.type === 'share:pin' && typeof m.id === 'string') { active = m.id; pins(); render(); openPanel(true); }
  });
  $('add').addEventListener('click', () => setPicking(true));
  $('cancel').addEventListener('click', () => setPicking(false));
  $('toggle').addEventListener('click', () => openPanel($('panel').hidden));
  $('close').addEventListener('click', () => openPanel(false));
  $('discard').addEventListener('click', () => { $('form').hidden = true; $('body').value = ''; spot = replyTo = null; render(); pins(); });
  $('retry').addEventListener('click', start);
  addEventListener('keydown', (e) => { if (e.key === 'Escape' && picking) setPicking(false); });
  $('form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const name = $('name').value.trim(), body = $('body').value.trim();
    if (!name || !body) return;
    $('post').disabled = true;
    try {
      await api('/comments', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(replyTo ? { name, body, parent_id: replyTo } : { name, body, anchor: spot }) });
      try { localStorage.setItem('share.name', name); } catch { /* ignore */ }
      $('body').value = ''; $('form').hidden = true; spot = replyTo = null;
      await loadComments();
    } catch (err) {
      $('form-error').textContent = err.error && err.status === 400 ? err.error.message : T.failedPost;
    } finally { $('post').disabled = false; }
  });
  start();
})();
