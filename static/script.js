(() => {
  'use strict';

  const API = '/api';
  const $ = (id) => document.getElementById(id);

  const TEXT_PALETTE = [
    '#000000', '#434343', '#666666', '#999999', '#b7b7b7', '#cccccc', '#d9d9d9', '#efefef', '#f3f3f3', '#ffffff',
    '#980000', '#ff0000', '#ff9900', '#ffff00', '#00ff00', '#00ffff', '#4a86e8', '#0000ff', '#9900ff', '#ff00ff',
    '#e6b8af', '#f4cccc', '#fce5cd', '#fff2cc', '#d9ead3', '#d0e0e3', '#c9daf8', '#cfe2f3', '#d9d2e9', '#ead1dc',
    '#dd7e6b', '#ea9999', '#f9cb9c', '#ffe599', '#b6d7a8', '#a2c4c9', '#a4c2f4', '#9fc5e8', '#b4a7d6', '#d5a6bd',
    '#cc4125', '#e06666', '#f6b26b', '#ffd966', '#93c47d', '#76a5af', '#6d9eeb', '#6fa8dc', '#8e7cc3', '#c27ba0',
    '#a61c00', '#cc0000', '#e69138', '#f1c232', '#6aa84f', '#45818e', '#3c78d8', '#3d85c6', '#674ea7', '#a64d79',
  ];
  const HL_PALETTE = TEXT_PALETTE.filter((c) => c.toLowerCase() !== '#ffffff');
  const PAGE_PALETTE = [
    '#ffffff', '#f5f5f7', '#000000', '#1c1c1e', '#fff8e7', '#e8f0fe', '#e6f4ea', '#fce8e6', '#f3e8fd', '#eceff1',
  ].concat(TEXT_PALETTE.slice(10, 40));

  const docTitle = $('doc-title');
  const statusText = $('status-text');
  const starBtn = $('star-btn');
  const saveBtn = $('save-btn');
  const themeToggle = $('theme-toggle');
  const presentBtn = $('present-btn');
  const sidebarNew = $('sidebar-new');
  const sidebarOpen = $('sidebar-open');
  const presentationOpenInput = $('presentation-open-input');
  const fileOpenInput = $('file-open-input');
  const insertImageInput = $('insert-image-input');
  const insertVideoInput = $('insert-video-input');
  const insertAudioInput = $('insert-audio-input');
  const docList = $('doc-list');
  const docsFolderLabel = $('docs-folder-label');
  const refreshFiles = $('refresh-files');
  const thumbList = $('thumb-list');
  const slidePage = $('slide-page');
  const stageWrap = $('stage-wrap');
  const filmstrip = $('filmstrip');
  const workspace = $('workspace');
  const mediaView = $('media-view');
  const mediaTitle = $('media-title');
  const mediaKind = $('media-kind');
  const pdfPages = $('pdf-pages');
  const imageView = $('image-view');
  const mediaFrame = $('media-frame');
  const presentOverlay = $('present-overlay');
  const presentSlide = $('present-slide');
  const fontLabel = $('font-label');
  const fsVal = $('fs-val');
  const colBar = $('col-bar');
  const hlBar = $('hl-bar');
  const pageBar = $('page-bar');

  // Defaults must be declared before emptyTitleSlide() / textEl() use them.
  let currentFont = 'Inter';
  let currentPt = 18;
  let currentColor = '#000000';
  let currentHighlight = '#ffe566';
  let currentPageColor = '#ffffff';
  let activeSlide = 0;
  let docId = null;
  let dirty = false;
  let starred = false;
  let saveTimer = null;
  let activeFilePath = null;
  let selectedElId = null;
  let undoStack = [];
  let redoStack = [];
  let presentIndex = 0;
  let viewMode = 'presentation'; // presentation | media

  // PDF.js state
  let pdfDoc = null;
  let pdfPageNum = 1;
  let pdfScale = 1.15;
  let pdfRenderToken = 0;
  let pendingPdfForInsert = null;

  function uid() {
    return crypto.randomUUID ? crypto.randomUUID() : 'id-' + Math.random().toString(36).slice(2, 11);
  }

  function emptyTitleSlide() {
    return {
      id: uid(),
      background: '#ffffff',
      elements: [
        textEl('title', 8, 28, 84, 28, '', 44, 'center'),
        textEl('subtitle', 12, 58, 76, 14, '', 22, 'center'),
      ],
    };
  }

  function textEl(kind, x, y, w, h, text, fontSize, align) {
    return {
      id: uid(),
      kind,
      x, y, w, h,
      text: text || '',
      src: null,
      mime: null,
      fontSize: fontSize || 18,
      fontFamily: currentFont || 'Inter',
      align: align || 'left',
      color: null,
      bold: null,
      italic: null,
      underline: null,
      strikethrough: null,
      highlight: null,
      verticalAlign: null,
    };
  }

  let slides = [emptyTitleSlide()];

  function setStatus(msg, kind) {
    if (!statusText) return;
    statusText.textContent = msg;
    statusText.classList.toggle('saving', kind === 'saving');
    statusText.classList.toggle('error', kind === 'error');
  }

  function markDirty() {
    dirty = true;
    setStatus('Unsaved changes', 'saving');
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => saveDocument(), 900);
  }

  function currentSlide() {
    return slides[activeSlide] || slides[0];
  }

  function snapshot() {
    return JSON.stringify({ slides, activeSlide });
  }

  function pushUndo() {
    undoStack.push(snapshot());
    if (undoStack.length > 80) undoStack.shift();
    redoStack = [];
  }

  function restore(snap) {
    try {
      const s = JSON.parse(snap);
      slides = s.slides || [emptyTitleSlide()];
      activeSlide = s.activeSlide || 0;
      renderAll();
    } catch { /* ignore */ }
  }

  function undo() {
    if (!undoStack.length) return;
    redoStack.push(snapshot());
    restore(undoStack.pop());
    markDirty();
  }

  function redo() {
    if (!redoStack.length) return;
    undoStack.push(snapshot());
    restore(redoStack.pop());
    markDirty();
  }

  function formatBytes(n) {
    if (n < 1024) return n + ' B';
    if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
    return (n / (1024 * 1024)).toFixed(1) + ' MB';
  }

  function placeholderFor(kind) {
    if (kind === 'title') return 'Click to add title';
    if (kind === 'subtitle') return 'Click to add subtitle';
    return 'Click to add text';
  }

  function closeMenus() {
    document.querySelectorAll('.glass-menu').forEach((m) => m.classList.add('hidden'));
    document.querySelectorAll('[aria-expanded="true"]').forEach((b) => b.setAttribute('aria-expanded', 'false'));
  }

  function toggleMenu(btn, menu) {
    const open = menu.classList.contains('hidden');
    closeMenus();
    if (open) {
      menu.classList.remove('hidden');
      if (btn) btn.setAttribute('aria-expanded', 'true');
    }
  }

  /* ── Color pickers ── */
  function fillSwatches(container, colors, onPick) {
    container.innerHTML = '';
    const grid = document.createElement('div');
    grid.className = 'swatch-grid';
    colors.forEach((c) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'swatch';
      b.style.background = c;
      b.title = c;
      b.addEventListener('click', () => onPick(c));
      grid.appendChild(b);
    });
    container.appendChild(grid);
  }

  fillSwatches($('picker'), TEXT_PALETTE, (c) => {
    currentColor = c;
    colBar.style.background = c;
    applyElStyle({ color: c });
    closeMenus();
  });
  fillSwatches($('hl-colors'), HL_PALETTE, (c) => {
    currentHighlight = c;
    hlBar.style.background = c;
    applyElStyle({ highlight: c });
    closeMenus();
  });
  fillSwatches($('page-picker'), PAGE_PALETTE, (c) => {
    currentPageColor = c;
    pageBar.style.background = c;
    pushUndo();
    currentSlide().background = c;
    markDirty();
    renderSlide();
    closeMenus();
  });
  $('hl-none').addEventListener('click', () => {
    applyElStyle({ highlight: null });
    closeMenus();
  });

  /* ── Render ── */
  function renderThumbs() {
    thumbList.innerHTML = '';
    slides.forEach((slide, i) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'thumb-card' + (i === activeSlide ? ' active' : '');
      const titleEl = (slide.elements || []).find((e) => e.kind === 'title');
      const subEl = (slide.elements || []).find((e) => e.kind === 'subtitle');
      const t = (titleEl && titleEl.text) || 'Slide ' + (i + 1);
      const s = (subEl && subEl.text) || '';
      btn.innerHTML =
        '<div class="thumb-num">' + (i + 1) +
        '</div><div class="thumb-preview" style="background:' +
        escapeAttr(slide.background || '#fff') +
        '"><div class="mini-title"></div><div class="mini-sub"></div></div>';
      const miniT = btn.querySelector('.mini-title');
      const miniS = btn.querySelector('.mini-sub');
      if (miniT) miniT.textContent = t || 'Untitled';
      if (miniS) miniS.textContent = s;
      btn.addEventListener('click', () => {
        activeSlide = i;
        selectedElId = null;
        renderAll();
      });
      thumbList.appendChild(btn);
    });
  }

  function escapeAttr(s) {
    return String(s).replace(/"/g, '&quot;');
  }

  function renderSlide() {
    const slide = currentSlide();
    if (!slide) {
      slidePage.innerHTML = '';
      return;
    }
    slidePage.innerHTML = '';
    slidePage.style.background = slide.background || '#ffffff';
    currentPageColor = slide.background || '#ffffff';
    pageBar.style.background = currentPageColor;

    (slide.elements || []).forEach((el) => {
      const node = document.createElement('div');
      node.className = 'slide-el' + (el.id === selectedElId ? ' selected' : '');
      node.dataset.id = el.id;
      node.dataset.kind = el.kind;
      node.style.left = el.x + '%';
      node.style.top = el.y + '%';
      node.style.width = el.w + '%';
      node.style.height = el.h + '%';
      applyDomStyle(node, el);

      if (el.kind === 'image' && el.src) {
        const img = document.createElement('img');
        img.className = 'media-fill';
        img.src = el.src;
        img.alt = el.text || 'Image';
        node.appendChild(img);
        node.contentEditable = 'false';
      } else if (el.kind === 'video' && el.src) {
        const v = document.createElement('video');
        v.className = 'media-fill';
        v.src = el.src;
        v.controls = true;
        v.preload = 'metadata';
        node.appendChild(v);
        const lab = document.createElement('span');
        lab.className = 'media-label';
        lab.textContent = el.text || 'Video';
        node.appendChild(lab);
        node.contentEditable = 'false';
      } else if (el.kind === 'audio' && el.src) {
        const a = document.createElement('audio');
        a.className = 'media-audio';
        a.src = el.src;
        a.controls = true;
        a.preload = 'metadata';
        node.appendChild(a);
        const lab = document.createElement('span');
        lab.className = 'media-label';
        lab.textContent = el.text || 'Audio';
        node.appendChild(lab);
        node.contentEditable = 'false';
      } else {
        node.contentEditable = 'true';
        node.spellcheck = false;
        const text = el.text || '';
        if (!text) {
          const ph = document.createElement('span');
          ph.className = 'ph';
          ph.textContent = placeholderFor(el.kind);
          node.appendChild(ph);
        } else {
          node.textContent = text;
        }
        node.addEventListener('focus', () => {
          selectedElId = el.id;
          const ph = node.querySelector('.ph');
          if (ph) node.innerHTML = '';
          syncToolbarFromEl(el);
          document.querySelectorAll('.slide-el').forEach((n) => {
            n.classList.toggle('selected', n.dataset.id === el.id);
          });
        });
        node.addEventListener('blur', () => {
          const val = node.innerText.replace(/\u00a0/g, ' ').replace(/\n$/, '');
          if ((el.text || '') !== val) {
            pushUndo();
            el.text = val;
            markDirty();
          }
          if (!val.trim()) {
            node.innerHTML = '';
            const ph = document.createElement('span');
            ph.className = 'ph';
            ph.textContent = placeholderFor(el.kind);
            node.appendChild(ph);
          }
          renderThumbs();
        });
        node.addEventListener('input', () => {
          const ph = node.querySelector('.ph');
          if (ph) ph.remove();
        });
      }

      node.addEventListener('mousedown', (e) => {
        if (e.target.closest('video, audio, img')) {
          selectedElId = el.id;
        } else if (!e.target.closest('video, audio')) {
          selectedElId = el.id;
        }
        document.querySelectorAll('.slide-el').forEach((n) => {
          n.classList.toggle('selected', n.dataset.id === el.id);
        });
        syncToolbarFromEl(el);
      });

      slidePage.appendChild(node);
    });
  }

  function applyDomStyle(node, el) {
    if (el.align) node.style.textAlign = el.align;
    if (el.fontSize) node.style.fontSize = el.fontSize + 'px';
    if (el.fontFamily) node.style.fontFamily = el.fontFamily;
    if (el.color) node.style.color = el.color;
    if (el.bold) node.style.fontWeight = '700';
    if (el.italic) node.style.fontStyle = 'italic';
    const decos = [];
    if (el.underline) decos.push('underline');
    if (el.strikethrough) decos.push('line-through');
    node.style.textDecoration = decos.length ? decos.join(' ') : '';
    if (el.highlight) node.style.backgroundColor = el.highlight;
    if (el.verticalAlign === 'super') {
      node.style.verticalAlign = 'super';
      node.style.fontSize = ((el.fontSize || 18) * 0.75) + 'px';
    } else if (el.verticalAlign === 'sub') {
      node.style.verticalAlign = 'sub';
      node.style.fontSize = ((el.fontSize || 18) * 0.75) + 'px';
    }
  }

  function syncToolbarFromEl(el) {
    if (!el) return;
    if (el.fontFamily) {
      currentFont = el.fontFamily;
      fontLabel.textContent = el.fontFamily;
    }
    if (el.fontSize) {
      currentPt = el.fontSize;
      fsVal.value = String(Math.round(el.fontSize));
    }
    if (el.color) {
      currentColor = el.color;
      colBar.style.background = el.color;
    }
    if (el.highlight) {
      currentHighlight = el.highlight;
      hlBar.style.background = el.highlight;
    }
    // Liquid-glass selected bubbles on format / align chrome
    document.querySelectorAll('.t-icon.fmt').forEach((btn) => {
      const cmd = btn.dataset.cmd;
      let on = false;
      if (cmd === 'bold') on = !!el.bold;
      else if (cmd === 'italic') on = !!el.italic;
      else if (cmd === 'underline') on = !!el.underline;
      else if (cmd === 'strikethrough') on = !!el.strikethrough;
      else if (cmd === 'superscript') on = el.verticalAlign === 'super';
      else if (cmd === 'subscript') on = el.verticalAlign === 'sub';
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-pressed', String(on));
    });
    document.querySelectorAll('.t-icon.align').forEach((btn) => {
      const on = (el.align || 'left') === btn.dataset.align;
      btn.classList.toggle('active', on);
      btn.setAttribute('aria-pressed', String(on));
    });
  }

  function selectedEl() {
    if (!selectedElId) return null;
    return (currentSlide().elements || []).find((e) => e.id === selectedElId) || null;
  }

  function applyElStyle(patch) {
    const el = selectedEl();
    if (!el || ['image', 'video', 'audio'].includes(el.kind)) {
      // apply as defaults for next text
      if (patch.color) currentColor = patch.color;
      if (patch.fontFamily) currentFont = patch.fontFamily;
      if (patch.fontSize) currentPt = patch.fontSize;
      return;
    }
    pushUndo();
    Object.keys(patch).forEach((k) => {
      el[k] = patch[k];
    });
    markDirty();
    renderSlide();
    syncToolbarFromEl(el);
  }

  function setViewMode(mode) {
    viewMode = mode;
    const media = mode === 'media';
    workspace.classList.toggle('workspace-media-mode', media);
    mediaView.classList.toggle('hidden', !media);
    stageWrap.classList.toggle('hidden', media);
    filmstrip.classList.toggle('hidden', media);
    if (!media) {
      renderThumbs();
      renderSlide();
    }
  }

  function renderAll() {
    if (viewMode === 'media') return;
    renderThumbs();
    renderSlide();
  }

  /* ── PDF / image viewer ── */
  function clearMediaViewer() {
    pdfRenderToken += 1;
    pdfDoc = null;
    pdfPageNum = 1;
    pdfPages.innerHTML = '';
    imageView.classList.add('hidden');
    imageView.removeAttribute('src');
    mediaFrame.classList.add('hidden');
    mediaFrame.src = 'about:blank';
  }

  async function openMediaFile(opened) {
    clearMediaViewer();
    mediaTitle.textContent = opened.title || opened.name || 'File';
    mediaKind.textContent = (opened.ext || opened.format || '').toUpperCase();
    setViewMode('media');

    if (opened.slides && opened.slides.length) {
      slides = opened.slides;
      activeSlide = opened.active_slide || 0;
    }

    const src = resolveBinarySrc(opened);
    if (!src) {
      setStatus('Could not load file bytes', 'error');
      return;
    }

    const ext = (opened.ext || '').toLowerCase();
    const mime = (opened.mime || '').toLowerCase();

    if (ext === 'pdf' || mime === 'application/pdf') {
      await renderPdf(src);
      setStatus('Opened PDF');
      return;
    }
    if (ext === 'png' || mime.startsWith('image/') || ['jpg', 'jpeg', 'webp', 'gif', 'svg'].includes(ext)) {
      imageView.classList.remove('hidden');
      imageView.src = src.url || src;
      setStatus('Opened image');
      return;
    }
    if (ext === 'pptx') {
      // Prefer editable slides extracted server-side; also keep raw available.
      setViewMode('presentation');
      setStatus('Opened PowerPoint · ' + slides.length + ' slide(s)');
      renderAll();
      return;
    }
    mediaFrame.classList.remove('hidden');
    mediaFrame.src = src.url || src;
  }

  function resolveBinarySrc(opened) {
    if (opened.view_url) return { kind: 'url', url: opened.view_url };
    if (opened.binary_base64) {
      const mime = opened.mime || 'application/octet-stream';
      return { kind: 'data', url: 'data:' + mime + ';base64,' + opened.binary_base64 };
    }
    return null;
  }

  async function renderPdf(src) {
    pdfPages.innerHTML = '<p class="muted" style="padding:24px">Loading PDF…</p>';
    if (!window.pdfjsLib) {
      pdfPages.innerHTML = '<p class="muted">PDF.js not available</p>';
      mediaFrame.classList.remove('hidden');
      mediaFrame.src = src.url;
      return;
    }
    window.pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
      'vendor/pdf.worker.min.js',
      window.location.href
    ).href;
    const token = ++pdfRenderToken;
    try {
      const loadingTask = window.pdfjsLib.getDocument(
        src.kind === 'url' ? src.url : { data: dataUrlToUint8(src.url) }
      );
      pdfDoc = await loadingTask.promise;
      if (token !== pdfRenderToken) return;
      pdfPageNum = 1;
      await paintAllPdfPages(token);
      updatePdfLabel();
    } catch (e) {
      console.warn(e);
      pdfPages.innerHTML = '<p class="muted">Could not render PDF. Falling back…</p>';
      mediaFrame.classList.remove('hidden');
      mediaFrame.src = src.url;
    }
  }

  function dataUrlToUint8(dataUrl) {
    const b64 = dataUrl.split(',')[1] || '';
    const bin = atob(b64);
    const arr = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
    return arr;
  }

  async function paintAllPdfPages(token) {
    const total = pdfDoc.numPages || 0;
    pdfPages.innerHTML = '';
    for (let i = 1; i <= total; i++) {
      if (token !== pdfRenderToken) return;
      const page = await pdfDoc.getPage(i);
      if (token !== pdfRenderToken) return;
      const viewport = page.getViewport({ scale: pdfScale });
      const wrap = document.createElement('div');
      wrap.className = 'pdf-page';
      wrap.id = 'pdf-page-' + i;
      wrap.dataset.page = String(i);
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      wrap.appendChild(canvas);
      pdfPages.appendChild(wrap);
      await page.render({ canvasContext: ctx, viewport }).promise;
    }
  }

  function updatePdfLabel() {
    const label = $('pdf-page-label');
    if (!label) return;
    if (!pdfDoc) {
      label.textContent = '—';
      return;
    }
    label.textContent = 'Page ' + pdfPageNum + ' / ' + pdfDoc.numPages;
  }

  function scrollToPdfPage(n) {
    const el = document.getElementById('pdf-page-' + n);
    if (el) el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    pdfPageNum = n;
    updatePdfLabel();
  }

  /* ── File list ── */
  async function loadFileList() {
    try {
      const [filesRes, dirRes] = await Promise.all([
        fetch(`${API}/files`),
        fetch(`${API}/files/docs-dir`),
      ]);
      if (!filesRes.ok) throw new Error('Failed to list files');
      const files = await filesRes.json();
      if (dirRes.ok) {
        const info = await dirRes.json();
        if (info.path) {
          const base = String(info.path).split(/[/\\]/).filter(Boolean).pop();
          docsFolderLabel.textContent = base || 'Documents';
          docsFolderLabel.title = info.path;
        }
      }
      renderFileList(files || []);
    } catch (e) {
      docList.innerHTML =
        '<div class="doc-empty">Could not list Documents folder.<br/>' +
        (e.message || e) +
        '</div>';
    }
  }

  function renderFileList(files) {
    const list = (files || []).filter((f) =>
      ['pdf', 'png', 'pptx', 'json', 'cog', 'jpg', 'jpeg', 'webp', 'gif', 'svg'].includes((f.ext || '').toLowerCase())
    );
    if (!list.length) {
      docList.innerHTML =
        '<div class="doc-empty">No supported files found in your Documents folder.</div>';
      return;
    }
    docList.innerHTML = '';
    list.forEach((f) => {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.className = 'doc-item' + (activeFilePath === f.path ? ' active' : '');
      const icon =
        f.ext === 'pdf' ? 'picture_as_pdf' : ['pptx', 'json', 'cog'].includes(f.ext) ? 'slideshow' : 'image';
      btn.innerHTML =
        '<span class="ext-badge">' +
        escapeHtml((f.ext || '').toUpperCase()) +
        '</span><span class="doc-meta"><span class="doc-name"></span><span class="doc-sub"></span></span>';
      btn.querySelector('.doc-name').textContent = f.name;
      btn.querySelector('.doc-sub').textContent = formatBytes(f.size || 0);
      btn.title = f.path;
      btn.addEventListener('click', () => openLibraryFile(f.path));
      docList.appendChild(btn);
    });
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');
  }

  async function openLibraryFile(relPath) {
    if (saveTimer) clearTimeout(saveTimer);
    if (dirty) {
      await saveDocument();
      if (dirty) return;
    }
    setStatus('Opening…', 'saving');
    try {
      const res = await fetch(`${API}/files/open`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path: relPath }),
      });
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || 'Open failed');
      }
      const opened = await res.json();
      activeFilePath = opened.path || relPath;
      docTitle.textContent = opened.title || opened.name || 'File';
      document.title = docTitle.textContent + ' — XSlide';
      docId = null;
      if (!opened.binary) {
        applyPresentationDoc(opened);
        activeFilePath = opened.path || relPath;
      } else {
        await openMediaFile(opened);
      }
      loadFileList();
    } catch (e) {
      setStatus(String(e.message || e), 'error');
    }
  }

  async function importFiles(fileList) {
    if (saveTimer) clearTimeout(saveTimer);
    if (dirty) {
      await saveDocument();
      if (dirty) return;
    }
    for (const file of fileList) {
      const ext = (file.name.split('.').pop() || '').toLowerCase();
      if (['json', 'cog'].includes(ext)) {
        try {
          const text = await file.text();
          const doc = JSON.parse(text);
          if (doc.slides) {
            applyPresentationDoc(doc);
            docId = null;
            activeFilePath = null;
            await saveDocument();
            continue;
          }
        } catch (e) {
          setStatus('Invalid file: ' + e.message, 'error');
          continue;
        }
      }
      const fd = new FormData();
      fd.append('file', file, file.name);
      setStatus('Importing ' + file.name + '…', 'saving');
      try {
        const res = await fetch(`${API}/files/import`, { method: 'POST', body: fd });
        if (!res.ok) {
          const err = await res.json().catch(() => ({}));
          throw new Error(err.error || 'Import failed');
        }
        const opened = await res.json();
        activeFilePath = opened.path || file.name;
        docTitle.textContent = opened.title || file.name;
        document.title = docTitle.textContent + ' — XSlide';
        docId = null;
        await openMediaFile(opened);
        setStatus('Imported ' + file.name);
        loadFileList();
      } catch (e) {
        setStatus(String(e.message || e), 'error');
      }
    }
  }

  function applyPresentationDoc(doc) {
    pushUndo();
    slides = Array.isArray(doc.slides) && doc.slides.length ? doc.slides : [emptyTitleSlide()];
    activeSlide = doc.active_slide || 0;
    docId = doc.id || null;
    starred = !!doc.starred;
    starBtn.setAttribute('aria-pressed', String(starred));
    docTitle.textContent = doc.title || 'Untitled presentation';
    document.title = docTitle.textContent + ' — XSlide';
    activeFilePath = doc.source_path || null;
    dirty = false;
    clearMediaViewer();
    setViewMode('presentation');
    setStatus('Opened presentation');
    renderAll();
  }

  async function newPresentation() {
    if (saveTimer) clearTimeout(saveTimer);
    if (dirty) {
      await saveDocument();
      if (dirty) return;
    }
    pushUndo();
    slides = [emptyTitleSlide()];
    activeSlide = 0;
    docId = null;
    activeFilePath = null;
    starred = false;
    starBtn.setAttribute('aria-pressed', 'false');
    docTitle.textContent = 'Untitled presentation';
    document.title = 'Untitled presentation — XSlide';
    dirty = false;
    selectedElId = null;
    clearMediaViewer();
    setViewMode('presentation');
    setStatus('New presentation');
    try {
      const res = await fetch(`${API}/documents`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: 'Untitled presentation', slides }),
      });
      if (res.ok) {
        const doc = await res.json();
        docId = doc.id;
        setStatus('Saved locally');
      }
    } catch { /* draft ok */ }
    renderAll();
  }

  async function saveDocument() {
    const title = (docTitle.textContent || '').trim() || 'Untitled presentation';
    const body = {
      title,
      slides,
      active_slide: activeSlide,
      starred,
      source_path: activeFilePath,
      source_format: activeFilePath
        ? (activeFilePath.split('.').pop() || '').toLowerCase()
        : null,
    };
    setStatus('Saving…', 'saving');
    try {
      let res;
      if (docId) {
        res = await fetch(`${API}/documents/${docId}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
      } else {
        res = await fetch(`${API}/documents`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        });
      }
      if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || 'Save failed');
      }
      const doc = await res.json();
      docId = doc.id;
      dirty = false;
      setStatus('Saved locally');
    } catch (e) {
      setStatus(String(e.message || e), 'error');
    }
  }

  async function exportPresentation() {
    if (viewMode === 'media') {
      setStatus('Open an editable presentation to export', 'error');
      return;
    }
    const format = $('export-format').value;
    const title = (docTitle.textContent || '').trim() || 'presentation';
    setStatus('Exporting…', 'saving');
    try {
      const exportSlides = format === 'pptx' ? JSON.parse(JSON.stringify(slides)) : slides;
      if (format === 'pptx') {
        for (const slide of exportSlides) {
          for (const element of slide.elements || []) {
            if (element.kind !== 'image' || !element.src) continue;
            if (/^data:image\/(png|jpeg|jpg|gif);base64,/i.test(element.src)) continue;
            element.src = await new Promise((resolve, reject) => {
              const image = new Image();
              image.onload = () => {
                const canvas = document.createElement('canvas');
                canvas.width = image.naturalWidth;
                canvas.height = image.naturalHeight;
                const context = canvas.getContext('2d');
                if (!context) return reject(new Error('Could not convert image for export'));
                context.drawImage(image, 0, 0);
                try { resolve(canvas.toDataURL('image/png')); }
                catch { reject(new Error('Could not read image for export')); }
              };
              image.onerror = () => reject(new Error('Could not load image for export'));
              image.src = element.src;
            });
          }
        }
      }
      const res = await fetch(`${API}/export`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ format, title, slides: exportSlides, active_slide: activeSlide }),
      });
      if (!res.ok) {
        const error = await res.json().catch(() => ({}));
        throw new Error(error.error || 'Export failed');
      }
      const blob = await res.blob();
      const cd = res.headers.get('Content-Disposition') || '';
      const star = cd.match(/filename\*=UTF-8''([^;]+)/i);
      const match = cd.match(/filename="([^"]+)"/);
      const filename = star ? decodeURIComponent(star[1]) : match ? match[1] : `${title}.${format}`;
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 60000);
      setStatus('Exported ' + filename);
    } catch (e) {
      setStatus(String(e.message || e), 'error');
    }
  }

  /* ── Insert media ── */
  function addTextBox() {
    if (viewMode === 'media') setViewMode('presentation');
    pushUndo();
    const el = textEl('text', 20, 40, 60, 16, '', currentPt, 'left');
    el.fontFamily = currentFont;
    el.color = currentColor;
    currentSlide().elements.push(el);
    selectedElId = el.id;
    markDirty();
    renderSlide();
  }

  function insertMediaElement(kind, src, mime, name) {
    if (viewMode === 'media') setViewMode('presentation');
    pushUndo();
    const el = {
      id: uid(),
      kind,
      x: 15,
      y: 20,
      w: kind === 'audio' ? 70 : 55,
      h: kind === 'audio' ? 20 : 50,
      text: name || kind,
      src,
      mime: mime || null,
      fontSize: 14,
      fontFamily: currentFont,
      align: 'left',
      color: null,
      bold: null,
      italic: null,
      underline: null,
      strikethrough: null,
      highlight: null,
      verticalAlign: null,
    };
    currentSlide().elements.push(el);
    selectedElId = el.id;
    markDirty();
    renderSlide();
    setStatus('Inserted ' + kind);
  }

  function readFileAsDataURL(file) {
    return new Promise((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(r.result);
      r.onerror = () => reject(new Error('read failed'));
      r.readAsDataURL(file);
    });
  }

  async function handleImageInsert(files) {
    for (const file of files) {
      const ext = (file.name.split('.').pop() || '').toLowerCase();
      if (ext === 'pdf' || file.type === 'application/pdf') {
        const dataUrl = await readFileAsDataURL(file);
        pendingPdfForInsert = dataUrl;
        const dlg = $('pdf-page-dialog');
        dlg.classList.remove('hidden');
        dlg.setAttribute('aria-hidden', 'false');
        $('pdf-page-num').value = '1';
        // preload page count
        try {
          if (window.pdfjsLib) {
            window.pdfjsLib.GlobalWorkerOptions.workerSrc = new URL(
              'vendor/pdf.worker.min.js',
              window.location.href
            ).href;
            const doc = await window.pdfjsLib.getDocument({ data: dataUrlToUint8(dataUrl) }).promise;
            $('pdf-page-num').max = String(doc.numPages);
            pendingPdfForInsert = { dataUrl, doc };
          }
        } catch (e) {
          console.warn(e);
        }
        return;
      }
      const dataUrl = await readFileAsDataURL(file);
      insertMediaElement('image', dataUrl, file.type || 'image/png', file.name);
    }
  }

  async function insertPdfPageAsImage() {
    const dlg = $('pdf-page-dialog');
    dlg.classList.add('hidden');
    const n = Math.max(1, parseInt($('pdf-page-num').value, 10) || 1);
    try {
      let doc = pendingPdfForInsert && pendingPdfForInsert.doc;
      let dataUrl = pendingPdfForInsert && pendingPdfForInsert.dataUrl;
      if (!doc && typeof pendingPdfForInsert === 'string') {
        dataUrl = pendingPdfForInsert;
      }
      if (!doc && dataUrl && window.pdfjsLib) {
        doc = await window.pdfjsLib.getDocument({ data: dataUrlToUint8(dataUrl) }).promise;
      }
      if (!doc) throw new Error('No PDF loaded');
      const page = await doc.getPage(Math.min(n, doc.numPages));
      const viewport = page.getViewport({ scale: 1.5 });
      const canvas = document.createElement('canvas');
      canvas.width = viewport.width;
      canvas.height = viewport.height;
      await page.render({ canvasContext: canvas.getContext('2d'), viewport }).promise;
      const img = canvas.toDataURL('image/png');
      insertMediaElement('image', img, 'image/png', 'PDF page ' + n);
    } catch (e) {
      setStatus('PDF page insert failed: ' + e.message, 'error');
    }
    pendingPdfForInsert = null;
  }

  async function handleVideoInsert(files) {
    for (const file of files) {
      const dataUrl = await readFileAsDataURL(file);
      insertMediaElement('video', dataUrl, file.type || 'video/mp4', file.name);
    }
  }

  async function handleAudioInsert(files) {
    for (const file of files) {
      const dataUrl = await readFileAsDataURL(file);
      insertMediaElement('audio', dataUrl, file.type || 'audio/mpeg', file.name);
    }
  }

  function addSlide() {
    if (viewMode === 'media') setViewMode('presentation');
    pushUndo();
    slides.splice(activeSlide + 1, 0, emptyTitleSlide());
    activeSlide += 1;
    selectedElId = null;
    markDirty();
    renderAll();
  }

  function deleteSlide() {
    if (slides.length <= 1) {
      setStatus('Need at least one slide', 'error');
      return;
    }
    pushUndo();
    slides.splice(activeSlide, 1);
    activeSlide = Math.min(activeSlide, slides.length - 1);
    selectedElId = null;
    markDirty();
    renderAll();
  }

  /* Present */
  function enterPresent() {
    if (viewMode === 'media') setViewMode('presentation');
    presentIndex = activeSlide;
    presentOverlay.classList.remove('hidden');
    presentOverlay.setAttribute('aria-hidden', 'false');
    renderPresentSlide();
  }

  function exitPresent() {
    presentOverlay.classList.add('hidden');
    presentOverlay.setAttribute('aria-hidden', 'true');
  }

  function renderPresentSlide() {
    const slide = slides[presentIndex];
    if (!slide) return;
    presentSlide.innerHTML = '';
    presentSlide.style.background = slide.background || '#fff';
    (slide.elements || []).forEach((el) => {
      const node = document.createElement('div');
      node.className = 'slide-el';
      node.dataset.kind = el.kind;
      node.style.left = el.x + '%';
      node.style.top = el.y + '%';
      node.style.width = el.w + '%';
      node.style.height = el.h + '%';
      node.style.border = 'none';
      applyDomStyle(node, el);
      if (el.kind === 'image' && el.src) {
        const img = document.createElement('img');
        img.className = 'media-fill';
        img.src = el.src;
        node.appendChild(img);
      } else if (el.kind === 'video' && el.src) {
        const v = document.createElement('video');
        v.className = 'media-fill';
        v.src = el.src;
        v.controls = true;
        node.appendChild(v);
      } else if (el.kind === 'audio' && el.src) {
        const a = document.createElement('audio');
        a.className = 'media-audio';
        a.src = el.src;
        a.controls = true;
        node.appendChild(a);
      } else {
        node.textContent = el.text || '';
      }
      presentSlide.appendChild(node);
    });
  }

  function toggleTheme() {
    const cur = document.documentElement.getAttribute('data-theme') || 'light';
    const next = cur === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    try {
      localStorage.setItem('xslide-theme', next);
    } catch { /* ignore */ }
    const meta = document.getElementById('meta-theme-color');
    if (meta) meta.content = next === 'dark' ? '#000000' : '#ffffff';
  }

  /* Events */
  sidebarNew.addEventListener('click', () => newPresentation());
  sidebarOpen.addEventListener('click', () => presentationOpenInput.click());
  presentationOpenInput.addEventListener('change', () => {
    if (presentationOpenInput.files?.length) {
      importFiles(Array.from(presentationOpenInput.files));
      presentationOpenInput.value = '';
    }
  });
  fileOpenInput.addEventListener('change', () => {
    if (fileOpenInput.files?.length) {
      importFiles(Array.from(fileOpenInput.files));
      fileOpenInput.value = '';
    }
  });
  refreshFiles.addEventListener('click', () => loadFileList());
  saveBtn.addEventListener('click', () => saveDocument());
  $('export-btn').addEventListener('click', exportPresentation);
  themeToggle.addEventListener('click', () => toggleTheme());
  presentBtn.addEventListener('click', () => enterPresent());
  starBtn.addEventListener('click', () => {
    starred = !starred;
    starBtn.setAttribute('aria-pressed', String(starred));
    markDirty();
  });
  docTitle.addEventListener('input', () => {
    document.title = (docTitle.textContent || 'Untitled') + ' — XSlide';
    markDirty();
  });

  $('add-slide-btn').addEventListener('click', () => addSlide());
  $('del-slide-btn').addEventListener('click', () => deleteSlide());
  $('undo-btn').addEventListener('click', () => undo());
  $('redo-btn').addEventListener('click', () => redo());
  $('media-to-slides').addEventListener('click', () => {
    clearMediaViewer();
    setViewMode('presentation');
    setStatus('Editing slides');
  });

  // Insert menu
  const insertBtn = $('insert-btn');
  const insertMenu = $('insert-menu');
  insertBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleMenu(insertBtn, insertMenu);
  });
  insertMenu.querySelectorAll('[data-insert]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const kind = btn.dataset.insert;
      closeMenus();
      if (kind === 'image') insertImageInput.click();
      else if (kind === 'video') insertVideoInput.click();
      else if (kind === 'audio') insertAudioInput.click();
      else if (kind === 'text') addTextBox();
    });
  });
  insertImageInput.addEventListener('change', () => {
    if (insertImageInput.files?.length) {
      handleImageInsert(Array.from(insertImageInput.files));
      insertImageInput.value = '';
    }
  });
  insertVideoInput.addEventListener('change', () => {
    if (insertVideoInput.files?.length) {
      handleVideoInsert(Array.from(insertVideoInput.files));
      insertVideoInput.value = '';
    }
  });
  insertAudioInput.addEventListener('change', () => {
    if (insertAudioInput.files?.length) {
      handleAudioInsert(Array.from(insertAudioInput.files));
      insertAudioInput.value = '';
    }
  });
  $('pdf-page-cancel').addEventListener('click', () => {
    $('pdf-page-dialog').classList.add('hidden');
    pendingPdfForInsert = null;
  });
  $('pdf-page-ok').addEventListener('click', () => insertPdfPageAsImage());

  // Font menu
  const fontBtn = $('font-btn');
  const fontMenu = $('font-menu');
  fontBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleMenu(fontBtn, fontMenu);
  });
  fontMenu.querySelectorAll('[data-font]').forEach((btn) => {
    btn.addEventListener('click', () => {
      currentFont = btn.dataset.font;
      fontLabel.textContent = currentFont;
      applyElStyle({ fontFamily: currentFont });
      closeMenus();
    });
  });

  $('fs-minus').addEventListener('click', () => {
    currentPt = Math.max(8, currentPt - 2);
    fsVal.value = String(currentPt);
    applyElStyle({ fontSize: currentPt });
  });
  $('fs-plus').addEventListener('click', () => {
    currentPt = Math.min(96, currentPt + 2);
    fsVal.value = String(currentPt);
    applyElStyle({ fontSize: currentPt });
  });
  fsVal.addEventListener('change', () => {
    const n = parseInt(fsVal.value, 10);
    if (!Number.isFinite(n)) return;
    currentPt = Math.min(96, Math.max(8, n));
    fsVal.value = String(currentPt);
    applyElStyle({ fontSize: currentPt });
  });

  // Format commands
  document.querySelectorAll('.t-icon.fmt').forEach((btn) => {
    btn.addEventListener('click', () => {
      const cmd = btn.dataset.cmd;
      const el = selectedEl();
      if (!el || ['image', 'video', 'audio'].includes(el.kind)) {
        // contentEditable selection fallback
        if (cmd === 'bold') document.execCommand('bold');
        else if (cmd === 'italic') document.execCommand('italic');
        else if (cmd === 'underline') document.execCommand('underline');
        else if (cmd === 'strikethrough') document.execCommand('strikeThrough');
        else if (cmd === 'superscript') document.execCommand('superscript');
        else if (cmd === 'subscript') document.execCommand('subscript');
        return;
      }
      if (cmd === 'bold') applyElStyle({ bold: !el.bold });
      else if (cmd === 'italic') applyElStyle({ italic: !el.italic });
      else if (cmd === 'underline') applyElStyle({ underline: !el.underline });
      else if (cmd === 'strikethrough') applyElStyle({ strikethrough: !el.strikethrough });
      else if (cmd === 'superscript') {
        applyElStyle({ verticalAlign: el.verticalAlign === 'super' ? null : 'super' });
      } else if (cmd === 'subscript') {
        applyElStyle({ verticalAlign: el.verticalAlign === 'sub' ? null : 'sub' });
      }
    });
  });

  document.querySelectorAll('.t-icon.align').forEach((btn) => {
    btn.addEventListener('click', () => applyElStyle({ align: btn.dataset.align }));
  });

  $('color-btn').addEventListener('click', (e) => {
    e.stopPropagation();
    toggleMenu($('color-btn'), $('picker'));
  });
  $('highlight-btn').addEventListener('click', (e) => {
    e.stopPropagation();
    toggleMenu($('highlight-btn'), $('hl-picker'));
  });
  $('page-color-btn').addEventListener('click', (e) => {
    e.stopPropagation();
    toggleMenu($('page-color-btn'), $('page-picker'));
  });

  // PDF nav
  $('pdf-prev').addEventListener('click', () => {
    if (!pdfDoc) return;
    scrollToPdfPage(Math.max(1, pdfPageNum - 1));
  });
  $('pdf-next').addEventListener('click', () => {
    if (!pdfDoc) return;
    scrollToPdfPage(Math.min(pdfDoc.numPages, pdfPageNum + 1));
  });
  $('pdf-zoom-in').addEventListener('click', async () => {
    if (!pdfDoc) return;
    pdfScale = Math.min(3, Math.round((pdfScale + 0.2) * 10) / 10);
    await paintAllPdfPages(++pdfRenderToken);
  });
  $('pdf-zoom-out').addEventListener('click', async () => {
    if (!pdfDoc) return;
    pdfScale = Math.max(0.5, Math.round((pdfScale - 0.2) * 10) / 10);
    await paintAllPdfPages(++pdfRenderToken);
  });

  slidePage.addEventListener('mousedown', (e) => {
    if (e.target === slidePage) {
      selectedElId = null;
      document.querySelectorAll('.slide-el').forEach((n) => n.classList.remove('selected'));
    }
  });

  document.addEventListener('click', (e) => {
    if (!e.target.closest('.tb-wrap') && !e.target.closest('.glass-menu')) closeMenus();
  });

  window.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 's') {
      e.preventDefault();
      saveDocument();
    } else if (mod && e.key.toLowerCase() === 'z' && !e.shiftKey) {
      e.preventDefault();
      undo();
    } else if (mod && (e.key.toLowerCase() === 'y' || (e.shiftKey && e.key.toLowerCase() === 'z'))) {
      e.preventDefault();
      redo();
    } else if (mod && e.key.toLowerCase() === 'b') {
      const el = selectedEl();
      if (el) applyElStyle({ bold: !el.bold });
    } else if (mod && e.key.toLowerCase() === 'i') {
      const el = selectedEl();
      if (el) applyElStyle({ italic: !el.italic });
    } else if (mod && e.key.toLowerCase() === 'u') {
      const el = selectedEl();
      if (el) applyElStyle({ underline: !el.underline });
    } else if (e.key === 'F5') {
      e.preventDefault();
      enterPresent();
    } else if (e.key === 'Escape') {
      if (!presentOverlay.classList.contains('hidden')) exitPresent();
      else closeMenus();
    } else if (!presentOverlay.classList.contains('hidden')) {
      if (e.key === 'ArrowRight' || e.key === ' ' || e.key === 'PageDown') {
        e.preventDefault();
        presentIndex = Math.min(slides.length - 1, presentIndex + 1);
        renderPresentSlide();
      } else if (e.key === 'ArrowLeft' || e.key === 'PageUp') {
        e.preventDefault();
        presentIndex = Math.max(0, presentIndex - 1);
        renderPresentSlide();
      }
    }
  });

  window.addEventListener('dragover', (e) => e.preventDefault());
  window.addEventListener('drop', (e) => {
    e.preventDefault();
    if (e.dataTransfer?.files?.length) importFiles(Array.from(e.dataTransfer.files));
  });

  if (window.XSuiteLiquidGlass?.attach) {
    window.XSuiteLiquidGlass.attach({ scrollEl: stageWrap });
  }

  colBar.style.background = currentColor;
  hlBar.style.background = currentHighlight;
  pageBar.style.background = currentPageColor;

  async function boot() {
    setViewMode('presentation');
    renderAll();
    await loadFileList();
    const launchToken = new URLSearchParams(window.location.search).get('launch');
    if (launchToken) {
      try {
        const res = await fetch(`${API}/files/launch?token=` + encodeURIComponent(launchToken));
        const opened = await res.json();
        if (!res.ok) throw new Error(opened.error || 'Could not open launch file');
        docId = null;
        activeFilePath = null;
        if (opened.binary) await openMediaFile(opened);
        else applyPresentationDoc(opened);
        setStatus('Opened ' + opened.name);
        return;
      } catch (e) {
        setStatus(String(e.message || e), 'error');
      }
    }
    try {
      const res = await fetch(`${API}/documents`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: 'Untitled presentation', slides }),
      });
      if (res.ok) {
        const doc = await res.json();
        docId = doc.id;
        setStatus('Saved locally');
      }
    } catch {
      setStatus('Local draft');
    }
  }

  boot();
})();
