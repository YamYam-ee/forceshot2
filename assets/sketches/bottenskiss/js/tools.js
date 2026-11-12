/* ==========================================================================
   tools.js – verktyg, snäppning, dragning och redigering
   ========================================================================== */
'use strict';

/* ---------------- snäppning ---------------- */
const Snap = (function () {
  let xs = [], ys = [], pts = [], geoRef = null;

  function rebuild(geo) {
    geoRef = geo;
    const sx = {}, sy = {};
    pts = [];
    (geo.snapSegs || geo.seams).forEach(s => {
      if (Math.abs(s.a.x - s.b.x) < 0.5) sx[Math.round(s.a.x)] = 1;
      if (Math.abs(s.a.y - s.b.y) < 0.5) sy[Math.round(s.a.y)] = 1;
      pts.push(s.a, s.b);
    });
    xs = Object.keys(sx).map(Number).sort((a, b) => a - b);
    ys = Object.keys(sy).map(Number).sort((a, b) => a - b);
  }

  function nearest(arr, v, tol) {
    let best = null, bd = tol;
    for (let i = 0; i < arr.length; i++) {
      const d = Math.abs(arr[i] - v);
      if (d < bd) { bd = d; best = arr[i]; }
    }
    return best;
  }

  function tol() { return Render.screenSize(10); }

  /** snäpp x/y oberoende mot plåtskarvar (för rektanglar) */
  function grid(p, off) {
    if (off || !State.doc.view.snap) return { x: p.x, y: p.y, sx: false, sy: false };
    const t = tol();
    const nx = nearest(xs, p.x, t), ny = nearest(ys, p.y, t);
    return { x: nx === null ? p.x : nx, y: ny === null ? p.y : ny, sx: nx !== null, sy: ny !== null };
  }

  /** snäpp mot punkter, skarvlinjer och cistercirklarna */
  function any(p, off) {
    if (off || !State.doc.view.snap) return { x: p.x, y: p.y, kind: null };
    const t = tol();
    const geoR = geoRef;
    const pt2 = Math.min(t, geoR ? geoR.L * 0.16 : t);
    let best = null, bd = pt2;
    for (let i = 0; i < pts.length; i++) {
      const d = dist(p, pts[i]);
      if (d < bd) { bd = d; best = { x: pts[i].x, y: pts[i].y, kind: 'point' }; }
    }
    if (best) return best;
    const g = grid(p, off);
    if (g.sx || g.sy) return { x: g.x, y: g.y, kind: 'grid' };
    const geo = geoRef;
    if (geo) {
      const rr = Math.hypot(p.x, p.y);
      [geo.R, geo.Ri].forEach(rad => {
        if (Math.abs(rr - rad) < t && rr > 1) {
          const k = rad / rr;
          const cand = { x: p.x * k, y: p.y * k, kind: 'circle' };
          if (!best) best = cand;
        }
      });
    }
    return best || { x: p.x, y: p.y, kind: null };
  }

  return { rebuild: rebuild, grid: grid, any: any, tol: tol, get xs() { return xs; }, get ys() { return ys; } };
})();

