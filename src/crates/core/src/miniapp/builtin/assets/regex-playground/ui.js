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
  matchesSummary: document.getElementById('matches-summary'),
  btnPrevMatch: document.getElementById('btn-prev-match'),
  btnNextMatch: document.getElementById('btn-next-match'),
  library: document.getElementById('library'),
  replaceInput: document.getElementById('replace-input'),
  replaceOutput: document.getElementById('replace-output'),
  statusPill: document.getElementById('status-pill'),
};

let lastMatches = [];

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
  dom.btnPrevMatch.addEventListener('click', () => stepActiveMatch(-1));
  dom.btnNextMatch.addEventListener('click', () => stepActiveMatch(1));
}

function stepActiveMatch(delta) {
  if (lastMatches.length === 0) return;
  const next = state.activeMatchIndex < 0
    ? (delta > 0 ? 0 : lastMatches.length - 1)
    : (state.activeMatchIndex + delta + lastMatches.length) % lastMatches.length;
  selectMatch(next, true);
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
    dom.matchesSummary.textContent = '正则语法错误';
    dom.matches.innerHTML = `<div class="empty">${escapeHtml(compiled.error)}</div>`;
    lastMatches = [];
    state.activeMatchIndex = -1;
    updateNavButtons();
    renderHighlight([]);
    renderReplace();
    return;
  }
  dom.patternError.hidden = true;
  dom.patternError.textContent = '';

  const text = dom.testText.value;
  const matches = findAllMatches(compiled.regex, text);
  lastMatches = matches;
  dom.matchCount.textContent = `${matches.length} 处匹配`;
  if (matches.length === 0) {
    dom.statusPill.textContent = '无匹配';
    dom.statusPill.className = 'status status--idle';
    dom.matchesSummary.textContent = text ? '无匹配' : '尚未匹配';
  } else {
    dom.statusPill.textContent = `命中 ${matches.length} 处`;
    dom.statusPill.className = 'status status--ok';
    const totalGroups = matches.reduce((acc, m) => acc + m.groups.length + m.namedGroups.length, 0);
    dom.matchesSummary.textContent = totalGroups > 0
      ? `${matches.length} 处 · ${totalGroups} 个分组`
      : `${matches.length} 处`;
  }
  state.activeMatchIndex = -1;
  updateNavButtons();
  renderHighlight(matches);
  renderMatches(matches);
  renderReplace();
}

function updateNavButtons() {
  const has = lastMatches.length > 0;
  dom.btnPrevMatch.disabled = !has;
  dom.btnNextMatch.disabled = !has;
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
  const text = dom.testText.value;
  matches.forEach((m, i) => {
    const el = document.createElement('div');
    el.className = 'match';
    el.dataset.idx = String(i);
    const allGroups = [...m.groups, ...m.namedGroups];
    let groupsHtml = '';
    if (allGroups.length > 0) {
      groupsHtml = '<div class="match__groups">' + allGroups.map((g) => {
        const tag = g.name != null ? `&lt;${escapeHtml(g.name)}&gt;` : `$${g.idx}`;
        const val = g.value === undefined || g.value === ''
          ? `<span class="match__group-val match__group-val--empty">${g.value === undefined ? 'undefined' : '(空)'}</span>`
          : `<span class="match__group-val">${escapeHtml(g.value)}</span>`;
        return `<div class="match__group"><span class="match__group-tag">${tag}</span>${val}</div>`;
      }).join('') + '</div>';
    }
    const { line, col } = lineColAt(text, m.index);
    const isEmpty = m.text === '';
    const matchTextHtml = isEmpty
      ? '<div class="match__text match__text--empty">空匹配（零宽）</div>'
      : `<div class="match__text" title="${escapeHtml(m.text)}">${escapeHtml(m.text)}</div>`;
    el.innerHTML = `
      <div class="match__head">
        <span class="match__index">#${i + 1}</span>
        <span class="match__loc">L${line}:${col} · ${m.index}–${m.end}</span>
      </div>
      ${matchTextHtml}
      ${groupsHtml}
    `;
    el.addEventListener('click', () => selectMatch(i, true));
    dom.matches.appendChild(el);
  });
}

function selectMatch(i, scrollIntoText) {
  state.activeMatchIndex = i;
  const m = lastMatches[i];
  if (!m) return;
  for (const node of dom.matches.querySelectorAll('.match')) {
    node.classList.toggle('is-active', node.dataset.idx === String(i));
  }
  const activeCard = dom.matches.querySelector(`.match[data-idx="${i}"]`);
  if (activeCard) activeCard.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  for (const mk of dom.highlight.querySelectorAll('mark')) mk.classList.remove('is-active');
  const target = dom.highlight.querySelector(`mark[data-idx="${i}"]`);
  if (target) target.classList.add('is-active');
  if (scrollIntoText) {
    const before = dom.testText.value.slice(0, m.index);
    const lineNo = before.split('\n').length - 1;
    const lineHeight = 13 * 1.55;
    dom.testText.scrollTop = Math.max(0, lineNo * lineHeight - 60);
    dom.testText.setSelectionRange(m.index, m.end);
    dom.testText.focus();
  }
}

function lineColAt(text, offset) {
  let line = 1;
  let lastBreak = -1;
  for (let i = 0; i < offset; i++) {
    if (text.charCodeAt(i) === 10) { line += 1; lastBreak = i; }
  }
  return { line, col: offset - lastBreak };
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
