/* ==========================================================================
   plateview.js – närbild på enskild plåt ("kortvy") för exakt inmätning
   ========================================================================== */
'use strict';

const PlateView = (function () {
  let svg, plate = null, view = { tx: 0, ty: 0, s: 1 };
  let tool = 'select', drag = null, selId = null, freePts = null;
  let open_ = false;

  function init() {
    svg = $('#pv-svg');
    $('#pv-close').addEventListener('click', close);
    $$('#pv-tools .pvt').forEach(b => b.addEventListener('click', () => {
      tool = b.dataset.pvt;
      $$('#pv-tools .pvt').forEach(x => x.classList.toggle('on', x === b));
    }));
    svg.addEventListener('pointerdown', onDown);
    svg.addEventListener('pointermove', onMove);
    svg.addEventListener('pointerup', onUp);
    svg.addEventListener('wheel', e => {
      e.preventDefault();
      const f = Math.pow(1.0016, -e.deltaY);
      const r = svg.getBoundingClientRect();
      const mx = e.clientX - r.left, my = e.clientY - r.top;
      const bx = (mx - view.tx) / view.s, by = (my - view.ty) / view.s;
      view.s = clamp(view.s * f, 0.001, 60);
      view.tx = mx - bx * view.s; view.ty = my - by * view.s;
      draw();
    }, { passive: false });
    window.addEventListener('keydown', e => {
      if (!open_) return;
      if (e.key === 'Escape') { e.stopPropagation(); close(); }
      if ((e.key === 'Delete' || e.key === 'Backspace') && selId) {
        const tag = (e.target.tagName || '').toLowerCase();
        if (tag === 'input' || tag === 'textarea') return;
        State.remove(selId); selId = null; draw();
      }
    }, true);
  }

  function open(pl) {
    plate = pl; open_ = true; selId = null; tool = 'select';
    $$('#pv-tools .pvt').forEach(x => x.classList.toggle('on', x.dataset.pvt === 'select'));
    $('#plate-modal').hidden = false;
    $('#pv-title').textContent = (pl.kind === 'ring' ? 'Randplåt ' : 'Plåt ') + pl.label;
    const dims = pl.kind === 'ring'
      ? 'Randplåt · bredd ' + mm(pl.r1 - pl.r0) + ' mm · båglängd ' + mm((pl.a1 - pl.a0) * (pl.r0 + pl.r1) / 2) + ' mm'
      : 'Plåtmått ' + mm(pl.w) + ' × ' + mm(pl.h) + ' mm' +
        (pl.split ? ' · kluven plåt' : (pl.clipped ? ' · kapad mot manteln' : ''));
    $('#pv-sub').textContent = dims;
    setTimeout(() => { if (plate) { fit(); draw(); readout(Geo.toModel(plate, { u: plate.w / 2, v: plate.h / 2 })); } }, 20);
  }
  function close() { open_ = false; $('#plate-modal').hidden = true; plate = null; Render.drawItems(); }

  function fit() {
    const r = svg.getBoundingClientRect();
    const b = polyBBox(plate.kind === 'ring' ? plate.poly : plate.quad);
    const pad = Math.max(b.w, b.h) * 0.30;
    const w = b.w + pad * 2, h = b.h + pad * 2;
    view.s = Math.min(r.width / w, r.height / h);
    view.tx = r.width / 2 - (b.x + b.w / 2) * view.s;
    view.ty = r.height / 2 - (b.y + b.h / 2) * view.s;
  }
  const sc = px => px / view.s;
  function mp(e) {
    const r = svg.getBoundingClientRect();
    return { x: (e.clientX - r.left - view.tx) / view.s, y: (e.clientY - r.top - view.ty) / view.s };
  }

  /* ---------- objekt som rör plåten ---------- */
  function nearItems() {
    const b = polyBBox(plate.poly);
    const pad = Math.max(b.w, b.h) * 0.6;
    const box = { x: b.x - pad, y: b.y - pad, w: b.w + pad * 2, h: b.h + pad * 2 };
    return State.doc.items.filter(it => {
      if (it.type === 'manhole' || it.type === 'nozzle' || it.type === 'text') return false;
      return rectsOverlap(Item.bbox(it), box);
    });
  }

  /* ---------- ritning ---------- */
  function draw() {
    if (!plate) return;
    const geo = State.geo;
    while (svg.firstChild) svg.removeChild(svg.firstChild);
    const root = S('g', { transform: 'translate(' + view.tx + ' ' + view.ty + ') scale(' + view.s + ')' });
    svg.appendChild(root);

    const defs = S('defs');
    const g = geo.lw.hatchGap * 0.5;
    defs.appendChild(S('pattern', { id: 'pv-hd', patternUnits: 'userSpaceOnUse', width: g, height: g, patternTransform: 'rotate(45)' },
      [S('path', { d: 'M0 0V' + g, stroke: '#1c222b', 'stroke-width': sc(0.9), opacity: .6 })]));
    defs.appendChild(S('pattern', { id: 'pv-hr', patternUnits: 'userSpaceOnUse', width: g * .8, height: g * .8, patternTransform: 'rotate(-45)' },
      [S('path', { d: 'M0 0V' + (g * .8), stroke: RED, 'stroke-width': sc(0.9), opacity: .55 })]));
    root.appendChild(defs);

    /* grannplåtar */
    const nb = S('g');
    geo.plates.concat(geo.ring).forEach(p => {
      if (p === plate) return;
      nb.appendChild(S('path', { d: polyPath(p.poly), fill: '#eef1f5', stroke: '#c3cbd6', 'stroke-width': sc(1) }));
    });
    root.appendChild(nb);

    /* egen plåt */
    if (plate.kind !== 'ring' && plate.clipped) {
      root.appendChild(S('path', { d: polyPath(plate.quad), fill: 'none', stroke: '#c3cbd6', 'stroke-width': sc(1), 'stroke-dasharray': sc(6) + ' ' + sc(4) }));
    }
    root.appendChild(S('path', { d: polyPath(plate.poly), fill: '#ffffff', stroke: INK, 'stroke-width': sc(2) }));

    /* linjal längs plåtkanterna */
    root.appendChild(ruler());

    /* objekt */
    const nums = State.numbers();
    nearItems().forEach(it => {
      const gg = S('g', { 'data-id': it.id, style: 'cursor:pointer' });
      if (it.type === 'doubler') {
        gg.appendChild(S('path', { d: shapePath(it.shape), fill: 'url(#pv-hd)', stroke: INK, 'stroke-width': sc(2.2) }));
      } else if (it.type === 'repair') {
        gg.appendChild(S('path', { d: shapePath(it.shape), fill: 'url(#pv-hr)', stroke: RED, 'stroke-width': sc(2) }));
        const c = shapeCenter(it.shape), b = shapeBBox(it.shape);
        gg.appendChild(S('text', {
          x: c.x, y: b.y - sc(8), 'text-anchor': 'middle', 'font-family': FONT, 'font-size': sc(15),
          'font-weight': 700, fill: RED, stroke: '#fff', 'stroke-width': sc(4), 'paint-order': 'stroke'
        }, (nums[it.id] || '') + (it.thickness ? '  ' + it.thickness : '')));
      } else if (it.type === 'point') {
        gg.appendChild(S('circle', { cx: it.x, cy: it.y, r: sc(5), fill: RED }));
        gg.appendChild(S('text', {
          x: it.x + sc(9), y: it.y - sc(7), 'font-family': FONT, 'font-size': sc(14), 'font-weight': 600,
          fill: RED, stroke: '#fff', 'stroke-width': sc(4), 'paint-order': 'stroke'
        }, it.value || ''));
      } else if (it.type === 'sump') {
        gg.appendChild(S('rect', { x: it.x, y: it.y, width: it.w, height: it.h, fill: 'none', stroke: INK, 'stroke-width': sc(2) }));
      } else if (it.type === 'split') {
        const sg = Item.seg(it);
        gg.appendChild(S('line', { x1: sg.a.x, y1: sg.a.y, x2: sg.b.x, y2: sg.b.y, stroke: INK, 'stroke-width': sc(2) }));
      }
      if (it.id === selId) {
        const b = Item.bbox(it), p = sc(4);
        gg.appendChild(S('rect', {
          x: b.x - p, y: b.y - p, width: b.w + p * 2, height: b.h + p * 2, fill: 'none',
          stroke: BLUE, 'stroke-width': sc(1.4), 'stroke-dasharray': sc(6) + ' ' + sc(4)
        }));
        if (Render.resizable(it)) Render.handlePoints(it).forEach(h =>
          gg.appendChild(S('rect', { x: h.x - sc(5), y: h.y - sc(5), width: sc(10), height: sc(10), fill: '#fff', stroke: BLUE, 'stroke-width': sc(1.6) })));
      }
      root.appendChild(gg);
    });

    /* pågående ritning */
    if (drag && drag.kind === 'draw' && drag.p1) {
      const t = drag.tool;
      if (t === 'rect' || t === 'doubler') {
        const r = rectFrom(drag.p0, drag.p1);
        root.appendChild(S('rect', {
          x: r.x, y: r.y, width: r.w, height: r.h, fill: t === 'rect' ? 'rgba(217,45,32,.12)' : 'rgba(31,111,235,.10)',
          stroke: t === 'rect' ? RED : BLUE, 'stroke-width': sc(1.6), 'stroke-dasharray': sc(6) + ' ' + sc(4)
        }));
        root.appendChild(dim(r.x + r.w / 2, r.y - sc(9), mm(r.w) + ' × ' + mm(r.h)));
      } else if (t === 'circle') {
        const rad = dist(drag.p0, drag.p1);
        root.appendChild(S('circle', { cx: drag.p0.x, cy: drag.p0.y, r: rad, fill: 'rgba(217,45,32,.12)', stroke: RED, 'stroke-width': sc(1.6), 'stroke-dasharray': sc(6) + ' ' + sc(4) }));
        root.appendChild(dim(drag.p0.x, drag.p0.y - rad - sc(9), 'Ø ' + mm(rad * 2)));
      }
    }
    if (freePts && freePts.length > 1) {
      root.appendChild(S('path', { d: polyPath(freePts, true), fill: 'rgba(217,45,32,.10)', stroke: RED, 'stroke-width': sc(1.8) }));
    }
    sidebar();
  }

  function dim(x, y, s) {
    return S('text', {
      x: x, y: y, 'text-anchor': 'middle', 'font-family': FONT, 'font-size': sc(14), 'font-weight': 600,
      fill: BLUE, stroke: '#fff', 'stroke-width': sc(4), 'paint-order': 'stroke'
    }, s);
  }

  /** måttlinjal längs plåtens egna kanter */
  function ruler() {
    const g = S('g');
    if (plate.kind === 'ring') return g;
    const stepCandidates = [50, 100, 200, 250, 500, 1000];
    const step = stepCandidates.find(s => s * view.s > 26) || 1000;
    const bigEvery = step >= 500 ? 1 : (step >= 200 ? 2 : 5);
    const off = sc(16), tick = sc(7), fs = sc(12);
    /* u-axel (undre kanten) */
    for (let u = 0; u <= plate.w + 1; u += step) {
      const p = Geo.toModel(plate, { u: u, v: plate.h });
      const q = Geo.toModel(plate, { u: u, v: plate.h + sc(0) });
      const dir = { x: plate.vy.x, y: plate.vy.y };
      const a = { x: q.x + dir.x * off, y: q.y + dir.y * off };
      const b = { x: a.x + dir.x * tick, y: a.y + dir.y * tick };
      const isBig = Math.round(u / step) % bigEvery === 0;
      g.appendChild(S('line', { x1: a.x, y1: a.y, x2: b.x, y2: b.y, stroke: '#8a94a3', 'stroke-width': sc(isBig ? 1.4 : .8) }));
      if (isBig) g.appendChild(S('text', {
        x: b.x + dir.x * fs * .9, y: b.y + dir.y * fs * .9 + fs * .35, 'text-anchor': 'middle',
        'font-family': FONT, 'font-size': fs, fill: '#6b7686'
      }, mm(u)));
    }
    /* v-axel (vänstra kanten) – mätt från nedre kant */
    for (let d0 = 0; d0 <= plate.h + 1; d0 += step) {
      const v = plate.h - d0;
      const p = Geo.toModel(plate, { u: 0, v: v });
      const dir = { x: -plate.ux.x, y: -plate.ux.y };
      const a = { x: p.x + dir.x * off, y: p.y + dir.y * off };
      const b = { x: a.x + dir.x * tick, y: a.y + dir.y * tick };
      const isBig = Math.round(d0 / step) % bigEvery === 0;
      g.appendChild(S('line', { x1: a.x, y1: a.y, x2: b.x, y2: b.y, stroke: '#8a94a3', 'stroke-width': sc(isBig ? 1.4 : .8) }));
      if (isBig) g.appendChild(S('text', {
        x: b.x + dir.x * fs * .7, y: b.y + dir.y * fs * .7 + fs * .35, 'text-anchor': 'end',
        'font-family': FONT, 'font-size': fs, fill: '#6b7686'
      }, mm(d0)));
    }
    return g;
  }

  /* ---------- interaktion ---------- */
  function onDown(e) {
    if (e.button === 1 || e.button === 2) { drag = { kind: 'pan', x: e.clientX, y: e.clientY }; return; }
    const p = mp(e);
    try { svg.setPointerCapture(e.pointerId); } catch (err) {}
    if (tool === 'select') {
      if (selId) {
        const it = State.byId(selId);
        if (it && Render.resizable(it)) {
          for (const h of Render.handlePoints(it)) {
            if (Math.abs(p.x - h.x) < sc(7) && Math.abs(p.y - h.y) < sc(7)) {
              drag = { kind: 'resize', it: it, h: h, b0: Object.assign({}, Item.bbox(it)), s0: it.shape ? cloneShape(it.shape) : null };
              return;
            }
          }
        }
      }
      const hit = nearItems().slice().reverse().find(it => Item.hit(it, p, sc(6)));
      selId = hit ? hit.id : null;
      if (hit) { State.select(hit.id); drag = { kind: 'move', it: hit, last: p, moved: false }; }
      draw();
      return;
    }
    if (tool === 'free') { freePts = [p]; drag = { kind: 'free' }; return; }
    if (tool === 'point') {
      const it = State.add({ type: 'point', x: p.x, y: p.y, value: '' });
      selId = it.id;
      Dlg.open('Mätpunkt på plåt ' + plate.label, [
        { key: 'value', label: 'Godstjocklek', value: '', ph: 't.ex. 4,0 mm', hint: localHint(p) },
        { key: 'note', label: 'Anmärkning', value: '', ph: '' }
      ], out => { it.value = out.value; it.note = out.note; State.touch('mätpunkt'); draw(); });
      draw();
      return;
    }
    drag = { kind: 'draw', tool: tool, p0: p, p1: null };
  }

  function onMove(e) {
    const p = mp(e);
    readout(p);
    if (!drag) return;
    if (drag.kind === 'pan') { view.tx += e.clientX - drag.x; view.ty += e.clientY - drag.y; drag.x = e.clientX; drag.y = e.clientY; draw(); return; }
    if (drag.kind === 'move') {
      Item.move(drag.it, p.x - drag.last.x, p.y - drag.last.y);
      drag.last = p; drag.moved = true; draw(); return;
    }
    if (drag.kind === 'resize') {
      const b = drag.b0, h = drag.h;
      let x0 = b.x, y0 = b.y, x1 = b.x + b.w, y1 = b.y + b.h;
      if (h.hx === 0) x0 = Math.min(p.x, x1 - 1); if (h.hx === 2) x1 = Math.max(p.x, x0 + 1);
      if (h.hy === 0) y0 = Math.min(p.y, y1 - 1); if (h.hy === 2) y1 = Math.max(p.y, y0 + 1);
      applyBox(drag.it, { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }, drag.s0);
      drag.moved = true; draw(); return;
    }
    if (drag.kind === 'free') {
      const last = freePts[freePts.length - 1];
      if (dist(last, p) > sc(4)) freePts.push(p);
      draw(); return;
    }
    if (drag.kind === 'draw') { drag.p1 = p; draw(); }
  }

  function onUp(e) {
    if (!drag) return;
    const d = drag; drag = null;
    if (d.kind === 'move' || d.kind === 'resize') { if (d.moved) State.commit('redigera'); Render.drawItems(); return; }
    if (d.kind === 'free') {
      const pts = freePts; freePts = null;
      if (pts && pts.length > 3) newRepair({ kind: 'poly', pts: pts });
      draw(); return;
    }
    if (d.kind === 'draw' && d.p1) {
      if (d.tool === 'rect' || d.tool === 'doubler') {
        const r = rectFrom(d.p0, d.p1);
        if (r.w < sc(6) || r.h < sc(6)) { draw(); return; }
        if (d.tool === 'doubler') {
          const it = State.add({ type: 'doubler', shape: { kind: 'rect', x: r.x, y: r.y, w: r.w, h: r.h }, note: '', thickness: '' });
          selId = it.id; State.select(it.id); draw();
        } else newRepair({ kind: 'rect', x: r.x, y: r.y, w: r.w, h: r.h });
      } else if (d.tool === 'circle') {
        const rad = dist(d.p0, d.p1);
        if (rad < sc(4)) { draw(); return; }
        newRepair({ kind: 'circle', cx: d.p0.x, cy: d.p0.y, r: rad });
      }
      return;
    }
    draw();
  }

  function applyBox(it, nb, s0) {
    if (it.type === 'sump') { it.x = nb.x; it.y = nb.y; it.w = nb.w; it.h = nb.h; return; }
    const s = it.shape; if (!s || typeof s !== 'object') return;
    if (s.kind === 'rect') { s.x = nb.x; s.y = nb.y; s.w = nb.w; s.h = nb.h; }
    else if (s.kind === 'circle') { s.r = Math.max(1, Math.min(nb.w, nb.h) / 2); s.cx = nb.x + nb.w / 2; s.cy = nb.y + nb.h / 2; }
    else if (s.kind === 'poly') {
      const src = s0 ? s0.pts : s.pts, ob = polyBBox(src);
      const kx = ob.w ? nb.w / ob.w : 1, ky = ob.h ? nb.h / ob.h : 1;
      s.pts = src.map(q => ({ x: nb.x + (q.x - ob.x) * kx, y: nb.y + (q.y - ob.y) * ky }));
    }
  }

  function newRepair(shape) {
    const c = shapeCenter(shape);
    const it = State.add({ type: 'repair', shape: shape, note: '', thickness: '', plateId: plate.label });
    selId = it.id; State.select(it.id);
    Dlg.open('Anmärkning på plåt ' + plate.label, [
      { key: 'thickness', label: 'Tjocklek (om uppmätt)', value: '', ph: 't.ex. 3,6 mm', hint: localHint(c) },
      { key: 'note', label: 'Beskrivning', type: 'textarea', value: '', ph: 'Vad har hittats?' }
    ], out => { it.thickness = out.thickness; it.note = out.note; State.touch('anmärkning'); draw(); });
    draw();
  }

  function localHint(p) {
    if (plate.kind === 'ring') return '';
    const l = Geo.toLocal(plate, p);
    return 'Läge på plåten: ' + mm(l.u) + ' mm från vänster kant, ' + mm(plate.h - l.v) + ' mm från nedre kant';
  }

  function readout(p) {
    const box = $('#pv-readout'); if (!box) return;
    if (plate.kind === 'ring') {
      box.innerHTML = 'Randplåt <b>' + plate.label + '</b><br>Bredd <b>' + mm(plate.r1 - plate.r0) + ' mm</b>';
      return;
    }
    const l = Geo.toLocal(plate, p);
    box.innerHTML =
      'Plåt <b>' + plate.label + '</b> · ' + mm(plate.w) + ' × ' + mm(plate.h) + ' mm' +
      '<br>Från vänster kant <b>' + mm(clamp(l.u, -99999, 99999)) + ' mm</b>' +
      '<br>Från nedre kant <b>' + mm(plate.h - l.v) + ' mm</b>' +
      '<br>Från övre kant <b>' + mm(l.v) + ' mm</b>';
  }

  /* ---------- sidopanel i kortvyn ---------- */
  function sidebar() {
    const host = $('#pv-props');
    if (host.contains(document.activeElement)) return;
    host.innerHTML = '';
    const list = $('#pv-list'); list.innerHTML = '';
    const nums = State.numbers();

    const quick = H('button', { class: 'btn ghost', text: 'Markera hela plåten som reparation', style: 'width:100%' });
    quick.onclick = () => {
      newRepair({ kind: 'poly', pts: plate.poly.map(q => ({ x: q.x, y: q.y })) });
    };
    host.appendChild(quick);

    const it = selId ? State.byId(selId) : null;
    if (it) {
      const c = Item.pos(it), b = Item.bbox(it);
      const info = H('div', { class: 'pv-readout' });
      let html = '<b>' + (nums[it.id] ? nums[it.id] + ' · ' : '') + (it.type === 'repair' ? 'Reparation' : it.type === 'doubler' ? 'Dubbling' : it.type === 'point' ? 'Mätpunkt' : it.type) + '</b>';
      if (plate.kind !== 'ring') {
        const l = Geo.toLocal(plate, c);
        html += '<br>Läge <b>' + mm(l.u) + ' / ' + mm(plate.h - l.v) + ' mm</b> (vänster / nedre kant)';
      }
      if (it.shape && typeof it.shape === 'object') {
        html += '<br>Mått <b>' + (it.shape.kind === 'circle' ? 'Ø' + mm(it.shape.r * 2) : mm(b.w) + ' × ' + mm(b.h)) + ' mm</b>';
        html += '<br>Yta <b>' + num(shapeArea(it.shape) / 1e6, 3) + ' m²</b>';
      }
      info.innerHTML = html;
      host.appendChild(info);

      if (it.type !== 'split') {
        const f1 = H('label', { class: 'fld' }, [H('span', { text: it.type === 'point' ? 'Godstjocklek' : 'Tjocklek' })]);
        const i1 = H('input', { type: 'text', value: (it.type === 'point' ? it.value : it.thickness) || '' });
        i1.addEventListener('input', () => { if (it.type === 'point') it.value = i1.value; else it.thickness = i1.value; draw(); });
        i1.addEventListener('change', () => State.commit('egenskap'));
        f1.appendChild(i1); host.appendChild(f1);

        const f2 = H('label', { class: 'fld' }, [H('span', { text: 'Beskrivning' })]);
        const i2 = H('textarea'); i2.value = it.note || '';
        i2.addEventListener('input', () => { it.note = i2.value; });
        i2.addEventListener('change', () => State.commit('egenskap'));
        f2.appendChild(i2); host.appendChild(f2);
      }
      const del = H('button', { class: 'btn ghost', text: 'Ta bort', style: 'width:100%;color:#ff6b6b' });
      del.onclick = () => { State.remove(it.id); selId = null; draw(); };
      host.appendChild(del);
    } else {
      host.appendChild(H('p', { class: 'empty', text: 'Välj ett verktyg ovan och rita in anmärkningen direkt på plåten. Måtten räknas automatiskt från plåtkanterna.' }));
    }

    /* lista över anmärkningar på plåten */
    const mine = nearItems().filter(x => {
      const c = Item.pos(x);
      return pointInPoly(c, plate.poly);
    });
    if (mine.length) {
      list.appendChild(H('p', { class: 'empty', text: 'På denna plåt:' }));
      mine.forEach(x => {
        const row = H('div', { class: 'orow' + (x.id === selId ? ' on' : '') });
        row.appendChild(H('span', { class: 'swatch', style: 'background:' + (x.type === 'repair' || x.type === 'point' ? RED : INK2) }));
        row.appendChild(H('span', { class: 'nm', text: (nums[x.id] ? nums[x.id] + ' · ' : '') + (x.type === 'repair' ? 'Reparation' : x.type === 'doubler' ? 'Dubbling' : x.type === 'point' ? 'Mätpunkt' : x.type) }));
        row.appendChild(H('span', { class: 'meta', text: x.value || x.thickness || '' }));
        row.onclick = () => { selId = x.id; State.select(x.id); draw(); };
        list.appendChild(row);
      });
    }
  }

  return { init: init, open: open, close: close, get isOpen() { return open_; } };
})();
