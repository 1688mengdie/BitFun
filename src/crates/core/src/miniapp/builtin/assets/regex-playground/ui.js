// Regex Playground — built-in MiniApp.
// Real-time matching, capture groups, replace preview, and a quick pattern library.

const PATTERN_LIBRARY = [
  { name: '邮箱地址', pattern: "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}", flags: 'g' },
  { name: '中国大陆手机号', pattern: "(?<!\\d)1[3-9]\\d{9}(?!\\d)", flags: 'g' },
  { name: 'URL（http/https）', pattern: "https?:\\/\\/[\\w\\-._~:\\/?#\\[\\]@!$&'()*+,;=%]+", flags: 'gi' },
  { name: 'IPv4 地址', pattern: "\\b(?:(?:25[0-5]|2[0-4]\\d|[01]?\\d?\\d)\\.){3}(?:25[0-5]|2[0-4]\\d|[01]?\\d?\\d)\\b", flags: 'g' },
  { name: 'IPv6 地址（简化）', pattern: "([0-9a-fA-F]{1,4}:){2,7}[0-9a-fA-F]{1,4}", flags: 'g' },
  { name: 'UUID v4', pattern: "[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}", flags: 'gi' },
  { name: '十六进制颜色', pattern: "#(?:[0-9a-fA-F]{3}){1,2}\\b", flags: 'g' },
  { name: '日期 YYYY-MM-DD', pattern: "\\b(\\d{4})-(0[1-9]|1[0-2])-(0[1-9]|[12]\\d|3[01])\\b", flags: 'g' },
  { name: '时间 HH:MM(:SS)', pattern: "\\b([01]?\\d|2[0-3]):[0-5]\\d(?::[0-5]\\d)?\\b", flags: 'g' },
  { name: 'Semver 版本号', pattern: "\\b\\d+\\.\\d+\\.\\d+(?:-[0-9A-Za-z.-]+)?(?:\\+[0-9A-Za-z.-]+)?\\b", flags: 'g' },
  { name: 'Git 短 SHA', pattern: "\\b[0-9a-f]{7,40}\\b", flags: 'g' },
  { name: '驼峰标识符', pattern: "\\b[a-z]+(?:[A-Z][a-z0-9]*)+\\b", flags: 'g' },
  { name: '中文字符', pattern: "[\\u4e00-\\u9fa5]+", flags: 'g' },
  { name: '前后空白', pattern: "^[ \\t]+|[ \\t]+$", flags: 'gm' },
  { name: '行首注释 //', pattern: "^\\s*\\/\\/.*$", flags: 'gm' },
];

const SAMPLE_TEXT = `# 把任意文本粘到这里，正则会实时高亮匹配项。

联系：alice@example.com / 13800138000
项目主页：https://github.com/cursor/bitfun
内部 IP：192.168.1.10 与 10.0.0.1
追踪 ID：8f2c3a01-4e6b-4d1c-9bb1-1f3a6d2c0a55
今日发版 v1.4.0-beta.2，对应 commit 7a3f9d2

// TODO: 抽取上面这一段为一个工具函数
const userName = "Bitfun";
`;

// ── DOM ──────────────────────────────────────────────
const dom = {
  pattern: document.getElementById('pattern'),
  flagsRow: document.getElementById('flags'),
  patternError: document.getElementById('pattern-error'),
  testText: document.getElementById('test-text'),
  highlight: document.getElementById('highlight'),
  matchCount: document.getElementById('match-count'),
  btnClear: document.getElementById('btn-clear'),
  matches: document.getElementById('matches'),
  library: document.getElementById('library'),
  replaceInput: document.getElementById('replace-input'),
  replaceOutput: document.getElementById('replace-output'),
  statusPill: document.getElementById('status-pill'),
};

const state = {
  flags: new Set(['g', 'm']),
  activeMatchIndex: -1,
};

// ── Init ─────────────────────────────────────────────
async function init() {
  buildLibrary();
  bindFlags();
  bindEditorSync();
  await restore();
  bindPersistence();
  recompute();
}

function buildLibrary() {
  dom.library.innerHTML = '';
  for (const item of PATTERN_LIBRARY) {
    const el = document.createElement('div');
    el.className = 'lib-item';
    el.innerHTML = `
      <div class="lib-item__name">${escapeHtml(item.name)}</div>
      <div class="lib-item__pattern">/${escapeHtml(item.pattern)}/${escapeHtml(item.flags)}</div>
    `;
    el.addEventListener('click', () => {
      dom.pattern.value = item.pattern;
      state.flags = new Set(item.flags.split(''));
      syncFlagsUi();
      recompute();
      dom.pattern.focus();
    });
    dom.library.appendChild(el);
  }
}

