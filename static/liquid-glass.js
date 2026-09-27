/**
 * XSlide — monochrome liquid glass material math + DOM driver.
 * Pure functions are testable without a browser DOM.
 * Grayscale only: specular + refraction, no chromatic edge.
 */
(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) {
    module.exports = api;
  }
  root.XSuiteLiquidGlass = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  'use strict';

  /**
   * Map pointer position relative to a surface rect into specular % and refraction px.
   * @param {number} clientX
   * @param {number} clientY
   * @param {{left:number,top:number,width:number,height:number}} rect
   * @param {{maxRefract?: number}} [opts]
   * @returns {{ x: number, y: number, rx: number, ry: number, nx: number, ny: number }}
   */
  function specularFromPointer(clientX, clientY, rect, opts) {
    const maxRefract = (opts && opts.maxRefract) != null ? opts.maxRefract : 10;
    const w = Math.max(rect.width, 1);
    const h = Math.max(rect.height, 1);
    const nx = clamp((clientX - rect.left) / w, 0, 1);
    const ny = clamp((clientY - rect.top) / h, 0, 1);
    // Specular sits slightly above the pointer for a top-lit glass read.
    const x = round2(nx * 100);
    const y = round2(clamp(ny * 100 * 0.78 + 8, 0, 100));
    const rx = round2((nx - 0.5) * 2 * maxRefract);
    const ry = round2((ny - 0.5) * 2 * maxRefract * 0.7);
    return { x, y, rx, ry, nx, ny };
  }

  /**
   * Scroll-driven specular bias (document peeks under chrome).
   * @param {number} scrollTop
   * @param {number} maxScroll
   * @returns {{ x: number, y: number, rx: number, ry: number }}
   */
  function specularFromScroll(scrollTop, maxScroll) {
    const t = maxScroll > 0 ? clamp(scrollTop / maxScroll, 0, 1) : 0;
    return {
      x: round2(46 + t * 12),
      y: round2(12 + t * 28),
      rx: round2(t * 3.5),
      ry: round2(t * 5.5),
    };
  }

  /**
   * Blend pointer + scroll contributions for a surface.
   */
  function blendSpecular(pointer, scroll, pointerWeight) {
    const pw = pointerWeight == null ? 0.82 : pointerWeight;
    const sw = 1 - pw;
    if (!pointer && !scroll) {
      return { x: 50, y: 18, rx: 0, ry: 0 };
    }
    if (!pointer) return { ...scroll };
    if (!scroll) return { x: pointer.x, y: pointer.y, rx: pointer.rx, ry: pointer.ry };
    return {
      x: round2(pointer.x * pw + scroll.x * sw),
      y: round2(pointer.y * pw + scroll.y * sw),
      rx: round2(pointer.rx * pw + scroll.rx * sw),
      ry: round2(pointer.ry * pw + scroll.ry * sw),
    };
  }

  function applySpecularVars(el, specular) {
    if (!el || !el.style || !specular) return false;
    el.style.setProperty('--specular-x', specular.x + '%');
    el.style.setProperty('--specular-y', specular.y + '%');
    el.style.setProperty('--refract-x', specular.rx + 'px');
    el.style.setProperty('--refract-y', specular.ry + 'px');
    return true;
  }

  /**
   * Drive all liquid-glass surfaces from pointer + optional scroll metrics.
   */
  function updateSurfaces(surfaces, clientX, clientY, scroll) {
    if (!surfaces || !surfaces.length) return 0;
    let n = 0;
    const scrollSpec =
      scroll && typeof scroll.scrollTop === 'number'
        ? specularFromScroll(scroll.scrollTop, scroll.maxScroll || 0)
        : null;

    for (let i = 0; i < surfaces.length; i++) {
      const el = surfaces[i];
      if (!el || !el.getBoundingClientRect) continue;
      const rect = el.getBoundingClientRect();
      if (rect.width < 2 || rect.height < 2) continue;
      const ptr =
        typeof clientX === 'number' && typeof clientY === 'number'
          ? specularFromPointer(clientX, clientY, rect)
          : null;
      const mixed = blendSpecular(ptr, scrollSpec, ptr ? 0.82 : 0);
      if (applySpecularVars(el, mixed)) n++;
    }
    return n;
  }

  function querySurfaces(root) {
    const doc = root || (typeof document !== 'undefined' ? document : null);
    if (!doc || !doc.querySelectorAll) return [];
    return Array.prototype.slice.call(
      doc.querySelectorAll(
        '.liquid-glass, .glass, .glass-menu, .floating-glass, .glass-btn:not(#save-btn):not(#present-btn), .sidebar-btn'
      )
    );
  }

  /**
   * Attach pointer + scroll listeners. Returns dispose().
   */
  function attach(options) {
    const opts = options || {};
    if (typeof window === 'undefined' || typeof document === 'undefined') {
      return function dispose() {};
    }
    if (window.matchMedia && window.matchMedia('(prefers-reduced-transparency: reduce)').matches) {
      return function dispose() {};
    }
    if (window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
      return function dispose() {};
    }

    let raf = 0;
    let lastX = window.innerWidth * 0.5;
    let lastY = 80;
    const scrollEl =
      opts.scrollEl ||
      document.getElementById('stage-wrap') ||
      document.getElementById('center');
    const maxRefract = opts.maxRefract != null ? opts.maxRefract : 10;

    function scrollMetrics() {
      if (!scrollEl) return { scrollTop: 0, maxScroll: 0 };
      const max = Math.max(0, scrollEl.scrollHeight - scrollEl.clientHeight);
      return { scrollTop: scrollEl.scrollTop || 0, maxScroll: max };
    }

    function tick() {
      raf = 0;
      const surfaces = typeof opts.getSurfaces === 'function' ? opts.getSurfaces() : querySurfaces();
      if (!surfaces || !surfaces.length) return;
      const scrollSpec = scrollMetrics();
      const scroll =
        scrollSpec.maxScroll > 0
          ? specularFromScroll(scrollSpec.scrollTop, scrollSpec.maxScroll)
          : null;
      for (let i = 0; i < surfaces.length; i++) {
        const el = surfaces[i];
        if (!el || !el.getBoundingClientRect) continue;
        const rect = el.getBoundingClientRect();
        if (rect.width < 2 || rect.height < 2) continue;
        const ptr = specularFromPointer(lastX, lastY, rect, { maxRefract });
        const mixed = blendSpecular(ptr, scroll, 0.82);
        applySpecularVars(el, mixed);
      }
    }

    function schedule() {
      if (raf) return;
      raf = window.requestAnimationFrame(tick);
    }

    function onPointer(e) {
      lastX = e.clientX;
      lastY = e.clientY;
      schedule();
    }

    function onScroll() {
      schedule();
    }

    window.addEventListener('pointermove', onPointer, { passive: true });
    if (scrollEl) scrollEl.addEventListener('scroll', onScroll, { passive: true });
    schedule();

    return function dispose() {
      window.removeEventListener('pointermove', onPointer);
      if (scrollEl) scrollEl.removeEventListener('scroll', onScroll);
      if (raf) window.cancelAnimationFrame(raf);
    };
  }

  function clamp(v, lo, hi) {
    return Math.min(hi, Math.max(lo, v));
  }

  function round2(v) {
    return Math.round(v * 100) / 100;
  }

  return {
    specularFromPointer,
    specularFromScroll,
    blendSpecular,
    applySpecularVars,
    updateSurfaces,
    querySurfaces,
    attach,
    clamp,
    round2,
  };
});
