'use strict';
// Shared by standalone Keys and the combined Miles Works account consent.
// Only metadata enters the DOM. Secret values are never fetched by this UI.
window.MilesKeyPermissions = function mountKeyPermissions(root, metadata) {
  const get = id => root.querySelector('#' + id);
  const t = (source, vars = {}) => window.MilesAccountI18n?.t(source, vars) ?? source.replace(/\{([A-Za-z][A-Za-z0-9_]*)\}/g, (match, key) => Object.prototype.hasOwnProperty.call(vars, key) ? String(vars[key]) : match);
  const scopes = new Set((metadata.scope || '').split(' '));
  const mayRead = scopes.has('keys:read'), mayWrite = scopes.has('keys:write');
  const broad = ['all-read', 'all-write', 'can-create'];
  const entries = metadata.vault?.entries || [];
  get('entries').replaceChildren(...entries.map(entry => {
    const row = document.createElement('tr'); row.dataset.name = entry.name;
    const name = document.createElement('td');
    name.textContent = entry.name + (entry.purpose ? ' — ' + entry.purpose : '') + (entry.disabled ? t(' (disabled)') : '');
    const fields = document.createElement('td'); fields.textContent = entry.fields.join(', ');
    row.append(name, fields);
    for (const permission of ['read', 'write']) {
      const cell = document.createElement('td'), label = document.createElement('label'), input = document.createElement('input');
      input.type = 'checkbox'; input.className = permission;
      input.setAttribute('aria-label', t(permission === 'read' ? 'Read {name}' : 'Change {name}', {name: entry.name}));
      input.disabled = permission === 'read' ? !mayRead : !mayWrite;
      label.append(input); cell.append(label); row.append(cell);
    }
    return row;
  }));
  get('empty-entries').hidden = entries.length > 0;
  for (const id of broad) {
    get(id).checked = false;
    get(id).disabled = id === 'all-read' ? !mayRead : !mayWrite;
  }
  get('broad-ack').checked = false;
  get('key-advanced').open = false;
  get('preset-note').hidden = false;
  get('preset-trusted').setAttribute('aria-pressed', 'false');
  get('preset-custom').setAttribute('aria-pressed', 'false');
  function payload() {
    return {
      all_read: mayRead && get('all-read').checked,
      all_write: mayWrite && get('all-write').checked,
      can_create: mayWrite && get('can-create').checked,
      entries: [...get('entries').querySelectorAll('tr')].map(row => ({
        name: row.dataset.name,
        read: mayRead && row.querySelector('.read').checked,
        write: mayWrite && row.querySelector('.write').checked,
      })).filter(entry => entry.read || entry.write),
    };
  }
  function render() {
    const selected = payload(), lines = [];
    // Only labels are refreshed; checkbox nodes and their values stay intact.
    for (const [index, row] of [...get('entries').querySelectorAll('tr')].entries()) {
      const entry = entries[index];
      row.firstElementChild.textContent = entry.name + (entry.purpose ? ' — ' + entry.purpose : '') + (entry.disabled ? t(' (disabled)') : '');
      row.querySelector('.read').setAttribute('aria-label', t('Read {name}', {name: entry.name}));
      row.querySelector('.write').setAttribute('aria-label', t('Change {name}', {name: entry.name}));
    }
    if (selected.all_read) lines.push(t('Read all secret values.'));
    if (selected.all_write) lines.push(t('Change values and remove fields.'));
    if (selected.can_create) lines.push(t('Create new entries.'));
    for (const entry of selected.entries) lines.push(t('{name}: {permissions}.', {name: entry.name, permissions: [entry.read ? t('read every field') : '', entry.write ? t('change every field') : ''].filter(Boolean).join('; ')}));
    if (!lines.length) lines.push(t('No keys access selected.'));
    get('permission-summary').replaceChildren(...lines.map(line => { const li = document.createElement('li'); li.textContent = line; return li; }));
    get('broad-confirm').hidden = !selected.all_read && !selected.all_write;
  }
  function changed() { get('broad-ack').checked = false; get('preset-note').hidden = true; render(); }
  for (const input of root.querySelectorAll('#all-read, #all-write, #can-create, #entries input')) input.onchange = () => {
    get('preset-trusted').setAttribute('aria-pressed', 'false');
    get('preset-custom').setAttribute('aria-pressed', 'true');
    changed();
  };
  get('preset-trusted').onclick = () => {
    for (const id of broad) get(id).checked = !get(id).disabled;
    for (const input of get('entries').querySelectorAll('input')) input.checked = false;
    get('key-advanced').open = false;
    get('preset-trusted').setAttribute('aria-pressed', 'true');
    get('preset-custom').setAttribute('aria-pressed', 'false');
    changed();
  };
  get('preset-custom').onclick = () => {
    for (const input of root.querySelectorAll('#all-read, #all-write, #can-create, #entries input')) input.checked = false;
    get('key-advanced').open = true;
    get('preset-trusted').setAttribute('aria-pressed', 'false');
    get('preset-custom').setAttribute('aria-pressed', 'true');
    changed();
  };
  render();
  return {
    payload,
    render,
    validate() {
      const selected = payload();
      if ((selected.all_read || selected.all_write) && !get('broad-ack').checked) {
        const source = 'Confirm access to all existing and future entries before allowing this connection.';
        const error = new Error(t(source)); error.i18nSource = source;
        throw error;
      }
      return selected;
    },
  };
};