function bindFlags() {
  syncFlagsUi();
  dom.flagsRow.addEventListener('click', (e) => {
    const btn = e.target.closest('.flag');
    if (!btn) return;
    const f = btn.dataset.flag;
    if (state.flags.has(f)) state.flags.delete(f); else state.flags.add(f);
    syncFlagsUi();
    recompute();
  });
}

function syncFlagsUi() {
  for (const btn of dom.flagsRow.querySelectorAll('.flag')) {
    btn.classList.toggle('is-active', state.flags.has(btn.dataset.flag));
  }
}

function bindEditorSync() {
  // Sync scroll between textarea and the highlight overlay.
  dom.testText.addEventListener('scroll', () => {
    dom.highlight.scrollTop = dom.testText.scrollTop;
    dom.highlight.scrollLeft = dom.testText.scrollLeft;
  });
  dom.testText.addEventListener('input', recompute);
  dom.pattern.addEventListener('input', recompute);
  dom.replaceInput.addEventListener('input', renderReplace);
  dom.btnClear.addEventListener('click', () => {
    dom.testText.value = '';
    recompute();
    dom.testText.focus();
  });
}

async function restore() {
  let saved = null;
  try { saved = await app.storage.get('regex-state'); } catch (_e) { /* ignore */ }
  if (saved && typeof saved === 'object') {
    dom.pattern.value = typeof saved.pattern === 'string' ? saved.pattern : '';
    if (typeof saved.text === 'string') dom.testText.value = saved.text;
    if (typeof saved.replacement === 'string') dom.replaceInput.value = saved.replacement;
    if (Array.isArray(saved.flags) && saved.flags.length) state.flags = new Set(saved.flags);
    syncFlagsUi();
  }
  if (!dom.pattern.value) dom.pattern.value = "[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\\.[A-Za-z]{2,}";
  if (!dom.testText.value) dom.testText.value = SAMPLE_TEXT;
}

function bindPersistence() {
  const save = debounce(() => {
    app.storage.set('regex-state', {
      pattern: dom.pattern.value,
      text: dom.testText.value,
      replacement: dom.replaceInput.value,
      flags: Array.from(state.flags),
    }).catch(() => {});
  }, 350);
  for (const target of [dom.pattern, dom.testText, dom.replaceInput]) {
    target.addEventListener('input', save);
  }
  dom.flagsRow.addEventListener('click', save);
}

function debounce(fn, delay) {
  let t = null;
  return (...args) => {
    if (t) clearTimeout(t);
    t = setTimeout(() => fn(...args), delay);
  };
}

// ── Compile + match ──────────────────────────────────
function compileRegex() {
  const flagStr = Array.from(state.flags).join('');
  try {
    return { ok: true, regex: new RegExp(dom.pattern.value, flagStr) };
  } catch (e) {
    return { ok: false, error: String(e.message || e) };
  }
}

function findAllMatches(regex, text) {
  const out = [];
  if (!text) return out;
  const isGlobalLike = regex.global || regex.sticky;
  if (!isGlobalLike) {
    const m = regex.exec(text);
    if (m) out.push(matchSnapshot(m));
    return out;
  }
  let lastIndex = -1;
  let safety = 0;
  while (safety++ < 10000) {
    const m = regex.exec(text);
    if (!m) break;
    if (m.index === lastIndex && m[0] === '') {
      regex.lastIndex += 1;
      continue;
    }
    lastIndex = m.index;
    out.push(matchSnapshot(m));
    if (m[0] === '') regex.lastIndex += 1;
  }
  return out;
}

function matchSnapshot(m) {
  return {
    text: m[0],
    index: m.index,
    end: m.index + m[0].length,
    groups: m.slice(1).map((v, i) => ({ idx: i + 1, name: null, value: v })),
    namedGroups: m.groups ? Object.entries(m.groups).map(([k, v]) => ({ idx: null, name: k, value: v })) : [],
  };
}

