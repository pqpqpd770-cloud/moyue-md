(function () {
  'use strict';

  const $ = (sel) => document.querySelector(sel);
  const root = document.documentElement;
  const state = {
    theme: 'dark',
    face: 'sans',
    fontSize: 17,
    toc: true,
    recent: [],
    meta: null,
    translation: '',
    lastSource: '',
    lastScope: '选中的内容',
    lastFromDocument: false,
    tocBeforePanel: false,
    comments: true,
    /* 每次翻译递增；只认最后一次的结果，避免切语言时旧请求把新结果覆盖掉 */
    translationRun: 0,
    currentRun: 0,
  };

  function ipc(msg) {
    if (window.ipc && window.ipc.postMessage) window.ipc.postMessage(JSON.stringify(msg));
  }

  function copyText(text) {
    if (!text) return;
    if (window.moyueCopy) window.moyueCopy(text);
  }

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  window.Moyue = { receive };

  function receive(raw) {
    let msg;
    try {
      msg = typeof raw === 'string' ? JSON.parse(raw) : raw;
    } catch (err) {
      return;
    }
    switch (msg.cmd) {
      case 'state': applyState(msg); break;
      case 'render': render(msg); break;
      case 'empty': showWelcome(msg); break;
      case 'toast': toast(msg.text, msg.kind); break;
      case 'drag': onDragState(msg.state); break;
      case 'translation': onTranslation(msg); break;
      case 'window': onWindowState(msg); break;
      case 'recent':
        state.recent = Array.isArray(msg.recent) ? msg.recent : [];
        renderRecent();
        break;
    }
  }

  function onWindowState(msg) {
    const maximized = !!msg.maximized;
    $('#w-max .i-max').classList.toggle('hidden', maximized);
    $('#w-max .i-restore').classList.toggle('hidden', !maximized);
    $('#w-max').title = maximized ? '还原' : '最大化';
    $('#w-max').setAttribute('aria-label', maximized ? '还原' : '最大化');
  }

  /* ================================================================ 状态 */

  function applyState(msg) {
    if (typeof msg.theme === 'string') state.theme = msg.theme;
    if (typeof msg.face === 'string') state.face = msg.face;
    if (typeof msg.fontSize === 'number') state.fontSize = msg.fontSize;
    if (typeof msg.toc === 'boolean') state.toc = msg.toc;
    if (Array.isArray(msg.recent)) state.recent = msg.recent;
    paintTheme();
    paintFace();
    applyFont();
    paintToc();
    renderRecent();
  }

  function markSeg(selector, value) {
    document.querySelectorAll(selector + ' button').forEach((button) => {
      button.setAttribute('aria-checked', String(button.dataset.value === value));
    });
  }

  function paintTheme() {
    const dark = state.theme !== 'light';
    root.classList.toggle('theme-dark', dark);
    root.classList.toggle('theme-light', !dark);
    markSeg('#seg-theme', dark ? 'dark' : 'light');
  }

  function paintFace() {
    const face = state.face === 'serif' ? 'serif' : 'sans';
    root.dataset.face = face;
    markSeg('#seg-face', face);
  }

  function applyFont() {
    root.style.setProperty('--base', state.fontSize + 'px');
    $('#font-value').textContent = String(state.fontSize);
  }

  function paintToc() {
    $('#toc').classList.toggle('collapsed', !state.toc);
    $('#t-toc').classList.toggle('on', state.toc);
    $('#t-toc').setAttribute('aria-pressed', String(state.toc));
  }

  function setTheme(theme) {
    if (theme === state.theme) return;
    state.theme = theme;
    paintTheme();
    ipc({ cmd: 'theme', value: theme });
  }

  function setFace(face) {
    if (face === state.face) return;
    state.face = face;
    paintFace();
    ipc({ cmd: 'face', value: face });
  }

  /* ================================================================ 正文 */

  function render(msg) {
    const view = $('#view');
    const keep = msg.keepScroll ? readRatio() : 0;

    document.body.classList.remove('no-doc', 'boot');
    $('#toc').classList.remove('away');

    const doc = $('#doc');
    const html = String(msg.html || '').trim();
    doc.innerHTML = html ? msg.html : '<p class="empty-doc">这个文件是空的。</p>';
    if (window.moyueDecorate) window.moyueDecorate(doc);
    buildToc();
    applyMeta(msg.meta);
    tuneZoomAnimation();

    view.scrollTop = 0;
    requestAnimationFrame(onScrollFrame);
    if (keep) {
      requestAnimationFrame(() => {
        view.scrollTop = (view.scrollHeight - view.clientHeight) * keep;
        onScrollFrame();
      });
    } else {
      // Late font loading changes the metrics and Chromium then re-anchors the
      // scroll position, which used to leave a freshly opened file part way down.
      holdAtTop();
    }
    const query = $('#find-input').value;
    if (query && !$('#finder').classList.contains('hidden')) runFind(query, true);
  }

  let holdTopUntil = 0;

  /** Undo Chromium's scroll re-anchoring right after a fresh render. */
  function holdAtTop() {
    holdTopUntil = Date.now() + 2400;
    const snap = () => {
      if (Date.now() > holdTopUntil) return;
      const view = $('#view');
      if (view.scrollTop !== 0) view.scrollTop = 0;
      onScrollFrame();
    };
    window.setTimeout(snap, 120);
    window.setTimeout(snap, 600);
    if (document.fonts && document.fonts.ready) document.fonts.ready.then(snap);
  }

  function showWelcome(msg) {
    $('#doc').innerHTML = '';
    document.body.classList.remove('boot');
    document.body.classList.add('no-doc');
    $('#toc').classList.add('away');
    $('#toc-list').innerHTML = '';
    $('#toc').classList.add('empty');
    heads = [];
    tocItems = [];
    activeIndex = -2;
    setCrumbSection('');
    $('#chip-name').textContent = '';
    state.meta = null;
    if (Array.isArray(msg.recent)) state.recent = msg.recent;
    renderRecent();
    hideMenu();
    root.style.setProperty('--seen', '0%');
  }

  function applyMeta(meta) {
    state.meta = meta || null;
    if (!meta) return;
    $('#chip-name').textContent = meta.name || '';
    $('#t-file').title = meta.path || '';
  }

  function thousand(value) {
    return String(value).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  }

  function readRatio() {
    const view = $('#view');
    const span = view.scrollHeight - view.clientHeight;
    return span > 0 ? view.scrollTop / span : 0;
  }

  /* ------------------------------------------- 阅读进度与书眉（同一帧更新） */

  let scrollQueued = false;

  function onScroll() {
    if (scrollQueued) return;
    scrollQueued = true;
    requestAnimationFrame(() => {
      scrollQueued = false;
      onScrollFrame();
    });
  }

  function onScrollFrame() {
    updateReadline();
    updateSection();
  }

  function updateReadline() {
    if (document.body.classList.contains('no-doc')) return;
    const view = $('#view');
    const span = view.scrollHeight - view.clientHeight;
    const ratio = span > 2 ? Math.max(0, Math.min(1, view.scrollTop / span)) : 0;
    root.style.setProperty('--seen', (ratio * 100).toFixed(2) + '%');
  }

  $('#view').addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('resize', onScroll);
  if (window.ResizeObserver) new ResizeObserver(onScroll).observe($('#page'));

  /* ================================================================ 首页 */

  function pathLine(path, className) {
    // 路径太长时从左边省略，保留最有用的末尾几级目录
    const line = el('span', className);
    const inner = el('bdi', '', path);
    line.appendChild(inner);
    return line;
  }

  function renderRecent() {
    const wrap = $('#recent-wrap');
    const list = $('#recent-list');
    list.innerHTML = '';
    if (!state.recent.length) {
      wrap.classList.add('hidden');
      return;
    }
    wrap.classList.remove('hidden');
    state.recent.slice(0, 8).forEach((path) => {
      const li = document.createElement('li');
      const button = el('button', 'recent-item');
      button.type = 'button';
      button.title = path;
      button.append(el('span', 'rname', baseName(path)), pathLine(dirName(path), 'rpath'));
      button.addEventListener('click', () => ipc({ cmd: 'openPath', value: path }));
      li.appendChild(button);
      list.appendChild(li);
    });
  }

  function baseName(path) {
    const parts = String(path).split(/[\\/]/);
    return parts[parts.length - 1] || path;
  }

  function dirName(path) {
    const parts = String(path).split(/[\\/]/);
    parts.pop();
    return parts.join('\\');
  }

  function samePath(a, b) {
    return !!a && !!b && String(a).toLowerCase() === String(b).toLowerCase();
  }

  /* ================================================================ 目录 */

  let heads = [];
  let tocItems = [];
  let titleIndex = -1;
  let activeIndex = -2;

  function buildToc() {
    const list = $('#toc-list');
    const toc = $('#toc');
    list.innerHTML = '';
    heads = [...$('#doc').querySelectorAll('h1, h2, h3, h4, h5, h6')];
    tocItems = [];
    titleIndex = -1;
    activeIndex = -2;
    toc.classList.toggle('empty', heads.length === 0);
    if (!heads.length) {
      setCrumbSection('');
      return;
    }

    // 大多数文档是“一个 H1 当标题，下面全是 H2”。这时 H1 单独当题目，
    // 其余按相对层级缩进，而不是让每一项都平白多缩一级。
    const levels = heads.map((head) => Number(head.tagName[1]));
    let top = Math.min(...levels);
    const topCount = levels.filter((level) => level === top).length;
    if (heads.length > 1 && levels[0] === top && topCount === 1) {
      titleIndex = 0;
      top = Math.min(...levels.slice(1));
    }

    heads.forEach((head, index) => {
      if (!head.id) head.id = 'sec-' + (index + 1);
      const depth = index === titleIndex ? 0 : Math.min(3, Math.max(0, levels[index] - top));
      const label = (head.textContent || '').trim() || '（空标题）';
      const item = el('button', 'toc-item', label);
      item.type = 'button';
      if (index === titleIndex) item.classList.add('title');
      if (depth >= 2) item.classList.add('deep');
      item.style.setProperty('--depth', String(depth));
      item.title = label;
      item.addEventListener('click', () => head.scrollIntoView({ block: 'start', behavior: 'smooth' }));
      list.appendChild(item);
      tocItems.push(item);
    });
  }

  /** The heading whose section is at the top of the viewport. */
  function currentHeadIndex() {
    if (!heads.length) return -1;
    const view = $('#view');
    const box = view.getBoundingClientRect();
    const limit = box.top + 80;
    let lo = 0;
    let hi = heads.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (heads[mid].getBoundingClientRect().top <= limit) {
        found = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
    // 滚到底时最后几节可能永远到不了顶部，就取视口里最后一个标题
    if (view.scrollTop > 0 && view.scrollTop >= view.scrollHeight - view.clientHeight - 2) {
      for (let i = heads.length - 1; i > found; i -= 1) {
        if (heads[i].getBoundingClientRect().top < box.bottom - 40) {
          found = i;
          break;
        }
      }
    }
    return found;
  }

  function updateSection() {
    const index = currentHeadIndex();
    if (index === activeIndex) return;
    activeIndex = index;
    tocItems.forEach((item, i) => item.classList.toggle('active', i === index));

    const item = tocItems[index];
    const list = $('#toc-list');
    if (item && state.toc) {
      const top = item.offsetTop - list.offsetTop;
      const bottom = top + item.offsetHeight;
      if (top < list.scrollTop + 8) list.scrollTop = Math.max(0, top - 24);
      else if (bottom > list.scrollTop + list.clientHeight - 8) list.scrollTop = bottom - list.clientHeight + 24;
    }

    const label = index >= 0 && index !== titleIndex ? (heads[index].textContent || '').trim() : '';
    setCrumbSection(label);
  }

  let crumbTimer = 0;

  function setCrumbSection(label) {
    const crumb = $('#crumb');
    const sec = $('#crumb-sec');
    window.clearTimeout(crumbTimer);
    crumb.classList.toggle('no-sec', !label);
    if (!label) {
      sec.textContent = '';
      sec.classList.remove('swap');
      return;
    }
    if (sec.textContent === label) return;
    if (!sec.textContent) {
      sec.textContent = label;
      return;
    }
    // 换节时淡出再淡入，像翻到新的一页
    sec.classList.add('swap');
    crumbTimer = window.setTimeout(() => {
      sec.textContent = label;
      sec.classList.remove('swap');
    }, 140);
  }

  /* ================================================================ 查找 */

  let findRanges = [];
  let findIndex = 0;

  function runFind(query, keepIndex) {
    clearFind();
    $('#finder').classList.remove('miss');
    if (!query) return;
    const docRoot = $('#doc');
    const needle = query.toLowerCase();
    const walker = document.createTreeWalker(docRoot, NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        const parent = node.parentElement;
        if (!parent || parent.closest('.codebar')) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    const nodes = [];
    let node;
    while ((node = walker.nextNode())) nodes.push(node);

    nodes.forEach((textNode) => {
      const text = textNode.nodeValue;
      const lower = text.toLowerCase();
      let from = 0;
      let at;
      while ((at = lower.indexOf(needle, from)) !== -1) {
        const range = document.createRange();
        range.setStart(textNode, at);
        range.setEnd(textNode, at + query.length);
        findRanges.push(range);
        from = at + query.length;
      }
    });

    if (!keepIndex) findIndex = 0;
    if (findIndex >= findRanges.length) findIndex = 0;
    $('#finder').classList.toggle('miss', findRanges.length === 0);
    paintFind();
    scrollToMatch();
  }

  function paintFind() {
    if (window.Highlight && CSS.highlights) {
      CSS.highlights.set('moyue-find', new Highlight(...findRanges));
      if (findRanges[findIndex]) {
        CSS.highlights.set('moyue-find-current', new Highlight(findRanges[findIndex]));
      }
    }
    $('#find-count').textContent = findRanges.length ? findIndex + 1 + '/' + findRanges.length : '0/0';
  }

  function clearFind() {
    findRanges = [];
    if (window.Highlight && CSS.highlights) {
      CSS.highlights.delete('moyue-find');
      CSS.highlights.delete('moyue-find-current');
    }
    $('#find-count').textContent = '0/0';
  }

  function scrollToMatch() {
    const range = findRanges[findIndex];
    if (!range) return;
    const node = range.startContainer;
    const target = node.nodeType === 1 ? node : node.parentElement;
    if (target) target.scrollIntoView({ block: 'center' });
  }

  function stepFind(delta) {
    if (!findRanges.length) return;
    findIndex = (findIndex + delta + findRanges.length) % findRanges.length;
    paintFind();
    scrollToMatch();
  }

  function openFind() {
    if (document.body.classList.contains('no-doc')) return;
    $('#finder').classList.remove('hidden');
    const input = $('#find-input');
    const selection = String(window.getSelection() || '').trim();
    if (selection && selection.length < 80 && !selection.includes('\n')) input.value = selection;
    input.focus();
    input.select();
    if (input.value) runFind(input.value, true);
  }

  function closeFind() {
    $('#finder').classList.add('hidden');
    $('#finder').classList.remove('miss');
    clearFind();
  }

  $('#find-input').addEventListener('input', (event) => runFind(event.target.value, false));
  $('#find-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      stepFind(event.shiftKey ? -1 : 1);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closeFind();
    }
  });
  $('#find-next').addEventListener('click', () => stepFind(1));
  $('#find-prev').addEventListener('click', () => stepFind(-1));
  $('#find-close').addEventListener('click', closeFind);

  /* ================================================================ 翻译 */

  /*
   * 改字号会让整篇文字按新尺寸重新光栅化。短文档一次几毫秒，平滑过渡很好看；
   * 长文档实测一帧能到 80ms（排版只占 4ms，其余是文字重绘），逐帧跑就是连卡。
   * 所以按体量挑时长：短文档 180ms 过渡，中等 100ms，长文档直接跳变
   * ——跳一帧比连卡十几帧舒服。
   */
  function tuneZoomAnimation() {
    const chars = ($('#doc').textContent || '').length;
    const cost = chars > 2500 ? '0s' : chars > 900 ? '.1s' : '.18s';
    root.style.setProperty('--zoom-anim', cost);
    root.dataset.zoomAnim = cost;
  }

  const LANGUAGE_LABELS = {
    'zh-CN': '简体中文',
    'zh-TW': '繁體中文',
    en: 'English',
    ja: '日本語',
    ko: '한국어',
    ru: 'Русский',
    de: 'Deutsch',
    fr: 'Français',
    es: 'Español',
    it: 'Italiano',
  };

  function onTranslation(msg) {
    /* 迟到的旧请求：直接丢掉，否则切语言时两批结果会互相覆盖、看起来"一直在翻译" */
    if (typeof msg.run === 'number' && msg.run !== state.currentRun) return;
    if (msg.state === 'start') {
      openPanel();
      showPanelState('正在翻译…');
    } else if (msg.state === 'progress') {
      if (msg.total > 1) showPanelState('正在翻译，第 ' + Math.min(msg.done + 1, msg.total) + ' 段，共 ' + msg.total + ' 段');
    } else if (msg.state === 'done') {
      state.translation = msg.markdown || '';
      if (msg.target && LANGUAGE_LABELS[msg.target]) $('#panel-lang').value = msg.target;
      const md = $('#panel-md');
      md.innerHTML = msg.html || '';
      if (window.moyueDecorate) window.moyueDecorate(md);
      md.classList.remove('hidden');
      $('#panel-state').classList.add('hidden');
      $('#panel-body').scrollTop = 0;
      if (state.lastFromDocument) {
        const extra = msg.comments > 0 ? `，另外翻了 ${msg.comments} 条代码注释` : '';
        toast('全文已译好，标题、列表和代码块都保留了' + extra, 'ok');
      }
    } else if (msg.state === 'error') {
      showPanelState((msg.message || '翻译失败') + '。检查一下网络，或者稍后再试。', true);
    }
  }

  /** The state line is a sibling of the rendered result, never replaces it. */
  function showPanelState(text, isError) {
    const note = $('#panel-state');
    $('#panel-state-text').textContent = text;
    note.classList.toggle('error', !!isError);
    note.classList.remove('hidden');
    $('#panel-md').classList.add('hidden');
    $('#panel-body').scrollTop = 0;
  }

  function openPanel() {
    // 译文面板和目录同时占位置会把正文挤瘦，所以打开译文时先把目录收起来
    if (state.toc && !$('#toc').classList.contains('away')) {
      state.tocBeforePanel = true;
      state.toc = false;
      paintToc();
    }
    $('#panel').classList.add('on');
  }

  function closePanel() {
    $('#panel').classList.remove('on');
    if (state.tocBeforePanel) {
      state.tocBeforePanel = false;
      state.toc = true;
      paintToc();
    }
    state.translation = '';
    $('#panel-md').innerHTML = '';
    $('#panel-md').classList.add('hidden');
    $('#panel-state').classList.add('hidden');
  }

  function requestTranslation(options) {
    const settings = options && options.text !== undefined ? options : {};
    let text = settings.text;
    let scope = settings.scope;
    let fromDocument = settings.fromDocument;

    if (text === undefined) {
      const selection = String(window.getSelection() || '').trim();
      text = selection;
      scope = '选中的内容';
      fromDocument = false;
      if (!text) {
        return requestDocumentTranslation();
      }
    }
    if (!text && !fromDocument) {
      toast('先打开一个文档，或者选中要翻译的文字', 'err');
      return;
    }
    if (fromDocument && !state.meta) {
      toast('先打开一个文档，或者选中要翻译的文字', 'err');
      return;
    }

    state.lastSource = text || ' ';
    state.lastScope = scope;
    state.lastFromDocument = !!fromDocument;
    const run = (state.translationRun += 1);
    state.currentRun = run;

    openPanel();
    showPanelState('正在翻译' + scope + '…');
    ipc({
      cmd: 'translate',
      value: {
        text: fromDocument ? '' : text,
        target: $('#panel-lang').value || 'auto',
        fromDocument: !!fromDocument,
        comments: state.comments,
        run,
      },
    });
  }

  /** Translate the whole document from its markdown source when we have it. */
  function requestDocumentTranslation() {
    requestTranslation({ text: '', scope: '全文', fromDocument: true });
  }

  /*
   * 重新翻译前先等一下：下拉框每按一次方向键都会触发 change，
   * 不防抖的话一次浏览就会连发好几次翻译。
   */
  let retranslateTimer = 0;
  function retranslate(delay) {
    if (!state.lastSource) return;
    window.clearTimeout(retranslateTimer);
    retranslateTimer = window.setTimeout(() => {
      requestTranslation({
        text: state.lastFromDocument ? '' : state.lastSource,
        scope: state.lastScope,
        fromDocument: state.lastFromDocument,
      });
    }, delay);
  }

  $('#panel-lang').addEventListener('change', () => retranslate(260));

  /* 代码块里的注释要不要一起翻：切换后用新设置重译 */
  $('#panel-comments').addEventListener('click', () => {
    state.comments = !state.comments;
    $('#panel-comments').setAttribute('aria-pressed', String(state.comments));
    retranslate(120);
  });

  $('#panel-close').addEventListener('click', closePanel);
  $('#panel-copy').addEventListener('click', () => {
    if (!state.translation) return;
    copyText(state.translation);
    toast('译文已复制', 'ok');
  });

  /* ============================================================ 弹出菜单 */

  const menu = $('#menu');
  const prefs = $('#prefs');
  let menuAnchor = null;

  function place(node, x, y, alignRight) {
    node.classList.remove('hidden');
    node.style.left = '0px';
    node.style.top = '0px';
    const width = node.offsetWidth;
    const height = node.offsetHeight;
    const left = alignRight ? x - width : x;
    node.style.left = Math.max(6, Math.min(left, window.innerWidth - width - 6)) + 'px';
    node.style.top = Math.max(6, Math.min(y, window.innerHeight - height - 6)) + 'px';
    node.style.transformOrigin = alignRight ? 'top right' : 'top left';
  }

  function hideMenu() {
    menu.classList.add('hidden');
    if (menuAnchor) menuAnchor.setAttribute('aria-expanded', 'false');
    menuAnchor = null;
  }

  function hidePrefs() {
    prefs.classList.add('hidden');
    $('#t-prefs').setAttribute('aria-expanded', 'false');
  }

  /*
   * items: { label, kbd, run, disabled } | { file: path, run } |
   *        { sep: true } | { head: text } | { empty: text } | { node }
   */
  function fillMenu(items) {
    menu.innerHTML = '';
    let lastWasSep = true;
    items.forEach((item, index) => {
      if (item.sep) {
        if (lastWasSep || index === items.length - 1) return;
        menu.appendChild(el('div', 'menu-sep'));
        lastWasSep = true;
        return;
      }
      lastWasSep = false;
      if (item.node) return menu.appendChild(item.node);
      if (item.head) return menu.appendChild(el('div', 'menu-head', item.head));
      if (item.empty) return menu.appendChild(el('div', 'menu-empty', item.empty));

      const row = el('button', 'mi');
      row.type = 'button';
      row.setAttribute('role', 'menuitem');
      if (item.file) {
        row.classList.add('file');
        row.title = item.file;
        row.append(el('span', 'rname', baseName(item.file)), pathLine(dirName(item.file), 'rpath'));
      } else {
        row.append(el('span', 'ml', item.label));
        if (item.kbd) row.append(el('span', 'mk', item.kbd));
        if (item.title) row.title = item.title;
      }
      if (item.disabled) row.disabled = true;
      row.addEventListener('click', () => {
        hideMenu();
        item.run();
      });
      menu.appendChild(row);
    });
  }

  function showMenu(x, y, items, anchor, alignRight) {
    hidePrefs();
    if (menuAnchor) menuAnchor.setAttribute('aria-expanded', 'false');
    fillMenu(items);
    menuAnchor = anchor || null;
    if (menuAnchor) menuAnchor.setAttribute('aria-expanded', 'true');
    place(menu, x, y, alignRight);
  }

  function toggleMenuFor(anchor, build, alignRight) {
    if (!menu.classList.contains('hidden') && menuAnchor === anchor) {
      hideMenu();
      return;
    }
    const box = anchor.getBoundingClientRect();
    showMenu(alignRight ? box.right : box.left, box.bottom + 6, build(), anchor, alignRight);
  }

  function fileInfoNode(meta) {
    const box = el('div', 'fileinfo');
    box.append(el('div', 'fname', meta.name || ''), el('div', 'fdir', meta.dir || dirName(meta.path || '')));
    const s = meta.stats || {};
    const stats = el('div', 'fstats');
    const stat = (number, unit, prefix) => {
      const span = el('span');
      if (prefix) span.append(prefix);
      span.append(el('b', '', number), unit);
      stats.appendChild(span);
    };
    stat(thousand(s.chars || 0), ' 字');
    stat(thousand(s.lines || 0), ' 行');
    stat(String(s.minutes || 1), ' 分钟', '约 ');
    if (meta.enc) stats.appendChild(el('span', '', meta.enc));
    box.appendChild(stats);
    return box;
  }

  function fileMenuItems() {
    const items = [];
    const meta = state.meta;
    if (meta) items.push({ node: fileInfoNode(meta) }, { sep: true });
    const others = state.recent.filter((path) => !samePath(path, meta && meta.path));
    items.push({ head: '最近打开' });
    if (others.length) {
      others.slice(0, 8).forEach((path) => items.push({ file: path, run: () => ipc({ cmd: 'openPath', value: path }) }));
    } else {
      items.push({ empty: '还没有打开过别的文件' });
    }
    items.push({ sep: true });
    items.push({ label: '打开其他文件…', kbd: 'Ctrl+O', run: () => ipc({ cmd: 'open' }) });
    return items;
  }

  function moreMenuItems() {
    const hasDoc = !!state.meta;
    const items = [{ label: '打开文件…', kbd: 'Ctrl+O', run: () => ipc({ cmd: 'open' }) }];
    if (hasDoc) {
      items.push(
        { label: '重新读取', kbd: 'Ctrl+R', run: () => ipc({ cmd: 'reload' }) },
        { sep: true },
        { label: '导出网页…', kbd: 'Ctrl+S', run: () => ipc({ cmd: 'export' }) },
        { label: '打印或存为 PDF', kbd: 'Ctrl+P', run: () => window.print() }
      );
    }
    items.push(
      { sep: true },
      { label: '设为 .md 默认打开程序', run: () => ipc({ cmd: 'associate' }) },
      { label: 'Windows 默认应用设置', run: () => ipc({ cmd: 'assocSettings' }) },
      { sep: true },
      { label: '快捷键', kbd: 'F1', run: openOverlay }
    );
    return items;
  }

  $('#t-file').addEventListener('click', (event) => {
    event.stopPropagation();
    toggleMenuFor($('#t-file'), fileMenuItems, false);
  });
  $('#t-more').addEventListener('click', (event) => {
    event.stopPropagation();
    toggleMenuFor($('#t-more'), moreMenuItems, true);
  });

  function togglePrefs() {
    if (!prefs.classList.contains('hidden')) {
      hidePrefs();
      return;
    }
    hideMenu();
    const button = $('#t-prefs');
    const box = button.getBoundingClientRect();
    button.setAttribute('aria-expanded', 'true');
    place(prefs, box.right, box.bottom + 6, true);
  }
  $('#t-prefs').addEventListener('click', (event) => {
    event.stopPropagation();
    togglePrefs();
  });

  $('#seg-face').addEventListener('click', (event) => {
    const button = event.target.closest('button[data-value]');
    if (button) setFace(button.dataset.value);
  });
  $('#seg-theme').addEventListener('click', (event) => {
    const button = event.target.closest('button[data-value]');
    if (button) setTheme(button.dataset.value);
  });

  document.addEventListener('mousedown', (event) => {
    const target = event.target;
    if (!menu.classList.contains('hidden') && !menu.contains(target) && !(menuAnchor && menuAnchor.contains(target))) hideMenu();
    if (!prefs.classList.contains('hidden') && !prefs.contains(target) && !$('#t-prefs').contains(target)) hidePrefs();
  });
  window.addEventListener('blur', () => {
    hideMenu();
    hidePrefs();
  });

  /** Arrow keys walk the open menu. */
  function menuKeys(event) {
    if (menu.classList.contains('hidden')) return false;
    const rows = [...menu.querySelectorAll('.mi:not([disabled])')];
    if (!rows.length) return false;
    const at = rows.indexOf(document.activeElement);
    let next = -1;
    if (event.key === 'ArrowDown') next = at < 0 ? 0 : (at + 1) % rows.length;
    else if (event.key === 'ArrowUp') next = at < 0 ? rows.length - 1 : (at - 1 + rows.length) % rows.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = rows.length - 1;
    if (next < 0) return false;
    event.preventDefault();
    rows[next].focus();
    return true;
  }

  document.addEventListener('contextmenu', (event) => {
    if (!event.target.closest('#doc')) return;
    event.preventDefault();
    const items = [];
    const selection = String(window.getSelection() || '').trim();
    if (selection) {
      items.push({ label: '复制', kbd: 'Ctrl+C', run: () => copyText(selection) });
      items.push({ label: '翻译选中的文字', kbd: 'Ctrl+T', run: () => requestTranslation({ text: selection, scope: '选中的内容', fromDocument: false }) });
      items.push({ sep: true });
    }
    const anchor = event.target.closest('a[href]');
    if (anchor) {
      const href = anchor.getAttribute('href');
      items.push({ label: '打开链接', title: href, run: () => ipc({ cmd: 'link', value: href }) });
      items.push({ label: '复制链接地址', run: () => copyText(href) });
      items.push({ sep: true });
    }
    items.push({ label: '全选', kbd: 'Ctrl+A', run: () => document.execCommand('selectAll') });
    if (!selection) items.push({ label: '翻译全文', kbd: 'Ctrl+T', run: requestDocumentTranslation });
    items.push({ label: '查找', kbd: 'Ctrl+F', run: openFind });
    items.push({ sep: true });
    items.push({ label: '导出网页…', kbd: 'Ctrl+S', run: () => ipc({ cmd: 'export' }) });
    showMenu(event.clientX, event.clientY, items, null, false);
  });

  /* ============================================================== 工具栏 */

  $('#w-open').addEventListener('click', () => ipc({ cmd: 'open' }));
  $('#t-translate').addEventListener('click', () => requestTranslation());
  $('#w-min').addEventListener('click', () => ipc({ cmd: 'win', value: 'minimize' }));
  $('#w-max').addEventListener('click', () => ipc({ cmd: 'win', value: 'maximize' }));
  $('#w-close').addEventListener('click', () => ipc({ cmd: 'win', value: 'close' }));
  $('#ov-close').addEventListener('click', closeOverlay);
  $('#overlay').addEventListener('click', (event) => {
    if (event.target === event.currentTarget) closeOverlay();
  });
  $('#recent-clear').addEventListener('click', () => ipc({ cmd: 'clearRecent' }));

  function toggleToc() {
    if (document.body.classList.contains('no-doc')) return;
    state.toc = !state.toc;
    state.tocBeforePanel = false;
    paintToc();
    ipc({ cmd: 'toc', value: state.toc });
  }
  $('#t-toc').addEventListener('click', toggleToc);
  $('#toc-collapse').addEventListener('click', () => {
    state.toc = false;
    state.tocBeforePanel = false;
    paintToc();
    ipc({ cmd: 'toc', value: false });
  });
  $('#t-find').addEventListener('click', openFind);

  function nudgeFont(delta, quiet) {
    const next = Math.min(30, Math.max(12, state.fontSize + delta));
    if (next === state.fontSize) return;
    state.fontSize = next;
    applyFont();
    ipc({ cmd: 'font', value: next });
    // 设置面板开着时数字就在眼前，不用再弹提示；滚轮连发也不提示
    if (!quiet && prefs.classList.contains('hidden')) toast('字号 ' + next, 'ok');
  }
  $('#t-plus').addEventListener('click', () => nudgeFont(1));
  $('#t-minus').addEventListener('click', () => nudgeFont(-1));

  window.addEventListener('wheel', (event) => {
    holdTopUntil = 0;
    if (!event.ctrlKey) return;
    event.preventDefault();
    nudgeFont(event.deltaY < 0 ? 1 : -1, true);
  }, { passive: false });

  /* ================================================================ 拖放 */

  function onDragState(kind) {
    $('#dropzone').classList.toggle('hidden', kind === 'leave' || kind === 'drop');
  }

  /* ================================================================ 链接 */

  $('#doc').addEventListener('click', (event) => {
    const anchor = event.target.closest('a[href]');
    if (!anchor) return;
    const href = anchor.getAttribute('href') || '';
    event.preventDefault();
    if (href.startsWith('#')) {
      const target = document.getElementById(decodeURIComponent(href.slice(1)));
      if (target) target.scrollIntoView({ block: 'start', behavior: 'smooth' });
      return;
    }
    ipc({ cmd: 'link', value: href });
  });

  /* ============================================================== 快捷键 */

  /* 帮助面板的开关都走这里：关闭时先播反向动画，动画跑完再真正隐藏 */
  let overlayTimer = 0;

  function openOverlay() {
    hideMenu();
    hidePrefs();
    window.clearTimeout(overlayTimer);
    $('#overlay').classList.remove('closing', 'hidden');
  }

  function closeOverlay() {
    const overlay = $('#overlay');
    if (overlay.classList.contains('hidden') || overlay.classList.contains('closing')) return;
    const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (reduce) {
      overlay.classList.add('hidden');
      return;
    }
    overlay.classList.add('closing');
    overlayTimer = window.setTimeout(() => {
      overlay.classList.add('hidden');
      overlay.classList.remove('closing');
    }, 190);
  }

  document.addEventListener('keydown', (event) => {
    const mod = event.ctrlKey || event.metaKey;
    const key = event.key;
    if (!mod && !event.shiftKey) holdTopUntil = 0;

    if (menuKeys(event)) return;

    if (key === 'Escape') {
      if (!$('#overlay').classList.contains('hidden')) return closeOverlay();
      if (!menu.classList.contains('hidden')) {
        const anchor = menuAnchor;
        hideMenu();
        if (anchor) anchor.focus();
        return;
      }
      if (!prefs.classList.contains('hidden')) {
        hidePrefs();
        return $('#t-prefs').focus();
      }
      if (!$('#finder').classList.contains('hidden')) return closeFind();
      if ($('#panel').classList.contains('on')) return closePanel();
      return;
    }
    if (key === 'F1') { event.preventDefault(); return openOverlay(); }
    if (key === 'F11') { event.preventDefault(); return ipc({ cmd: 'fullscreen' }); }
    if (key === 'F5') { event.preventDefault(); return ipc({ cmd: 'reload' }); }

    if (!mod) return;
    const lower = key.toLowerCase();
    if (lower === 'o') { event.preventDefault(); ipc({ cmd: 'open' }); }
    else if (lower === 'r') { event.preventDefault(); ipc({ cmd: 'reload' }); }
    else if (lower === 'f') { event.preventDefault(); openFind(); }
    else if (lower === 't') { event.preventDefault(); requestTranslation(); }
    else if (lower === 'b') { event.preventDefault(); toggleToc(); }
    else if (lower === 'd') { event.preventDefault(); setTheme(state.theme === 'dark' ? 'light' : 'dark'); }
    else if (lower === 's') { event.preventDefault(); ipc({ cmd: 'export' }); }
    else if (lower === 'p') { event.preventDefault(); window.print(); }
    else if (lower === '0') { event.preventDefault(); state.fontSize = 17; applyFont(); ipc({ cmd: 'font', value: 17 }); }
    else if (key === '=' || key === '+') { event.preventDefault(); nudgeFont(1); }
    else if (key === '-' || key === '_') { event.preventDefault(); nudgeFont(-1); }
  });

  /* ================================================================ 提示 */

  function toast(text, kind) {
    if (!text) return;
    const node = el('div', 'toast' + (kind === 'err' ? ' err' : kind === 'ok' ? ' ok' : ''));
    node.setAttribute('role', kind === 'err' ? 'alert' : 'status');
    node.appendChild(el('span', '', text));
    const host = $('#toasts');
    host.appendChild(node);
    while (host.children.length > 3) host.firstElementChild.remove();
    setTimeout(() => {
      node.classList.add('out');
      setTimeout(() => node.remove(), 320);
    }, kind === 'err' ? 4600 : 2200);
  }

  /* ================================================================ 启动 */

  window.addEventListener('error', (event) => {
    ipc({ cmd: 'jsError', value: String(event.message || '脚本异常') });
  });

  paintTheme();
  paintFace();
  applyFont();
  paintToc();
  ipc({ cmd: 'ready' });
})();