/* ---------------- verktyg ---------------- */
const Tools = (function () {
  let svg;
  let drag = null;
  let space = false;
  let lastThickness = '';
  let hoverPlate = null;

  const TOOLNAMES = {
    select: 'Välj', split: 'Klyv plåt', doubler: 'Dubbling', 'doubler-circle': 'Rund dubbling',
    repair: 'Reparation', 'repair-circle': 'Rund reparation', 'repair-ring': 'Reparation randplåt', 'repair-edge': 'Kant / svets', point: 'Mätpunkt',
    manhole: 'Mannlucka', nozzle: 'Anslutning', sump: 'Potta', text: 'Text'
  };
  const HINTS = {
    select: 'Klicka för att välja · dra för att flytta · dubbelklicka på en plåt för närbild · mellanslag+dra = panorera',
    split: 'Dra ett streck över plåten – det snäpper och delar plåten. Alt = fri linje',
    doubler: 'Dra ut ett område. Dubblingar som möter varandra växer ihop. Alt = utan snäpp',
    'doubler-circle': 'Klicka i mitten och dra ut radien',
    repair: 'Dra ut ett område · dra längs randplåten för ett bågsegment · klicka på en bottenplåt för närbild',
    'repair-circle': 'Klicka i mitten och dra ut radien',
    'repair-ring': 'Dra runt kanten – segmentet följer randplåten. Klick = hel randplåt. Alt = utan snäpp',
    'repair-edge': 'Markerar på ytterlinjen mot mantelsvetsen – dra runt kanten. Kan flyttas till innerkanten efteråt',
    point: 'Klicka där godstjockleken är uppmätt',
    manhole: 'Klicka eller dra vid cisternkanten – snäpper till manteln',
    nozzle: 'Klicka vid kanten för anslutning · Alt = fri placering',
    sump: 'Dra ut pottans storlek – kan ändras efteråt',
    text: 'Klicka där texten ska peka'
  };

  function init(node) {
    svg = node;
    svg.addEventListener('pointerdown', onDown);
    svg.addEventListener('pointermove', onMove);
    svg.addEventListener('pointerup', onUp);
    svg.addEventListener('pointercancel', onUp);
    svg.addEventListener('dblclick', onDbl);
    svg.addEventListener('wheel', onWheel, { passive: false });
    svg.addEventListener('contextmenu', e => e.preventDefault());
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    State.on('geo', g => Snap.rebuild(g));
    State.on('tool', t => {
      $('#hud-tool').textContent = TOOLNAMES[t] || t;
      $('#hud-hint').textContent = HINTS[t] || '';
      svg.classList.toggle('draw', t !== 'select');
      Render.setOverlayExtra(null);
    });
  }

  /* ---------- hjälp ---------- */
  function mp(e) { return Render.toModel(e.clientX, e.clientY); }
  function tolPick() { return Render.screenSize(7); }

  function itemAt(p) {
    const items = State.doc.items;
    const t = tolPick();
    for (let i = items.length - 1; i >= 0; i--) if (Item.hit(items[i], p, t)) return items[i];
    return null;
  }
  /* ---------- justerbara skarvar ---------- */
  function ensureRowSeams(row) {
    const L = State.doc.layout;
    L.rowSeams = L.rowSeams || {};
    if (!L.rowSeams[row]) L.rowSeams[row] = (State.geo.bands[row].segs || []).slice(1).map(g => g.a);
    return L.rowSeams[row];
  }
  function ensureRowY() {
    const L = State.doc.layout, geo = State.geo;
    if (!Array.isArray(L.rowY) || L.rowY.length !== geo.N - 1) L.rowY = geo.bounds.slice();
    return L.rowY;
  }
  function ensureRingAngles() {
    const d = State.doc, geo = State.geo;
    const n = geo.ring.length;
    if (!Array.isArray(d.ring.angles) || d.ring.angles.length !== n) {
      const base = (d.ring.rot || 0) * D2R;
      d.ring.angles = geo.ring.map(r => {
        let a = r.a0 - geo.rot - base;
        return ((a % TAU) + TAU) % TAU;
      }).sort((a, b) => a - b);
    }
    return d.ring.angles;
  }

  /** skarv under pekaren som går att dra */
  function seamAt(p) {
    const geo = State.geo;
    const tol = Render.screenSize(6);
    let best = null, bs = tol;
    for (const s of geo.seams) {
      if (s.row === undefined && s.bound === undefined && s.ringIdx === undefined) continue;
      const c = closestOnSeg(p, s.a, s.b);
      if (c.d > tol) continue;
      /* nära en ände av skarven? då menade man troligen den korsande skarven */
      const endBias = (c.t < 0.03 || c.t > 0.97) ? tol * 0.8 : 0;
      const score = c.d + endBias;
      if (score < bs) { bs = score; best = s; }
    }
    return best;
  }
  function findSeam(sm) {
    const geo = State.geo;
    return geo.seams.find(s =>
      (sm.type === 'joint' && s.row === sm.row && s.idx === sm.idx) ||
      (sm.type === 'row' && s.bound === sm.bound) ||
      (sm.type === 'ring' && s.ringIdx === sm.ringIdx)) || null;
  }

  function dragSeam(sm, p, alt) {
    const geo = State.geo, doc = State.doc;
    const q = alt ? p : Snap.grid(p, false);
    const lp = geo.IX({ x: q.x, y: q.y });
    if (sm.type === 'joint') {
      const arr = ensureRowSeams(sm.row);
      if (!arr.length) return;
      const c = geo.bands[sm.row].c;
      const lo = sm.idx > 0 ? arr[sm.idx - 1] : -c;
      const hi = sm.idx < arr.length - 1 ? arr[sm.idx + 1] : c;
      const min = Math.max(50, geo.L * 0.05);
      arr[sm.idx] = clamp(lp.x, lo + min, hi - min);
    } else if (sm.type === 'row') {
      const arr = ensureRowY();
      const lo = sm.bound > 0 ? arr[sm.bound - 1] : -geo.Ri;
      const hi = sm.bound < arr.length - 1 ? arr[sm.bound + 1] : geo.Ri;
      const min = Math.max(50, geo.bandH * 0.12);
      arr[sm.bound] = clamp(lp.y, lo + min, hi - min);
    } else if (sm.type === 'ring') {
      const arr = ensureRingAngles();
      const n = arr.length, i = sm.ringIdx;
      const base = (doc.ring.rot || 0) * D2R;
      let v = Math.atan2(lp.y, lp.x) - base;
      while (v - arr[i] > Math.PI) v -= TAU;
      while (arr[i] - v > Math.PI) v += TAU;
      const prev = i > 0 ? arr[i - 1] : arr[n - 1] - TAU;
      const next = i < n - 1 ? arr[i + 1] : arr[0] + TAU;
      const minA = TAU * 0.008;
      arr[i] = clamp(v, prev + minA, next - minA);
    }
    State.regen();
    Render.setOverlayExtra(seamOverlay(sm));
  }

  /** blå markering + mått på båda sidor om skarven */
  function seamOverlay(sm, hoverOnly) {
    const geo = State.geo;
    const s = findSeam(sm);
    if (!s) return null;
    const g = S('g', { 'pointer-events': 'none' });
    g.appendChild(S('line', {
      x1: s.a.x, y1: s.a.y, x2: s.b.x, y2: s.b.y,
      stroke: BLUE, 'stroke-width': Render.screenSize(hoverOnly ? 2.2 : 3), opacity: hoverOnly ? .75 : 1
    }));
    if (hoverOnly) return g;
    const mid = { x: (s.a.x + s.b.x) / 2, y: (s.a.y + s.b.y) / 2 };
    const dir = { x: s.b.x - s.a.x, y: s.b.y - s.a.y };
    const len = Math.hypot(dir.x, dir.y) || 1;
    const nx = -dir.y / len, ny = dir.x / len;
    const off = Render.screenSize(16);
    let m1 = '', m2 = '';
    if (sm.type === 'joint') {
      const b = geo.bands[sm.row];
      const segs = b ? (b.segs || []) : [];
      const wid = g => Math.max(0, Math.min(g.b, b.c) - Math.max(g.a, -b.c));
      if (segs[sm.idx]) m1 = mm(wid(segs[sm.idx]));
      if (segs[sm.idx + 1]) m2 = mm(wid(segs[sm.idx + 1]));
    } else if (sm.type === 'row') {
      const b1 = geo.bands[sm.bound], b2 = geo.bands[sm.bound + 1];
      if (b1) m1 = mm(b1.h);
      if (b2) m2 = mm(b2.h);
    } else {
      const n = geo.ring.length;
      const r0 = geo.ring[(sm.ringIdx - 1 + n) % n], r1 = geo.ring[sm.ringIdx];
      const rm = (geo.Ri + geo.R) / 2;
      if (r0) m1 = mm((r0.a1 - r0.a0) * rm);
      if (r1) m2 = mm((r1.a1 - r1.a0) * rm);
    }
    [[-1, m1], [1, m2]].forEach(pair => {
      const x = mid.x + nx * off * pair[0], y = mid.y + ny * off * pair[0];
      g.appendChild(dimText(x, y, pair[1]));
    });
    return g;
  }

  function northAt(e) {
    const doc = State.doc, geo = State.geo;
    if (!doc.view.north || !doc.view.sheet) return null;
    const sp = Render.toSheet(e.clientX, e.clientY);
    const np = Render.northPos(geo, doc);
    if (dist(sp, np) > geo.R * 0.11) return null;
    if (e.shiftKey) return { kind: 'northrot', c: np, moved: false };
    return { kind: 'north', off: { x: np.x - sp.x, y: np.y - sp.y }, moved: false };
  }
  function splitEndAt(p) {
    for (const id of State.sel) {
      const it = State.byId(id);
      if (!it || it.type !== 'split') continue;
      const r = Render.screenSize(7);
      const sg = Item.seg(it);
      if (dist(p, sg.a) < r) return { it: it, end: 'a' };
      if (dist(p, sg.b) < r) return { it: it, end: 'b' };
    }
    return null;
  }
  function handleAt(p) {
    for (const id of State.sel) {
      const it = State.byId(id); if (!it || !Render.resizable(it)) continue;
      const hs = Render.screenSize(6);
      for (const h of Render.handlePoints(it)) if (Math.abs(p.x - h.x) < hs && Math.abs(p.y - h.y) < hs) return { it: it, h: h };
    }
    return null;
  }

  /* ---------- pekare ---------- */
  function onDown(e) {
    if (e.button === 1 || space || e.button === 2) {
      drag = { kind: 'pan', x: e.clientX, y: e.clientY };
      try { svg.setPointerCapture(e.pointerId); } catch (err) {} svg.classList.add('panning');
      e.preventDefault(); return;
    }
    if (e.button !== 0) return;
    const p = mp(e);
    const tool = State.tool;
    try { svg.setPointerCapture(e.pointerId); } catch (err) {}

    if (tool === 'select') {
      const nd = northAt(e);
      if (nd) { drag = nd; return; }
      const sp = splitEndAt(p);
      if (sp) { drag = { kind: 'splitend', it: sp.it, end: sp.end, moved: false }; return; }
      const h = handleAt(p);
      if (h) { drag = { kind: 'resize', it: h.it, h: h.h, b0: Object.assign({}, Item.bbox(h.it)), s0: h.it.shape ? cloneShape(h.it.shape) : null, p0: p, moved: false }; return; }
      const it = itemAt(p);
      if (it) {
        if (!State.isSel(it.id)) State.select(it.id, e.shiftKey);
        const sub = labelHandleAt(it, p);
        drag = { kind: sub ? 'label' : 'move', it: it, p0: p, last: p, moved: false, ids: State.sel.slice() };
        return;
      }
      const sm = seamAt(p);
      if (sm) {
        drag = { kind: 'seam', sm: sm, moved: false };
        Render.setOverlayExtra(seamOverlay(sm));
        return;
      }
      if (!e.shiftKey) State.select(null);
      drag = { kind: 'marquee', p0: p, p1: p };
      return;
    }

    /* ritverktyg */
    drag = { kind: 'draw', tool: tool, p0: snapFor(tool, p, e.altKey), p1: null, alt: e.altKey, moved: false, raw0: p };
  }

  function onMove(e) {
    const p = mp(e);
    updateCoord(p);
    if (!drag) { hoverUpdate(p); return; }

    if (drag.kind === 'pan') {
      Render.pan(e.clientX - drag.x, e.clientY - drag.y);
      drag.x = e.clientX; drag.y = e.clientY; return;
    }
    if (drag.kind === 'move') {
      const dx = p.x - drag.last.x, dy = p.y - drag.last.y;
      if (Math.abs(p.x - drag.p0.x) + Math.abs(p.y - drag.p0.y) > Render.screenSize(2)) drag.moved = true;
      if (drag.moved) {
        const one = drag.ids.length === 1 ? State.byId(drag.ids[0]) : null;
        if (one && one.shape && one.shape.kind === 'arc') {
          slideArc(one, p, e.altKey, drag);
          State.emit('items');
          return;
        }
        const snapped = snapMove(drag, p, e.altKey);
        drag.ids.forEach(id => { const it = State.byId(id); if (it) Item.move(it, snapped.dx, snapped.dy); });
        drag.last = { x: drag.last.x + snapped.dx, y: drag.last.y + snapped.dy };
        if (drag.ids.some(id => { const t = State.byId(id); return t && t.type === 'split'; })) State.regen();
        else State.emit('items');
      }
      return;
    }
    if (drag.kind === 'splitend') {
      const q = Snap.any(p, e.altKey);
      drag.it[drag.end].x = q.x; drag.it[drag.end].y = q.y;
      drag.moved = true; State.regen(); return;
    }
    if (drag.kind === 'north') {
      const sp = Render.toSheet(e.clientX, e.clientY);
      State.doc.view.northPos = { x: sp.x + drag.off.x, y: sp.y + drag.off.y };
      drag.moved = true; Render.drawSheet(); return;
    }
    if (drag.kind === 'northrot') {
      const sp = Render.toSheet(e.clientX, e.clientY);
      let a = Math.atan2(sp.y - drag.c.y, sp.x - drag.c.x) * R2D + 90 - (State.doc.view.rot || 0);
      if (!e.altKey) a = Math.round(a / 15) * 15;
      a = ((a % 360) + 360) % 360;
      State.doc.view.northRot = a;
      const sl = $('#v-north-rot');
      if (sl) { sl.value = Math.round(a); $('#v-north-rot-v').textContent = Math.round(a) + '°'; }
      drag.moved = true; Render.drawSheet(); return;
    }
    if (drag.kind === 'label') {
      const it = drag.it, c = Item.pos(it);
      it.lx = p.x - c.x; it.ly = p.y - c.y; drag.moved = true;
      State.emit('items'); return;
    }
    if (drag.kind === 'resize') { doResize(drag, p, e.altKey); drag.moved = true; State.emit('items'); return; }
    if (drag.kind === 'seam') { dragSeam(drag.sm, p, e.altKey); drag.moved = true; return; }
    if (drag.kind === 'marquee') { drag.p1 = p; previewMarquee(drag); return; }
    if (drag.kind === 'draw') {
      drag.p1 = snapFor(drag.tool, p, e.altKey);
      drag.raw1 = p;
      drag.alt = e.altKey;
      if (dist(drag.raw0, p) > Render.screenSize(3)) drag.moved = true;
      previewDraw(drag);
      return;
    }
  }

  function onUp(e) {
    svg.classList.remove('panning');
    if (!drag) return;
    const d = drag; drag = null;
    Render.setOverlayExtra(null);

    if (d.kind === 'pan') return;
    if (d.kind === 'move' && d.moved) { State.commit('flytta'); return; }
    if (d.kind === 'label' && d.moved) { State.commit('flytta etikett'); return; }
    if (d.kind === 'resize' && d.moved) { State.commit('ändra storlek'); return; }
    if (d.kind === 'splitend') { if (d.moved) State.commit('justera klyvning'); return; }
    if (d.kind === 'north') { if (d.moved) State.commit('flytta norrpil'); return; }
    if (d.kind === 'northrot') { if (d.moved) State.commit('vrid norrpil'); return; }
    if (d.kind === 'seam') { if (d.moved) State.commit('justera skarv'); return; }
    if (d.kind === 'marquee') {
      const r = rectFrom(d.p0, d.p1);
      if (r.w > Render.screenSize(3) || r.h > Render.screenSize(3)) {
        const ids = State.doc.items.filter(it => rectsOverlap(Item.bbox(it), r)).map(it => it.id);
        State.select(ids, e.shiftKey);
      }
      return;
    }
    if (d.kind === 'draw') finishDraw(d, e);
  }

  function onDbl(e) {
    const p = mp(e);
    if (State.tool !== 'select') return;
    const it = itemAt(p);
    if (it) { Panels.focusProps(); return; }
    const pl = Geo.plateAt(State.geo, p);
    if (pl) PlateView.open(pl);
  }

  function onWheel(e) {
    e.preventDefault();
    const f = Math.pow(1.0016, -e.deltaY);
    Render.zoomAt(f, e.clientX, e.clientY);
  }

  function onKeyDown(e) {
    const ae = document.activeElement;
    const tag = ((e.target && e.target.tagName) || (ae && ae.tagName) || '').toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select') return;
    if ((e.target && e.target.isContentEditable) || (ae && ae.isContentEditable)) return;
    const dlg = $('#dlg'), pv = $('#plate-modal');
    if ((dlg && !dlg.hidden) || (pv && !pv.hidden)) return;
    if (e.code === 'Space') { space = true; svg.classList.add('pan'); e.preventDefault(); return; }
    if (e.key === 'Escape') { drag = null; Render.setOverlayExtra(null); State.select(null); State.setTool('select'); return; }
    if ((e.key === 'Delete' || e.key === 'Backspace') && State.sel.length) { State.remove(State.sel.slice()); e.preventDefault(); return; }
    if (e.ctrlKey || e.metaKey) {
      const k = e.key.toLowerCase();
      if (k === 'z') { e.preventDefault(); e.shiftKey ? State.redo() : State.undo(); }
      else if (k === 'y') { e.preventDefault(); State.redo(); }
      else if (k === 's') { e.preventDefault(); IO.save(); }
      else if (k === 'o') { e.preventDefault(); IO.openDialog(); }
      else if (k === 'd' && State.sel.length) { e.preventDefault(); duplicate(); }
      else if (k === 'a') { e.preventDefault(); State.select(State.doc.items.map(i => i.id)); }
      return;
    }
    const map = { v: 'select', s: 'split', d: 'doubler', c: 'doubler-circle', r: 'repair', b: 'repair-ring', k: 'repair-edge', p: 'point', m: 'manhole', n: 'nozzle', o: 'sump', t: 'text' };
    const k = e.key.toLowerCase();
    if (map[k]) { State.setTool(e.shiftKey && k === 'r' ? 'repair-circle' : map[k]); return; }
    if (k === '0') { Render.fit(); return; }
    if (k === '+' || k === '=') { Render.zoomAt(1.2); return; }
    if (k === '-') { Render.zoomAt(1 / 1.2); return; }
    if (State.sel.length && e.key.indexOf('Arrow') === 0) {
      const step = e.shiftKey ? State.geo.R * 0.02 : State.geo.R * 0.004;
      const dx = e.key === 'ArrowLeft' ? -step : e.key === 'ArrowRight' ? step : 0;
      const dy = e.key === 'ArrowUp' ? -step : e.key === 'ArrowDown' ? step : 0;
      State.sel.forEach(id => { const it = State.byId(id); if (it) Item.move(it, dx, dy); });
      State.emit('items'); State.commit('flytta'); e.preventDefault();
    }
  }
  function onKeyUp(e) { if (e.code === 'Space') { space = false; svg.classList.remove('pan'); } }

  function updateCoord(p) {
    const el = $('#hud-coord'); if (!el) return;
    const r = Math.hypot(p.x, p.y);
    const pl = Geo.plateAt(State.geo, p);
    el.textContent = 'X ' + mm(p.x) + '  Y ' + mm(p.y) + '  ·  r ' + mm(r) + (pl ? '  ·  ' + (pl.kind === 'ring' ? 'Randplåt ' : 'Plåt ') + pl.label : '');
  }
  function hoverUpdate(p) {
    if (State.tool === 'select') {
      const sm = seamAt(p);
      if (sm) {
        const s2 = findSeam(sm);
        let cur = 'move';
        if (s2) {
          const a = Math.atan2(s2.b.y - s2.a.y, s2.b.x - s2.a.x) * R2D + (State.doc.view.rot || 0);
          const d = Math.abs(((a % 180) + 180) % 180 - 90);
          cur = d < 45 ? 'col-resize' : 'row-resize';
        }
        svg.style.cursor = cur;
        hoverPlate = null;
        Render.setOverlayExtra(seamOverlay(sm, true));
        return;
      }
      svg.style.cursor = '';
    }
    if (State.tool !== 'select' && State.tool !== 'repair') return;
    const pl = Geo.plateAt(State.geo, p);
    if (pl === hoverPlate) return;
    hoverPlate = pl;
    if (!pl || drag) { Render.setOverlayExtra(null); return; }
    const g = S('g', { 'pointer-events': 'none' });
    g.appendChild(S('path', {
      d: polyPath(pl.poly), fill: 'rgba(31,111,235,.10)', stroke: BLUE,
      'stroke-width': Render.screenSize(1.4)
    }));
    Render.setOverlayExtra(g);
  }

  /* ---------- randplåtsbågar ---------- */
  /** närmaste randplåtsskarv i radianer, eller null */
  function snapRingAngle(a) {
    const geo = State.geo;
    if (!geo.ringOn || geo.ringWhole || !geo.ring.length || !State.doc.view.snap) return null;
    const rm = (geo.Ri + geo.R) / 2;
    const tol = Render.screenSize(12) / rm;
    let best = null, bd = tol;
    geo.ring.forEach(r => {
      [r.a0, r.a1].forEach(v => {
        let d = v - a;
        while (d > Math.PI) d -= TAU;
        while (d < -Math.PI) d += TAU;
        if (Math.abs(d) < bd) { bd = Math.abs(d); best = a + d; }
      });
    });
    return best;
  }

  /** båge av dragningen om den började i randplåten */
  function ringArcFrom(d, anyRadius) {
    const geo = State.geo;
    if (!geo.ringOn || !geo.ring.length) return null;
    if (!anyRadius) {
      const rr = Math.hypot(d.raw0.x, d.raw0.y);
      if (rr < geo.Ri || rr > geo.R) return null;
    }
    const p1 = d.raw1 || d.raw0;
    let a0 = Math.atan2(d.raw0.y, d.raw0.x);
    let a1 = Math.atan2(p1.y, p1.x);
    let dd = a1 - a0;
    while (dd > Math.PI) dd -= TAU;
    while (dd < -Math.PI) dd += TAU;
    a1 = a0 + dd;
    if (a1 < a0) { const t = a0; a0 = a1; a1 = t; }
    if (!d.alt) {
      const s0 = snapRingAngle(a0); if (s0 !== null) a0 = s0;
      const s1 = snapRingAngle(a1); if (s1 !== null) a1 = s1;
    }
    if (a1 - a0 < TAU * 0.004) return null;
    return { kind: 'arc', a0: a0, a1: a1, r0: geo.Ri, r1: geo.R };
  }

  /** randplåten som ligger i den riktningen */
  function ringPlateAtAngle(a) {
    const geo = State.geo;
    return geo.ring.find(r => {
      let d = a - r.a0;
      while (d < 0) d += TAU;
      while (d >= TAU) d -= TAU;
      return d <= (r.a1 - r.a0);
    }) || null;
  }

  /** glid bågen runt manteln */
  function slideArc(it, p, alt, d) {
    const s = it.shape;
    const span = s.a1 - s.a0;
    if (d.arcOff === undefined) d.arcOff = Math.atan2(d.p0.y, d.p0.x) - (s.a0 + s.a1) / 2;
    let a0 = Math.atan2(p.y, p.x) - d.arcOff - span / 2;
    if (!alt) { const sn = snapRingAngle(a0); if (sn !== null) a0 = sn; }
    s.a0 = a0; s.a1 = a0 + span;
  }

  /* ---------- snäppning per verktyg ---------- */
  /** snäpper rektangelns hörn mot plåtskarvar, men aldrig så att den kollapsar */
  function snapRect(a, b, alt) {
    const raw = rectFrom(a, b);
    if (alt || !State.doc.view.snap) return raw;
    const ga = Snap.grid(a, false), gb = Snap.grid(b, false);
    const snapped = rectFrom(ga, gb);
    const out = { x: snapped.x, y: snapped.y, w: snapped.w, h: snapped.h };
    if (raw.w > 1 && out.w < raw.w * 0.45) { out.x = raw.x; out.w = raw.w; }
    if (raw.h > 1 && out.h < raw.h * 0.45) { out.y = raw.y; out.h = raw.h; }
    return out;
  }

  function snapFor(tool, p, alt) {
    if (tool === 'manhole' || tool === 'nozzle') {
      if (alt) return p;
      const R = State.geo.R, r = Math.hypot(p.x, p.y) || 1;
      return { x: p.x / r * R, y: p.y / r * R, onEdge: true };
    }
    if (tool === 'split') return alt ? Snap.any(p, false) : { x: p.x, y: p.y };
    if (tool === 'repair-ring' || tool === 'repair-edge') return { x: p.x, y: p.y };
    if (tool === 'point' || tool === 'text') return alt ? p : Snap.any(p, alt);
    const g = Snap.grid(p, alt);
    return { x: g.x, y: g.y };
  }
  function snapMove(d, p, alt) {
    const it = d.it;
    let dx = p.x - d.last.x, dy = p.y - d.last.y;
    if (!alt && State.doc.view.snap && (it.type === 'doubler' || it.type === 'repair' || it.type === 'sump') && d.ids.length === 1) {
      const b = Item.bbox(it);
      const target = { x: b.x + dx, y: b.y + dy };
      const g = Snap.grid(target, false);
      dx += g.x - target.x; dy += g.y - target.y;
    }
    return { dx: dx, dy: dy };
  }

  /* ---------- storleksändring ---------- */
  function doResize(d, p, alt) {
    const it = d.it, h = d.h;
    if (h.arcEnd !== undefined && it.shape && it.shape.kind === 'arc') {
      const s = it.shape;
      let a = Math.atan2(p.y, p.x);
      const cur = h.arcEnd === 0 ? s.a0 : s.a1;
      while (a - cur > Math.PI) a -= TAU;
      while (cur - a > Math.PI) a += TAU;
      if (!alt) { const sn = snapRingAngle(a); if (sn !== null) a = sn; }
      const minS = TAU * 0.004, maxS = TAU * 0.985;
      if (h.arcEnd === 0) s.a0 = clamp(a, s.a1 - maxS, s.a1 - minS);
      else s.a1 = clamp(a, s.a0 + minS, s.a0 + maxS);
      return;
    }
    const g = Snap.grid(p, alt);
    const b = Object.assign({}, d.b0);
    let x0 = b.x, y0 = b.y, x1 = b.x + b.w, y1 = b.y + b.h;
    if (h.hx === 0) x0 = Math.min(g.x, x1 - 1); if (h.hx === 2) x1 = Math.max(g.x, x0 + 1);
    if (h.hy === 0) y0 = Math.min(g.y, y1 - 1); if (h.hy === 2) y1 = Math.max(g.y, y0 + 1);
    const nb = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    applyBBox(it, nb, d.s0);
  }
  function applyBBox(it, nb, s0) {
    if (it.type === 'sump') { it.x = nb.x; it.y = nb.y; it.w = nb.w; it.h = nb.h; return; }
    const s = it.shape;
    if (!s) return;
    if (s.kind === 'rect') { s.x = nb.x; s.y = nb.y; s.w = nb.w; s.h = nb.h; }
    else if (s.kind === 'circle') { s.r = Math.max(1, Math.min(nb.w, nb.h) / 2); s.cx = nb.x + nb.w / 2; s.cy = nb.y + nb.h / 2; }
    else if (s.kind === 'poly') {
      const ob = polyBBox(s0 ? s0.pts : s.pts);
      const src = s0 ? s0.pts : s.pts;
      const kx = ob.w ? nb.w / ob.w : 1, ky = ob.h ? nb.h / ob.h : 1;
      s.pts = src.map(q => ({ x: nb.x + (q.x - ob.x) * kx, y: nb.y + (q.y - ob.y) * ky }));
    }
  }

  /* ---------- etiketthandtag (flytta text/nummer) ---------- */
  function labelHandleAt(it, p) {
    if (it.type !== 'point' && it.type !== 'text' && it.type !== 'repair') return false;
    /* står pekaren i själva formen menar man objektet, inte lappen */
    if (it.shape && typeof it.shape === 'object' && shapeHit(it.shape, p, 0)) return false;
    return dist(p, Item.labelPos(it)) < Render.screenSize(14);
  }

  /* ---------- förhandsvisning ---------- */
  function previewMarquee(d) {
    const r = rectFrom(d.p0, d.p1);
    const g = S('g', { 'pointer-events': 'none' });
    g.appendChild(S('rect', {
      x: r.x, y: r.y, width: r.w, height: r.h, fill: 'rgba(31,111,235,.10)',
      stroke: BLUE, 'stroke-width': Render.screenSize(1)
    }));
    Render.setOverlayExtra(g);
  }

  function previewDraw(d) {
    if (!d.p1) return;
    const g = S('g', { 'pointer-events': 'none' });
    const sw = Render.screenSize(1.6);
    const t = d.tool;
    if (t === 'repair' || t === 'repair-ring' || t === 'repair-edge') {
      const arc = ringArcFrom(d, t !== 'repair');
      if (arc) {
        if (t === 'repair-edge') { arc.band = 'edge'; arc.r0 = State.geo.R - State.geo.R * 0.0065; arc.r1 = State.geo.R + State.geo.R * 0.0065; }
        g.appendChild(S('path', {
          d: shapePath(arc), fill: 'rgba(217,45,32,.12)', stroke: RED, 'stroke-width': sw,
          'stroke-dasharray': Render.screenSize(6) + ' ' + Render.screenSize(4)
        }));
        const c = shapeCenter(arc), am = (arc.a0 + arc.a1) / 2;
        g.appendChild(dimText(c.x + Math.cos(am) * State.geo.R * 0.12, c.y + Math.sin(am) * State.geo.R * 0.12, mm(arcLen(arc))));
        Render.setOverlayExtra(g);
        return;
      }
    }
    if (t === 'sump') {
      const r0 = snapRect(d.raw0, d.raw1 || d.p1, d.alt);
      const dia = Math.max(r0.w, r0.h);
      g.appendChild(S('ellipse', {
        cx: r0.x + r0.w / 2, cy: r0.y + r0.h / 2, rx: dia / 2, ry: dia / 2,
        fill: 'rgba(31,111,235,.10)', stroke: BLUE, 'stroke-width': sw,
        'stroke-dasharray': Render.screenSize(6) + ' ' + Render.screenSize(4)
      }));
      g.appendChild(dimText(r0.x + r0.w / 2, r0.y + r0.h / 2 - dia / 2 - Render.screenSize(8), 'Ø ' + mm(dia)));
    } else if (t === 'doubler' || t === 'repair') {
      const r = snapRect(d.raw0, d.raw1 || d.p1, d.alt);
      g.appendChild(S('rect', {
        x: r.x, y: r.y, width: r.w, height: r.h,
        fill: t === 'repair' ? 'rgba(217,45,32,.10)' : 'rgba(31,111,235,.10)',
        stroke: t === 'repair' ? RED : BLUE, 'stroke-width': sw,
        'stroke-dasharray': Render.screenSize(6) + ' ' + Render.screenSize(4)
      }));
      g.appendChild(dimText(r.x + r.w / 2, r.y - Render.screenSize(8), mm(r.w) + ' × ' + mm(r.h)));
    } else if (t === 'doubler-circle' || t === 'repair-circle') {
      const rad = dist(d.p0, d.raw1 || d.p1);
      g.appendChild(S('circle', {
        cx: d.p0.x, cy: d.p0.y, r: rad, fill: t === 'repair-circle' ? 'rgba(217,45,32,.10)' : 'rgba(31,111,235,.10)',
        stroke: t === 'repair-circle' ? RED : BLUE, 'stroke-width': sw,
        'stroke-dasharray': Render.screenSize(6) + ' ' + Render.screenSize(4)
      }));
      g.appendChild(dimText(d.p0.x, d.p0.y - rad - Render.screenSize(8), 'Ø ' + mm(rad * 2)));
    } else if (t === 'split') {
      const s = splitLine(d);
      if (s) {
        g.appendChild(S('line', { x1: s.a.x, y1: s.a.y, x2: s.b.x, y2: s.b.y, stroke: BLUE, 'stroke-width': Render.screenSize(2.2) }));
        [s.a, s.b].forEach(q => g.appendChild(S('circle', { cx: q.x, cy: q.y, r: Render.screenSize(3), fill: '#fff', stroke: BLUE, 'stroke-width': Render.screenSize(1.2) })));
        g.appendChild(dimText((s.a.x + s.b.x) / 2, Math.min(s.a.y, s.b.y) - Render.screenSize(9), mm(dist(s.a, s.b))));
      }
    } else if (t === 'manhole' || t === 'nozzle') {
      const q = d.p1 || d.p0;
      const it = ghostNozzle(t, q, d.alt);
      g.appendChild(t === 'manhole' ? Render.__drawManhole(State.geo, it, 'MH') : Render.__drawNozzle(State.geo, it, 'S'));
      g.setAttribute('opacity', .6);
    }
    Render.setOverlayExtra(g);
  }
  function dimText(x, y, s) {
    const fs = Render.screenSize(12);
    const rot = (State.doc.view.rot || 0);
    return S('text', {
      x: x, y: y, 'text-anchor': 'middle', 'font-family': FONT, 'font-size': fs, 'font-weight': 600,
      fill: BLUE, stroke: '#fff', 'stroke-width': fs * .3, 'paint-order': 'stroke',
      transform: rot ? 'rotate(' + r2(-rot) + ' ' + r2(x) + ' ' + r2(y) + ')' : null
    }, s);
  }
  function ghostNozzle(t, p, alt) {
    const a = Math.atan2(p.y, p.x);
    return t === 'manhole'
      ? { type: 'manhole', angle: a, dn: 600 }
      : { type: 'nozzle', angle: a, dn: 150, free: alt ? { x: p.x, y: p.y } : null, x: p.x, y: p.y };
  }

  /* ---------- klyvlinje ---------- */
  function splitLine(d) {
    const a = d.p0, b = d.p1;
    if (!b) return null;
    if (d.alt) return { a: a, b: b };
    const dx = Math.abs(b.x - a.x), dy = Math.abs(b.y - a.y);
    if (dx < 1 && dy < 1) return null;
    const geo = State.geo;
    const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    const pl = Geo.plateAt(geo, mid) || Geo.plateAt(geo, a);
    const BIG = geo.R * 4;
    const bb = pl ? polyBBox(pl.poly) : null;
    const vertical = dy >= dx;

    /* snitt-koordinat, snäppt mot skarvar men aldrig mot plåtens egen kant */
    let v = vertical ? mid.x : mid.y;
    const g = Snap.grid(vertical ? { x: v, y: mid.y } : { x: mid.x, y: v }, false);
    const gv = vertical ? g.x : g.y;
    if (Math.abs(gv - v) < Snap.tol()) {
      const e0 = vertical ? bb && bb.x : bb && bb.y;
      const e1 = vertical ? bb && (bb.x + bb.w) : bb && (bb.y + bb.h);
      const onEdge = bb && (Math.abs(gv - e0) < Math.max(2, geo.R * 0.002) || Math.abs(gv - e1) < Math.max(2, geo.R * 0.002));
      if (!onEdge) v = gv;
    }

    /* klipp exakt mot plåtens kontur – kan aldrig gå ut i randplåten */
    if (pl) {
      const p1 = vertical ? { x: v, y: -BIG } : { x: -BIG, y: v };
      const p2 = vertical ? { x: v, y: BIG } : { x: BIG, y: v };
      const seg = Geo.lineInPoly(pl.poly, p1, p2);
      if (seg) return seg;
    }
    const lim = Geo.halfW(geo.Ri, v);
    if (vertical) {
      return {
        a: { x: v, y: Math.max(Math.min(a.y, b.y), -lim) },
        b: { x: v, y: Math.min(Math.max(a.y, b.y), lim) }
      };
    }
    return {
      a: { x: Math.max(Math.min(a.x, b.x), -lim), y: v },
      b: { x: Math.min(Math.max(a.x, b.x), lim), y: v }
    };
  }

  /* ---------- slutför ritning ---------- */
  function finishDraw(d, e) {
    const t = d.tool;
    const p1 = d.p1 || d.p0;
    const minSize = Render.screenSize(6);

    if (t === 'repair' || t === 'repair-ring' || t === 'repair-edge') {
      const arc = ringArcFrom(d, t !== 'repair');
      if (arc) {
        if (t === 'repair-edge') arc.band = 'edge';
        addRepair(arc, { kindName: t === 'repair-edge' ? 'Kant/svets ytterkant' : 'Reparation randplåt' });
        return;
      }
    }
    if (t === 'repair-edge') {
      const geo = State.geo;
      if (!geo.ringOn || !geo.ring.length) { toast('Randplåten är avstängd – slå på den först', 'warn'); return; }
      const a = Math.atan2(d.raw0.y, d.raw0.x);
      const rp = ringPlateAtAngle(a);
      const a0 = rp ? rp.a0 : a - 0.06, a1 = rp ? rp.a1 : a + 0.06;
      addRepair({ kind: 'arc', band: 'edge', a0: a0, a1: a1, r0: geo.R, r1: geo.R },
        { kindName: 'Kant/svets ytterkant' });
      return;
    }
    if (t === 'repair-ring') {
      /* för kort dragning = klick: markera hela randplåten i den riktningen */
      const geo = State.geo;
      if (!geo.ringOn || !geo.ring.length) { toast('Randplåten är avstängd – slå på den först', 'warn'); return; }
      const rp = ringPlateAtAngle(Math.atan2(d.raw0.y, d.raw0.x));
      if (rp) addRepair({ kind: 'arc', a0: rp.a0, a1: rp.a1, r0: rp.r0, r1: rp.r1 }, { kindName: 'Reparation randplåt' });
      return;
    }
    if (t === 'doubler' || t === 'repair' || t === 'sump') {
      const r = snapRect(d.raw0, d.raw1 || p1, d.alt);
      if (r.w < minSize || r.h < minSize) {
        if (t === 'repair') return clickRepair(d.raw0);
        if (t === 'sump') { const s = State.geo.R * 0.12; makeSump({ x: d.p0.x - s / 2, y: d.p0.y - s / 2, w: s, h: s }); return; }
        return;
      }
      if (t === 'sump') { makeSump(r); return; }
      if (t === 'doubler') { addDoubler({ kind: 'rect', x: r.x, y: r.y, w: r.w, h: r.h }); return; }
      if (t === 'repair') { addRepair({ kind: 'rect', x: r.x, y: r.y, w: r.w, h: r.h }); return; }
      makeSump(r); return;
    }
    if (t === 'doubler-circle' || t === 'repair-circle') {
      const rad = dist(d.p0, d.raw1 || p1);
      if (rad < minSize) return;
      const sh = { kind: 'circle', cx: d.p0.x, cy: d.p0.y, r: rad };
      if (t === 'doubler-circle') addDoubler(sh); else addRepair(sh);
      return;
    }
    if (t === 'split') {
      const s = splitLine(d);
      if (!s || dist(s.a, s.b) < minSize) return;
      const it = State.add({ type: 'split', a: s.a, b: s.b });
      State.select(it.id);
      return;
    }
    if (t === 'point') {
      const p = d.p1 || d.p0;
      const it = State.add({ type: 'point', x: p.x, y: p.y, value: lastThickness });
      State.select(it.id);
      Dlg.open('Mätpunkt – godstjocklek', [
        { key: 'value', label: 'Uppmätt tjocklek', value: lastThickness, ph: 't.ex. 4,0 mm' },
        { key: 'note', label: 'Anmärkning (valfritt)', value: '', ph: 't.ex. gropfrätning ovansida' }
      ], out => {
        it.value = out.value; it.note = out.note;
        lastThickness = out.value;
        State.touch('mätpunkt');
      });
      return;
    }
    if (t === 'text') {
      const p = d.p1 || d.p0;
      Dlg.open('Anteckning', [{ key: 'text', label: 'Text', type: 'textarea', value: '' }], out => {
        if (!out.text.trim()) return;
        const it = State.add({ type: 'text', x: p.x, y: p.y, text: out.text.trim() });
        State.select(it.id);
      });
      return;
    }
    if (t === 'manhole' || t === 'nozzle') {
      const p = p1;
      const a = Math.atan2(p.y, p.x);
      const it = t === 'manhole'
        ? { type: 'manhole', angle: a, dn: 600, note: '' }
        : { type: 'nozzle', angle: a, dn: 150, note: '', free: d.alt ? { x: p.x, y: p.y } : null, x: p.x, y: p.y };
      const added = State.add(it);
      State.select(added.id);
      State.setTool('select');
      return;
    }
  }

  /* ---------- skapare ---------- */
  function addDoubler(shape) {
    const it = State.add({ type: 'doubler', shape: shape, note: '', thickness: '' });
    const grp = State.groups().find(g => g.ids.indexOf(it.id) >= 0);
    if (grp && grp.ids.length > 1) toast('Dubbling sammanslagen med ' + (grp.ids.length - 1) + ' befintlig(a)', 'ok');
    State.select(it.id);
  }
  function addRepair(shape, opts) {
    const o = opts || {};
    const it = State.add(Object.assign({ type: 'repair', shape: shape, note: '', thickness: '' }, o));
    if (shape.kind === 'arc') State.regen();
    State.select(it.id);
    Dlg.open('Reparation / anmärkning', [
      { key: 'thickness', label: 'Tjocklek (om uppmätt)', value: it.thickness || '', ph: 't.ex. 3,6 mm' },
      { key: 'note', label: 'Beskrivning', type: 'textarea', value: it.note || '', ph: 'Vad har hittats? t.ex. genomfrätning, gropfrätning, sprucken svets…' }
    ], out => { it.thickness = out.thickness; it.note = out.note; State.touch('reparation'); });
    return it;
  }
  function clickRepair(p) {
    const pl = Geo.plateAt(State.geo, p);
    if (!pl) return;
    if (pl.kind === 'ring') {
      addRepair({ kind: 'arc', a0: pl.a0, a1: pl.a1, r0: pl.r0, r1: pl.r1 }, { kindName: 'Reparation randplåt' });
      return;
    }
    PlateView.open(pl);
  }
  function makeSump(r) {
    const d0 = Math.max(r.w, r.h);
    const cx = r.x + r.w / 2, cy = r.y + r.h / 2;
    const it = State.add({
      type: 'sump', shape: 'circle',
      x: cx - d0 / 2, y: cy - d0 / 2, w: d0, h: d0, depth: 300, note: ''
    });
    State.select(it.id);
  }
  function duplicate() {
    const copies = [];
    const off = State.geo.R * 0.04;
    State.sel.forEach(id => {
      const it = State.byId(id); if (!it) return;
      const c = JSON.parse(JSON.stringify(it));
      delete c.id; delete c.seq;
      const n = State.add(c, true);
      Item.move(n, off, off);
      copies.push(n.id);
    });
    if (copies.length) { State.emit('items'); State.commit('duplicera'); State.select(copies); }
  }

  /* ---------- dra från verktygsraden ---------- */
  function initRailDrag(rail) {
    rail.addEventListener('pointerdown', e => {
      const btn = e.target.closest('.tool'); if (!btn) return;
      const tool = btn.dataset.tool;
      State.setTool(tool);
      if (['manhole', 'nozzle', 'sump'].indexOf(tool) < 0) return;
      let dragging = false;
      const move = ev => {
        const r = svg.getBoundingClientRect();
        const inside = ev.clientX > r.left && ev.clientX < r.right && ev.clientY > r.top && ev.clientY < r.bottom;
        if (!inside) { Render.setOverlayExtra(null); return; }
        dragging = true;
        const p = Render.toModel(ev.clientX, ev.clientY);
        const g = S('g', { 'pointer-events': 'none', opacity: .6 });
        if (tool === 'sump') {
          const s = State.geo.R * 0.12;
          g.appendChild(S('circle', { cx: p.x, cy: p.y, r: s / 2, fill: 'url(#hatch-s)', stroke: INK, 'stroke-width': State.geo.lw.doubler }));
        } else {
          const q = snapFor(tool, p, ev.altKey);
          const it = ghostNozzle(tool, q, ev.altKey);
          g.appendChild(tool === 'manhole' ? Render.__drawManhole(State.geo, it, 'MH') : Render.__drawNozzle(State.geo, it, 'S'));
        }
        Render.setOverlayExtra(g);
      };
      const up = ev => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        Render.setOverlayExtra(null);
        const r = svg.getBoundingClientRect();
        const inside = ev.clientX > r.left && ev.clientX < r.right && ev.clientY > r.top && ev.clientY < r.bottom;
        if (!dragging || !inside) return;
        const p = Render.toModel(ev.clientX, ev.clientY);
        if (tool === 'sump') { const s = State.geo.R * 0.12; makeSump({ x: p.x - s / 2, y: p.y - s / 2, w: s, h: s }); }
        else {
          const q = snapFor(tool, p, ev.altKey);
          const a = Math.atan2(q.y, q.x);
          const it = tool === 'manhole'
            ? { type: 'manhole', angle: a, dn: 600, note: '' }
            : { type: 'nozzle', angle: a, dn: 150, note: '', free: ev.altKey ? { x: q.x, y: q.y } : null, x: q.x, y: q.y };
          State.select(State.add(it).id);
        }
        State.setTool('select');
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    });
  }

  return {
    init: init, initRailDrag: initRailDrag, duplicate: duplicate,
    addRepair: addRepair, addDoubler: addDoubler
  };
})();