function recompute() {
  const compiled = compileRegex();
  if (!compiled.ok) {
    dom.patternError.hidden = false;
    dom.patternError.textContent = compiled.error;
    dom.statusPill.textContent = '语法错误';
    dom.statusPill.className = 'status status--err';
    dom.matchCount.textContent = '— 处匹配';
    dom.matches.innerHTML = `<div class="empty">${escapeHtml(compiled.error)}</div>`;
    renderHighlight([]);
    renderReplace();
    return;
  }
  dom.patternError.hidden = true;
  dom.patternError.textContent = '';

  const text = dom.testText.value;
  const matches = findAllMatches(compiled.regex, text);
  dom.matchCount.textContent = `${matches.length} 处匹配`;
  if (matches.length === 0) {
    dom.statusPill.textContent = '无匹配';
    dom.statusPill.className = 'status status--idle';
  } else {
    dom.statusPill.textContent = `命中 ${matches.length} 处`;
    dom.statusPill.className = 'status status--ok';
  }
  state.activeMatchIndex = -1;
  renderHighlight(matches);
  renderMatches(matches);
  renderReplace();
}

// ── Render helpers ───────────────────────────────────
function renderHighlight(matches) {
  const text = dom.testText.value;
  if (matches.length === 0) {
    dom.highlight.innerHTML = escapeHtml(text) + '\n';
    return;
  }
  let html = '';
  let cursor = 0;
  matches.forEach((m, i) => {
    if (m.index > cursor) html += escapeHtml(text.slice(cursor, m.index));
    const cls = i === state.activeMatchIndex ? 'is-active' : '';
    html += `<mark data-idx="${i}" class="${cls}">${escapeHtml(text.slice(m.index, m.end))}</mark>`;
    cursor = m.end;
  });
  if (cursor < text.length) html += escapeHtml(text.slice(cursor));
  dom.highlight.innerHTML = html + '\n';
}

function renderMatches(matches) {
  if (matches.length === 0) {
    dom.matches.innerHTML = '<div class="empty">没有匹配项。试着调整正则或测试文本。</div>';
    return;
  }
  dom.matches.innerHTML = '';
  matches.forEach((m, i) => {
    const el = document.createElement('div');
    el.className = 'match';
    let groupsHtml = '';
    const allGroups = [...m.groups, ...m.namedGroups];
    if (allGroups.length > 0) {
      groupsHtml = '<div class="match__groups">' + allGroups.map((g) => {
        const tag = g.name != null ? `&lt;${escapeHtml(g.name)}&gt;` : `$${g.idx}`;
        const val = g.value === undefined ? '<i style="opacity:.5">undefined</i>' : escapeHtml(g.value);
        return `<div class="match__group"><b>${tag}</b> = ${val}</div>`;
      }).join('') + '</div>';
    }
    el.innerHTML = `
      <div class="match__head">
        <span class="match__index">#${i + 1}</span>
        <span>idx ${m.index}–${m.end}</span>
      </div>
      <div class="match__text">${escapeHtml(m.text) || '<i style="opacity:.5">空匹配</i>'}</div>
      ${groupsHtml}
    `;
    el.addEventListener('click', () => {
      state.activeMatchIndex = i;
      for (const node of dom.matches.querySelectorAll('.match')) node.classList.remove('is-active');
      el.classList.add('is-active');
      // Highlight active mark in overlay
      for (const mk of dom.highlight.querySelectorAll('mark')) mk.classList.remove('is-active');
      const target = dom.highlight.querySelector(`mark[data-idx="${i}"]`);
      if (target) target.classList.add('is-active');
      // Scroll textarea to the match
      const before = dom.testText.value.slice(0, m.index);
      const lineNo = before.split('\n').length - 1;
      const lineHeight = 13 * 1.55;
      dom.testText.scrollTop = Math.max(0, lineNo * lineHeight - 60);
      dom.testText.setSelectionRange(m.index, m.end);
      dom.testText.focus();
    });
    dom.matches.appendChild(el);
  });
}

function renderReplace() {
  const replacement = dom.replaceInput.value;
  if (replacement === '') { dom.replaceOutput.hidden = true; return; }
  const compiled = compileRegex();
  if (!compiled.ok) { dom.replaceOutput.hidden = true; return; }
  let result;
  try {
    result = dom.testText.value.replace(compiled.regex, replacement);
  } catch (e) {
    result = `[替换失败] ${e.message}`;
  }
  dom.replaceOutput.hidden = false;
  dom.replaceOutput.textContent = result;
}

function escapeHtml(s) {
  return String(s == null ? '' : s).replace(/[&<>]/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;',
  }[c]));
}

init();
