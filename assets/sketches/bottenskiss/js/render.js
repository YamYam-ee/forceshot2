/* ==========================================================================
   render.js – ritar hela ritningen (blad, cistern, plåtar, objekt)
   Bladet ligger stilla; cistern + objekt ligger i en vridbar grupp (vy-vridning).
   ========================================================================== */
'use strict';

const INK = '#12161c';
const INK2 = '#3a4350';
const RED = '#d92d20';
const BLUE = '#1f6feb';
const FONT = 'Inter, "Segoe UI", Arial, Helvetica, sans-serif';

/* ---------- objektgeometri (delas av ritning och verktyg) ---------- */
const Item = {
  /** klyvningens verkliga sträckning (klippt mot plåten) */
  seg(it) {
    const g = State.geo;
    const s = g && g.splitSegs ? g.splitSegs[it.id] : null;
    return s || { a: it.a, b: it.b };
  },
  pos(it) {
    const geo = State.geo;
    if (it.type === 'manhole' || it.type === 'nozzle') {
      if (it.free) return { x: it.x, y: it.y };
      return { x: Math.cos(it.angle) * geo.R, y: Math.sin(it.angle) * geo.R };
    }
    if (it.type === 'point' || it.type === 'text') return { x: it.x, y: it.y };
    if (it.type === 'split') { const s = Item.seg(it); return { x: (s.a.x + s.b.x) / 2, y: (s.a.y + s.b.y) / 2 }; }
    if (it.type === 'sump') return { x: it.x + it.w / 2, y: it.y + it.h / 2 };
    if (it.shape) return shapeCenter(it.shape);
    return { x: 0, y: 0 };
  },
  bbox(it) {
    const geo = State.geo;
    if (it.type === 'split') {
      const s = Item.seg(it);
      return { x: Math.min(s.a.x, s.b.x), y: Math.min(s.a.y, s.b.y), w: Math.abs(s.a.x - s.b.x), h: Math.abs(s.a.y - s.b.y) };
    }
    if (it.type === 'sump') return { x: it.x, y: it.y, w: it.w, h: it.h };
    if (it.shape) return shapeBBox(it.shape);
    const p = Item.pos(it), r = geo ? geo.R * 0.03 : 10;
    return { x: p.x - r, y: p.y - r, w: r * 2, h: r * 2 };
  },
  hit(it, p, tol) {
    if (it.type === 'split') { const s = Item.seg(it); return closestOnSeg(p, s.a, s.b).d <= tol; }
    if (it.type === 'sump') return pointInRect(p, it, tol);
    if (it.shape) {
      if (shapeHit(it.shape, p, tol)) return true;
      /* nummerlappen hör till objektet */
      if (it.type === 'repair' && dist(p, Item.labelPos(it)) <= tol * 2.4) return true;
      return false;
    }
    if (it.type === 'point' || it.type === 'text') {
      const a = Item.pos(it);
      const lp = { x: a.x + (it.lx || 0), y: a.y + (it.ly || 0) };
      return dist(p, a) <= tol * 1.4 || dist(p, lp) <= tol * 2.2;
    }
    if (it.type === 'manhole' || it.type === 'nozzle') {
      const a = Item.pos(it);
      const rr = it.type === 'manhole' ? (State.geo.R * 0.055) : Math.max(it.dn / 2, State.geo.R * 0.018);
      return dist(p, a) <= rr + tol;
    }
    return false;
  },
  /** var etikettlappen hamnar (samma regel som ritningen använder) */
  labelPos(it) {
    const geo = State.geo;
    const c = Item.pos(it);
    let lx = it.lx, ly = it.ly;
    if (lx === undefined || ly === undefined) {
      if (it.type === 'point') { lx = geo.R * 0.07; ly = -geo.R * 0.05; }
      else if (it.type === 'text') { lx = geo.R * 0.09; ly = -geo.R * 0.06; }
      else if (it.shape && it.shape.kind === 'arc') {
        const a = (it.shape.a0 + it.shape.a1) / 2;
        const band = it.shape.band;
        const off = (band === 'edge' || band === 'inner') ? geo.R * 0.085 : geo.R * 0.14;
        lx = Math.cos(a) * off; ly = Math.sin(a) * off;
      } else {
        const b = Item.bbox(it);
        lx = 0; ly = -(b.h / 2 + geo.R * 0.05);
      }
    }
    return { x: c.x + lx, y: c.y + ly };
  },
  move(it, dx, dy) {
    if (it.type === 'split') { it.a.x += dx; it.a.y += dy; it.b.x += dx; it.b.y += dy; return; }
    if (it.type === 'sump') { it.x += dx; it.y += dy; return; }
    if (it.shape) { moveShape(it.shape, dx, dy); return; }
    if (it.type === 'point' || it.type === 'text') { it.x += dx; it.y += dy; return; }
    if (it.type === 'manhole' || it.type === 'nozzle') {
      const p = Item.pos(it);
      const q = { x: p.x + dx, y: p.y + dy };
      if (it.free) { it.x = q.x; it.y = q.y; }
      else it.angle = Math.atan2(q.y, q.x);
    }
  }
};

const Render = (function () {
  let svg, world, gSheet, gRot, gTank, gItems, gOverlay;
  let view = { tx: 0, ty: 0, s: 1 };
  let maskN = 0;

  /* ================= grund ================= */
  function init(node) {
    svg = node;
    svg.innerHTML = '';
    world = S('g', { id: 'world' });
    gSheet = S('g', { id: 'lyr-sheet' });
    gRot = S('g', { id: 'lyr-rot' });
    gTank = S('g', { id: 'lyr-tank' });
    gItems = S('g', { id: 'lyr-items' });
    gOverlay = S('g', { id: 'lyr-overlay' });
    gRot.appendChild(gTank); gRot.appendChild(gItems); gRot.appendChild(gOverlay);
    world.appendChild(gSheet); world.appendChild(gRot);
    svg.appendChild(world);
    applyView();
  }

  const vrot = () => (State.doc && State.doc.view.rot) || 0;

  function applyView() {
    world.setAttribute('transform', 'translate(' + view.tx + ' ' + view.ty + ') scale(' + view.s + ')');
    gRot.setAttribute('transform', 'rotate(' + vrot() + ')');
  }
  function screenSize(px) { return px / view.s; }

  /** skärm -> modellkoordinater (vy-vridningen borträknad) */
  function toModel(clientX, clientY) {
    const p = toSheet(clientX, clientY);
    const a = -vrot() * D2R, c = Math.cos(a), s = Math.sin(a);
    return { x: p.x * c - p.y * s, y: p.x * s + p.y * c };
  }
  /** skärm -> bladkoordinater (utan vy-vridning) */
  function toSheet(clientX, clientY) {
    const r = svg.getBoundingClientRect();
    return { x: (clientX - r.left - view.tx) / view.s, y: (clientY - r.top - view.ty) / view.s };
  }
  function modelToSheet(p) {
    const a = vrot() * D2R, c = Math.cos(a), s = Math.sin(a);
    return { x: p.x * c - p.y * s, y: p.x * s + p.y * c };
  }

  function fit(pad) {
    const geo = State.geo; if (!geo) return;
    const r = svg.getBoundingClientRect();
    const sh = State.doc.view.sheet ? geo.sheet : { x: -geo.R * 1.2, y: -geo.R * 1.2, w: geo.R * 2.4, h: geo.R * 2.4 };
    const m = pad === undefined ? 28 : pad;
    const s = Math.min((r.width - m * 2) / sh.w, (r.height - m * 2 - 22) / sh.h);
    view.s = s;
    view.tx = r.width / 2 - (sh.x + sh.w / 2) * s;
    view.ty = (r.height - 18) / 2 - (sh.y + sh.h / 2) * s;
    applyView();
    drawOverlayRefresh();
  }
  function zoomAt(factor, cx, cy) {
    const r = svg.getBoundingClientRect();
    const mx = cx === undefined ? r.width / 2 : cx - r.left;
    const my = cy === undefined ? r.height / 2 : cy - r.top;
    const before = { x: (mx - view.tx) / view.s, y: (my - view.ty) / view.s };
    view.s = clamp(view.s * factor, 0.002, 40);
    view.tx = mx - before.x * view.s;
    view.ty = my - before.y * view.s;
    applyView(); drawOverlayRefresh();
  }
  function pan(dx, dy) { view.tx += dx; view.ty += dy; applyView(); }

  /** centrera vyn på en modellpunkt */
  function centerOn(p) {
    const r = svg.getBoundingClientRect();
    const q = modelToSheet(p);
    view.tx = r.width / 2 - q.x * view.s;
    view.ty = (r.height - 20) / 2 - q.y * view.s;
    applyView(); drawOverlayRefresh();
  }

  /** transform som håller texten upprätt trots vy-vridning */
  function upright(geo, x, y) {
    return geo.vrot ? 'rotate(' + r2(-geo.vrot) + ' ' + r2(x) + ' ' + r2(y) + ')' : null;
  }

  /* ================= defs ================= */
  function buildDefs(geo) {
    const d = S('defs');
    const g = geo.lw.hatchGap;
    d.appendChild(S('pattern', { id: 'hatch-d', patternUnits: 'userSpaceOnUse', width: g, height: g, patternTransform: 'rotate(45)' }, [
      S('path', { d: 'M0 0V' + g, stroke: '#1c222b', 'stroke-width': geo.lw.hatch, opacity: .62 }),
      S('path', { d: 'M' + g + ' 0V' + g, stroke: '#1c222b', 'stroke-width': geo.lw.hatch, opacity: .62 })
    ]));
    d.appendChild(S('pattern', { id: 'hatch-r', patternUnits: 'userSpaceOnUse', width: g * .85, height: g * .85, patternTransform: 'rotate(-45)' }, [
      S('path', { d: 'M0 0V' + (g * .85), stroke: RED, 'stroke-width': geo.lw.hatch * .9, opacity: .5 })
    ]));
    d.appendChild(S('pattern', { id: 'hatch-s', patternUnits: 'userSpaceOnUse', width: g * .7, height: g * .7, patternTransform: 'rotate(45)' }, [
      S('path', { d: 'M0 0V' + (g * .7), stroke: '#28303a', 'stroke-width': geo.lw.hatch * .8, opacity: .35 })
    ]));
    d.appendChild(S('marker', { id: 'arw', viewBox: '0 0 10 10', refX: 9, refY: 5, markerWidth: 7, markerHeight: 7, orient: 'auto-start-reverse' },
      [S('path', { d: 'M0 0.6 L9.4 5 L0 9.4 z', fill: INK })]));
    return d;
  }

  /* ---------- ritningsblad (roteras inte) ---------- */
  function buildSheet(geo, doc) {
    const sh = geo.sheet, R = geo.R, g = S('g', { id: 'sheet' });
    g.appendChild(S('rect', { x: sh.x, y: sh.y, width: sh.w, height: sh.h, fill: '#ffffff' }));
    g.appendChild(S('rect', {
      x: sh.x + sh.pad, y: sh.y + sh.pad, width: sh.w - sh.pad * 2, height: sh.h - sh.pad * 2,
      fill: 'none', stroke: INK, 'stroke-width': geo.lw.base * 1.5
    }));

    const col = sh.col;
    const titleH = R * 0.46, legH = R * 0.31;
    const tX = col.x, tW = col.w;
    const titleY = col.y + col.h - titleH;
    const legY = titleY - legH - R * 0.05;

    /* namnruta */
    const tb = S('g');
    tb.appendChild(S('rect', { x: tX, y: titleY, width: tW, height: titleH, fill: 'none', stroke: INK, 'stroke-width': geo.lw.base * 1.5 }));
    const rowH = titleH / 4;
    for (let i = 1; i < 4; i++) tb.appendChild(S('line', { x1: tX, y1: titleY + rowH * i, x2: tX + tW, y2: titleY + rowH * i, stroke: INK, 'stroke-width': geo.lw.base * .8 }));
    tb.appendChild(S('line', { x1: tX + tW * .42, y1: titleY + rowH, x2: tX + tW * .42, y2: titleY + titleH, stroke: INK, 'stroke-width': geo.lw.base * .8 }));

    const cellTxt = (x, y, lab, val, big) => {
      const gg = S('g');
      gg.appendChild(S('text', {
        x: x + R * 0.022, y: y + R * 0.038, 'font-family': FONT, 'font-size': geo.fs.tiny * .82,
        fill: '#8a94a3', 'letter-spacing': R * 0.0016
      }, lab));
      gg.appendChild(S('text', {
        x: x + R * 0.022, y: y + rowH - R * 0.028, 'font-family': FONT,
        'font-size': big ? geo.fs.titleBig : geo.fs.title, 'font-weight': big ? 700 : 500, fill: INK
      }, val || '–'));
      return gg;
    };
    const m = doc.meta;
    tb.appendChild(cellTxt(tX, titleY, 'OBJEKT', m.objekt || '—', true));
    tb.appendChild(cellTxt(tX, titleY + rowH, 'CISTERN / TANK', m.cistern || '—'));
    tb.appendChild(cellTxt(tX + tW * .42, titleY + rowH, 'BOTTENPLÅT', m.tjocklek || '—'));
    tb.appendChild(cellTxt(tX, titleY + rowH * 2, 'RITAD AV', m.ritad || '—'));
    tb.appendChild(cellTxt(tX + tW * .42, titleY + rowH * 2, 'DATUM', m.datum || '—'));
    tb.appendChild(cellTxt(tX, titleY + rowH * 3, 'DIAMETER', doc.view.dimDia ? 'Ø ' + mm(geo.R * 2) + ' mm' : '—'));
    tb.appendChild(cellTxt(tX + tW * .42, titleY + rowH * 3, 'PLÅTMÅTT', doc.view.dimPlate ? mm(geo.L) + ' × ' + mm(geo.bandH) + ' mm' : '—'));
    g.appendChild(tb);

    /* teckenförklaring */
    const lg = S('g');
    lg.appendChild(S('rect', { x: tX, y: legY, width: tW, height: legH, fill: 'none', stroke: INK, 'stroke-width': geo.lw.base * 1.2 }));
    lg.appendChild(S('text', { x: tX + R * 0.022, y: legY + R * 0.045, 'font-family': FONT, 'font-size': geo.fs.tiny * .82, fill: '#8a94a3', 'letter-spacing': R * 0.0016 }, 'TECKENFÖRKLARING'));
    const keys = [
      { t: 'Dubblingsplåt', kind: 'd' },
      { t: 'Reparation / anmärkning', kind: 'r' },
      { t: 'Godstjocklek (mätpunkt)', kind: 'p' },
      { t: 'Mannlucka / anslutning', kind: 'm' }
    ];
    keys.forEach((k, i) => {
      const y = legY + R * 0.075 + i * (R * 0.052);
      const bx = tX + R * 0.026, bw = R * 0.075, bh = R * 0.036;
      if (k.kind === 'd') lg.appendChild(S('rect', { x: bx, y: y, width: bw, height: bh, fill: 'url(#hatch-d)', stroke: INK, 'stroke-width': geo.lw.base * 1.4 }));
      else if (k.kind === 'r') lg.appendChild(S('rect', { x: bx, y: y, width: bw, height: bh, fill: 'url(#hatch-r)', stroke: RED, 'stroke-width': geo.lw.base * 1.2 }));
      else if (k.kind === 'p') {
        lg.appendChild(S('circle', { cx: bx + bw * .35, cy: y + bh / 2, r: bh * .34, fill: RED }));
        lg.appendChild(S('line', { x1: bx + bw * .45, y1: y + bh * .38, x2: bx + bw, y2: y, stroke: RED, 'stroke-width': geo.lw.base * .8 }));
      } else lg.appendChild(S('path', {
        d: 'M' + bx + ' ' + (y + bh) + 'h' + bw + 'M' + (bx + bw * .28) + ' ' + (y + bh) + 'v' + (-bh * .8) + 'h' + (bw * .44) + 'v' + (bh * .8),
        fill: 'none', stroke: INK, 'stroke-width': geo.lw.base * 1.3
      }));
      lg.appendChild(S('text', { x: bx + bw + R * 0.026, y: y + bh * .78, 'font-family': FONT, 'font-size': geo.fs.tiny, fill: INK }, k.t));
    });
    g.appendChild(lg);

    /* anmärkningstabell */
    if (doc.view.table) {
      const rows = reportRows().slice(0, 60);
      const tabY = col.y, tabH = legY - R * 0.05 - tabY;
      const gt = S('g');
      gt.appendChild(S('rect', { x: tX, y: tabY, width: tW, height: tabH, fill: 'none', stroke: INK, 'stroke-width': geo.lw.base * 1.2 }));
      const hH = R * 0.05;
      gt.appendChild(S('line', { x1: tX, y1: tabY + hH, x2: tX + tW, y2: tabY + hH, stroke: INK, 'stroke-width': geo.lw.base * 1.2 }));
      const cols = [
        { t: 'NR', w: .09 }, { t: 'TYP', w: .20 }, { t: 'PLÅT', w: .13 },
        { t: 'MÅTT', w: .22 }, { t: 'TJ.', w: .11 }, { t: 'ANMÄRKNING', w: .25 }
      ];
      let cx = tX;
      const colX = [];
      cols.forEach((c, i) => {
        colX.push(cx);
        if (i) gt.appendChild(S('line', { x1: cx, y1: tabY, x2: cx, y2: tabY + tabH, stroke: INK, 'stroke-width': geo.lw.base * .55, opacity: .5 }));
        gt.appendChild(S('text', { x: cx + R * 0.012, y: tabY + hH * .68, 'font-family': FONT, 'font-size': geo.fs.tiny * .8, fill: '#8a94a3' }, c.t));
        cx += tW * c.w;
      });
      const maxRows = Math.floor((tabH - hH) / (R * 0.042));
      const rh = Math.min(R * 0.042, (tabH - hH) / Math.max(1, Math.min(rows.length, maxRows)));
      rows.slice(0, maxRows).forEach((r, i) => {
        const y = tabY + hH + rh * (i + 1) - rh * 0.28;
        [r.n, r.typ, r.plat, r.matt, r.tj, r.anm].forEach((v, ci) => {
          if (!v) return;
          const maxc = Math.floor((tW * cols[ci].w) / (geo.fs.tiny * 0.52));
          const s = String(v).length > maxc ? String(v).slice(0, maxc - 1) + '…' : String(v);
          gt.appendChild(S('text', {
            x: colX[ci] + R * 0.012, y: y, 'font-family': FONT, 'font-size': geo.fs.tiny * .92,
            fill: ci === 0 ? RED : INK, 'font-weight': ci === 0 ? 700 : 400
          }, s));
        });
        if (i) gt.appendChild(S('line', { x1: tX, y1: tabY + hH + rh * i, x2: tX + tW, y2: tabY + hH + rh * i, stroke: INK, 'stroke-width': geo.lw.base * .4, opacity: .35 }));
      });
      if (rows.length > maxRows) gt.appendChild(S('text', {
        x: tX + tW - R * 0.02, y: tabY + tabH - R * 0.016, 'text-anchor': 'end',
        'font-family': FONT, 'font-size': geo.fs.tiny * .85, fill: '#8a94a3'
      }, '+ ' + (rows.length - maxRows) + ' till – se bilaga'));
      gt.appendChild(S('text', { x: tX + R * 0.012, y: tabY - R * 0.018, 'font-family': FONT, 'font-size': geo.fs.tiny * .85, fill: '#8a94a3', 'letter-spacing': R * 0.0014 }, 'ANMÄRKNINGAR'));
      g.appendChild(gt);
    }

    /* norrpil – flyttbar */
    if (doc.view.north) {
      const np = northPos(geo, doc);
      const ang = (doc.view.northRot || 0) + geo.vrot;
      const nn = S('g', { id: 'north', transform: 'translate(' + r2(np.x) + ' ' + r2(np.y) + ') rotate(' + r2(ang) + ')' });
      const s2 = R * 0.075;
      nn.appendChild(S('circle', { cx: 0, cy: 0, r: s2 * 1.25, fill: 'none', stroke: INK2, 'stroke-width': geo.lw.thin * .8, opacity: .45 }));
      nn.appendChild(S('path', { d: 'M0 ' + (-s2) + 'L' + (s2 * .42) + ' ' + (s2 * .8) + 'L0 ' + (s2 * .42) + 'L' + (-s2 * .42) + ' ' + (s2 * .8) + 'Z', fill: INK }));
      nn.appendChild(S('text', {
        x: 0, y: -s2 - R * 0.022, 'text-anchor': 'middle', 'font-family': FONT,
        'font-size': geo.fs.note, 'font-weight': 700, fill: INK,
        transform: ang ? 'rotate(' + r2(-ang) + ' 0 ' + r2(-s2 - R * 0.022) + ')' : null
      }, 'N'));
      g.appendChild(nn);
    }
    return g;
  }

  function northPos(geo, doc) {
    if (doc.view.northPos) return doc.view.northPos;
    const sh = geo.sheet;
    return { x: sh.x + sh.pad + geo.R * 0.13, y: sh.y + sh.pad + geo.R * 0.17 };
  }

  /* ---------- cistern + plåtar (roteras) ---------- */
  function buildTank(geo, doc) {
    const g = S('g', { id: 'tank' });
    const R = geo.R, Ri = geo.Ri;

    g.appendChild(S('circle', { cx: 0, cy: 0, r: R, fill: '#ffffff' }));

    const cl = R * 1.09;
    const dash = (R * 0.05) + ' ' + (R * 0.018) + ' ' + (R * 0.006) + ' ' + (R * 0.018);
    g.appendChild(S('g', { stroke: INK2, 'stroke-width': geo.lw.thin, 'stroke-dasharray': dash, opacity: .75 }, [
      S('line', { x1: -cl, y1: 0, x2: cl, y2: 0 }),
      S('line', { x1: 0, y1: -cl, x2: 0, y2: cl })
    ]));

    let dSeam = '';
    geo.seams.forEach(s => { dSeam += 'M' + r2(s.a.x) + ' ' + r2(s.a.y) + 'L' + r2(s.b.x) + ' ' + r2(s.b.y); });
    g.appendChild(S('path', { d: dSeam, fill: 'none', stroke: INK, 'stroke-width': geo.lw.seam, 'stroke-linecap': 'square' }));

    if (geo.ringOn) g.appendChild(S('circle', { cx: 0, cy: 0, r: Ri, fill: 'none', stroke: INK, 'stroke-width': geo.lw.seam * 1.15 }));
    g.appendChild(S('circle', { cx: 0, cy: 0, r: R, fill: 'none', stroke: INK, 'stroke-width': geo.lw.outline }));

    /* plåtetiketter */
    if (doc.view.ids) {
      const lab = S('g', { 'font-family': FONT, fill: '#5a6472', 'text-anchor': 'middle' });
      geo.plates.forEach(p => {
        const b = polyBBox(p.poly);
        const fs = Math.min(p.h * 0.24, p.w * 0.30, geo.L * 0.15, R * 0.05);
        if (Math.min(b.w, b.h) < fs * 1.9) return;
        lab.appendChild(S('text', {
          x: r2(p.c.x), y: r2(p.c.y + fs * .35), 'font-size': r2(fs),
          transform: upright(geo, p.c.x, p.c.y + fs * .35)
        }, p.label));
      });
      geo.ring.forEach(rp => {
        if (rp.whole) return;
        const fs = Math.min(geo.ringW * 0.30, R * 0.032);
        const deg = rp.mid * R2D;
        const scr = deg + geo.vrot;
        const flip = ((scr % 360) + 360) % 360 > 90 && ((scr % 360) + 360) % 360 < 270;
        lab.appendChild(S('text', {
          x: 0, y: 0, 'font-size': r2(fs),
          transform: 'translate(' + r2(rp.c.x) + ' ' + r2(rp.c.y) + ') rotate(' + r2(deg + (flip ? 180 : 0)) + ') translate(0 ' + r2(fs * .35) + ')'
        }, rp.label));
      });
      g.appendChild(lab);
    }

    /* rad-/kolumnreferenser */
    if (doc.view.refs) {
      const rg = S('g', { 'font-family': FONT, 'font-size': r2(geo.fs.ref), fill: INK2, 'text-anchor': 'middle', 'font-weight': 600 });
      const off = R + geo.padLbl * 0.52;
      geo.bands.forEach(b => {
        if (b.c <= 0) return;
        const yc = (b.y0 + b.y1) / 2;
        [-off, off].forEach(x => {
          const p = geo.TX({ x: x, y: yc });
          rg.appendChild(S('text', { x: r2(p.x), y: r2(p.y + geo.fs.ref * .35), transform: upright(geo, p.x, p.y + geo.fs.ref * .35) }, b.ref || alpha(b.i)));
        });
      });
      const midB = geo.bands[geo.midIdx];
      const midSegs = midB.segs || [];
      midSegs.forEach((s, i) => {
        const xc = (Math.max(s.a, -midB.c) + Math.min(s.b, midB.c)) / 2;
        const no = geo.flipCols ? midSegs.length - i : i + 1;
        [off, -off].forEach(yy => {
          const p = geo.TX({ x: xc, y: yy });
          rg.appendChild(S('text', { x: r2(p.x), y: r2(p.y + geo.fs.ref * .35), transform: upright(geo, p.x, p.y + geo.fs.ref * .35) }, String(no)));
        });
      });
      g.appendChild(rg);
    }

    return g;
  }

  /** måttsättning – ligger i bladlagret så den inte snedställs av vy-vridningen */
  function buildDims(geo, doc) {
    const g = S('g', { id: 'dims' });
    const R = geo.R;
    if (doc.view.dimDia) {
      const y = R + geo.padLbl * 1.35;
      const dg = S('g', { stroke: INK, 'stroke-width': geo.lw.thin, fill: 'none' });
      dg.appendChild(S('line', { x1: -R, y1: y, x2: R, y2: y, 'marker-start': 'url(#arw)', 'marker-end': 'url(#arw)' }));
      dg.appendChild(S('line', { x1: -R, y1: R * 0.35, x2: -R, y2: y + R * 0.03, opacity: .55 }));
      dg.appendChild(S('line', { x1: R, y1: R * 0.35, x2: R, y2: y + R * 0.03, opacity: .55 }));
      g.appendChild(dg);
      g.appendChild(S('text', {
        x: 0, y: y - R * 0.022, 'text-anchor': 'middle', 'font-family': FONT,
        'font-size': r2(geo.fs.note), fill: INK, stroke: '#fff', 'stroke-width': geo.fs.note * .28,
        'paint-order': 'stroke'
      }, 'Ø ' + mm(R * 2)));
    }
    if (doc.view.dimPlate) {
      const y2 = R + geo.padLbl * 1.35 + R * 0.075;
      const info = ['Plåt ' + mm(geo.L) + ' × ' + mm(geo.bandH) + ' mm', geo.N + ' rader · ' + geo.plates.length + ' plåtar'];
      if (geo.ringOn) info.push(geo.ringWhole
        ? 'Randplåt odelad · b=' + mm(geo.ringW) + ' mm'
        : 'Randplåt ' + geo.ring.length + ' st · b=' + mm(geo.ringW) + ' mm');
      g.appendChild(S('text', {
        x: 0, y: y2, 'text-anchor': 'middle', 'font-family': FONT,
        'font-size': r2(geo.fs.tiny), fill: '#66707e'
      }, info.join('   ·   ')));
    }
    return g;
  }

  /* ================= objektlager ================= */
  function buildItems(geo, doc) {
    const g = S('g', { id: 'items' });
    const localDefs = S('defs');
    g.appendChild(localDefs);
    const num = State.numbers();

    /* dubblingar, sammanslagna per grupp */
    State.groups().forEach((grp, gi) => {
      const d = grp.shapes.map(s => shapePath(s)).join(' ');
      const bb = shapesBBox(grp.shapes);
      const pad = geo.lw.doubler * 3;
      const mid = 'dm' + (++maskN);
      localDefs.appendChild(S('mask', { id: mid, maskUnits: 'userSpaceOnUse', x: bb.x - pad, y: bb.y - pad, width: bb.w + pad * 2, height: bb.h + pad * 2 }, [
        S('rect', { x: bb.x - pad, y: bb.y - pad, width: bb.w + pad * 2, height: bb.h + pad * 2, fill: '#fff' }),
        S('path', { d: d, 'fill-rule': 'nonzero', fill: '#000' })
      ]));
      const gg = S('g');
      gg.appendChild(S('path', { d: d, 'fill-rule': 'nonzero', fill: 'url(#hatch-d)', stroke: 'none' }));
      gg.appendChild(S('path', { d: d, 'fill-rule': 'nonzero', fill: 'none', stroke: INK, 'stroke-width': geo.lw.doubler * 2, mask: 'url(#' + mid + ')', 'stroke-linejoin': 'round' }));
      if (!grp.items.every(i => i.hideDims)) {
        let big = grp.shapes[0], ba = shapeArea(grp.shapes[0]);
        grp.shapes.forEach(s => { const a = shapeArea(s); if (a > ba) { ba = a; big = s; } });
        const c = shapeCenter(big);
        const fs = Math.min(geo.R * 0.045, Math.sqrt(ba) * 0.34);
        gg.appendChild(S('text', {
          x: r2(c.x), y: r2(c.y + fs * .35), 'text-anchor': 'middle', 'font-family': FONT,
          'font-size': r2(fs), 'font-weight': 700, fill: INK, stroke: '#fff',
          'stroke-width': fs * .3, 'paint-order': 'stroke', transform: upright(geo, c.x, c.y + fs * .35)
        }, 'D' + (gi + 1)));
      }
      g.appendChild(gg);
    });

    /* pottor */
    doc.items.filter(i => i.type === 'sump').forEach(it => {
      const gg = S('g');
      const inset = Math.min(it.w, it.h) * 0.14;
      const cx = it.x + it.w / 2, cy = it.y + it.h / 2;
      if (it.shape !== 'rect') {
        gg.appendChild(S('ellipse', { cx: cx, cy: cy, rx: it.w / 2, ry: it.h / 2, fill: 'url(#hatch-s)', stroke: INK, 'stroke-width': geo.lw.doubler }));
        gg.appendChild(S('ellipse', { cx: cx, cy: cy, rx: Math.max(1, it.w / 2 - inset), ry: Math.max(1, it.h / 2 - inset), fill: 'none', stroke: INK, 'stroke-width': geo.lw.seam }));
      } else {
        gg.appendChild(S('rect', { x: it.x, y: it.y, width: it.w, height: it.h, fill: 'url(#hatch-s)', stroke: INK, 'stroke-width': geo.lw.doubler }));
        gg.appendChild(S('rect', { x: it.x + inset, y: it.y + inset, width: Math.max(1, it.w - inset * 2), height: Math.max(1, it.h - inset * 2), fill: 'none', stroke: INK, 'stroke-width': geo.lw.seam }));
        gg.appendChild(S('path', {
          d: 'M' + it.x + ' ' + it.y + 'l' + inset + ' ' + inset +
            'M' + (it.x + it.w) + ' ' + it.y + 'l' + (-inset) + ' ' + inset +
            'M' + it.x + ' ' + (it.y + it.h) + 'l' + inset + ' ' + (-inset) +
            'M' + (it.x + it.w) + ' ' + (it.y + it.h) + 'l' + (-inset) + ' ' + (-inset),
          stroke: INK, 'stroke-width': geo.lw.seam, fill: 'none'
        }));
      }
      if (!it.hideDims) {
        const fs = Math.min(geo.R * 0.038, Math.min(it.w, it.h) * 0.3);
        const ly = it.y + it.h + fs * 1.25;
        gg.appendChild(S('text', {
          x: r2(cx), y: r2(ly), 'text-anchor': 'middle', 'font-family': FONT, 'font-size': r2(fs),
          'font-weight': 600, fill: INK, stroke: '#fff', 'stroke-width': fs * .3, 'paint-order': 'stroke',
          transform: upright(geo, cx, ly)
        }, (num[it.id] || 'P') + (it.note ? ' ' + it.note : '')));
      }
      g.appendChild(gg);
    });

    /* reparationer */
    doc.items.filter(i => i.type === 'repair').forEach(it => {
      const gg = S('g');
      const onEdge = it.shape.kind === 'arc' && (it.shape.band === 'edge' || it.shape.band === 'inner');
      if (onEdge) {
        gg.appendChild(S('path', { d: shapePath(it.shape), fill: RED, stroke: RED, 'stroke-width': geo.lw.thin }));
        const rm = (it.shape.r0 + it.shape.r1) / 2, tk = geo.R * 0.020;
        [it.shape.a0, it.shape.a1].forEach(a => gg.appendChild(S('line', {
          x1: r2(Math.cos(a) * (rm - tk)), y1: r2(Math.sin(a) * (rm - tk)),
          x2: r2(Math.cos(a) * (rm + tk)), y2: r2(Math.sin(a) * (rm + tk)),
          stroke: RED, 'stroke-width': geo.lw.repair
        })));
      } else
      gg.appendChild(S('path', { d: shapePath(it.shape), fill: 'url(#hatch-r)', stroke: RED, 'stroke-width': geo.lw.repair, 'stroke-linejoin': 'round' }));
      if (!it.hideDims) {
        const b = shapeBBox(it.shape);
        const isArc = it.shape.kind === 'arc';
        const fs = isArc ? geo.R * 0.042 : Math.min(geo.R * 0.042, Math.max(b.w, b.h) * 0.34, geo.R * 0.05);
        const n = num[it.id] || '?';
        const aMid = isArc ? (it.shape.a0 + it.shape.a1) / 2 : 0;
        const off = onEdge ? geo.R * 0.085 : geo.R * 0.14;
        const lx = it.lx === undefined ? (isArc ? Math.cos(aMid) * off : 0) : it.lx;
        const ly = it.ly === undefined ? (isArc ? Math.sin(aMid) * off : -(b.h / 2 + fs * 1.1)) : it.ly;
        const c = shapeCenter(it.shape);
        const px = c.x + lx, py = c.y + ly;
        if (Math.hypot(lx, ly) > fs) gg.appendChild(S('line', { x1: c.x, y1: c.y, x2: px, y2: py, stroke: RED, 'stroke-width': geo.lw.thin, opacity: .8 }));
        const bw = fs * (0.72 + String(n).length * 0.42), bh = fs * 1.18;
        const lab = S('g', { transform: upright(geo, px, py) });
        lab.appendChild(S('rect', { x: r2(px - bw / 2), y: r2(py - bh / 2), width: r2(bw), height: r2(bh), rx: bh * .18, fill: '#fff', stroke: RED, 'stroke-width': geo.lw.thin * 1.5 }));
        lab.appendChild(S('text', { x: r2(px), y: r2(py + fs * .34), 'text-anchor': 'middle', 'font-family': FONT, 'font-size': r2(fs), 'font-weight': 700, fill: RED }, n));
        if (it.thickness) lab.appendChild(S('text', {
          x: r2(px + bw * .75), y: r2(py + fs * .34), 'font-family': FONT, 'font-size': r2(fs * .92),
          fill: RED, stroke: '#fff', 'stroke-width': fs * .28, 'paint-order': 'stroke'
        }, it.thickness));
        gg.appendChild(lab);
      }
      g.appendChild(gg);
    });

    /* mätpunkter */
    doc.items.filter(i => i.type === 'point').forEach(it => {
      const gg = S('g');
      const r = geo.R * 0.011;
      gg.appendChild(S('circle', { cx: it.x, cy: it.y, r: r, fill: RED }));
      if (!it.hideDims) {
        const lx = it.lx === undefined ? geo.R * 0.07 : it.lx;
        const ly = it.ly === undefined ? -geo.R * 0.05 : it.ly;
        const fs = geo.fs.note * 1.15;
        gg.appendChild(S('line', { x1: it.x, y1: it.y, x2: it.x + lx, y2: it.y + ly, stroke: RED, 'stroke-width': geo.lw.thin * 1.2 }));
        const tx = it.x + lx + (lx < 0 ? -fs * .2 : fs * .2), ty = it.y + ly + fs * .34;
        gg.appendChild(S('text', {
          x: r2(tx), y: r2(ty), 'text-anchor': lx < 0 ? 'end' : 'start',
          'font-family': FONT, 'font-size': r2(fs), 'font-weight': 600, fill: RED,
          stroke: '#fff', 'stroke-width': fs * .3, 'paint-order': 'stroke', transform: upright(geo, tx, ty)
        }, it.value || ''));
      }
      g.appendChild(gg);
    });

    /* klyvningar – ritas som riktiga plåtskarvar */
    doc.items.filter(i => i.type === 'split').forEach(it => {
      const s = Item.seg(it);
      g.appendChild(S('line', { x1: r2(s.a.x), y1: r2(s.a.y), x2: r2(s.b.x), y2: r2(s.b.y), stroke: INK, 'stroke-width': geo.lw.seam, 'stroke-linecap': 'square' }));
    });

    /* mannluckor & anslutningar */
    doc.items.filter(i => i.type === 'manhole' || i.type === 'nozzle').forEach(it => {
      g.appendChild(it.type === 'manhole' ? drawManhole(geo, it, num[it.id]) : drawNozzle(geo, it, num[it.id]));
    });

    /* texter */
    doc.items.filter(i => i.type === 'text').forEach(it => {
      const gg = S('g');
      const fs = geo.fs.note;
      const lx = it.lx || geo.R * 0.09, ly = it.ly || -geo.R * 0.06;
      gg.appendChild(S('line', { x1: it.x, y1: it.y, x2: it.x + lx, y2: it.y + ly, stroke: INK2, 'stroke-width': geo.lw.thin }));
      gg.appendChild(S('circle', { cx: it.x, cy: it.y, r: geo.R * 0.006, fill: INK2 }));
      const tx = it.x + lx + (lx < 0 ? -fs * .2 : fs * .2), ty = it.y + ly;
      const blk = S('g', { transform: upright(geo, tx, ty) });
      String(it.text || '').split('\n').forEach((ln, i) => {
        blk.appendChild(S('text', {
          x: r2(tx), y: r2(ty + fs * (0.34 + i * 1.15)),
          'text-anchor': lx < 0 ? 'end' : 'start', 'font-family': FONT, 'font-size': r2(fs), fill: INK,
          stroke: '#fff', 'stroke-width': fs * .3, 'paint-order': 'stroke'
        }, ln));
      });
      gg.appendChild(blk);
      g.appendChild(gg);
    });

    return g;
  }

  function drawManhole(geo, it, label) {
    const R = geo.R, a = it.angle;
    const halfA = (it.dn || 600) / 2 / R * 1.9;
    const out = R * 0.075, inn = R * 0.02;
    const gg = S('g');
    const pt = (rad, ang) => ({ x: Math.cos(ang) * rad, y: Math.sin(ang) * rad });
    const a0 = a - halfA, a1 = a + halfA;
    const p1 = pt(R - inn, a0), p2 = pt(R + out, a0), p3 = pt(R + out, a1), p4 = pt(R - inn, a1);
    const d = 'M' + r2(p1.x) + ' ' + r2(p1.y) + 'L' + r2(p2.x) + ' ' + r2(p2.y) +
      'A' + (R + out) + ' ' + (R + out) + ' 0 0 1 ' + r2(p3.x) + ' ' + r2(p3.y) +
      'L' + r2(p4.x) + ' ' + r2(p4.y) +
      'A' + (R - inn) + ' ' + (R - inn) + ' 0 0 0 ' + r2(p1.x) + ' ' + r2(p1.y) + 'Z';
    gg.appendChild(S('path', { d: d, fill: '#fff', stroke: INK, 'stroke-width': geo.lw.outline }));
    gg.appendChild(S('line', {
      x1: r2(Math.cos(a) * (R - inn)), y1: r2(Math.sin(a) * (R - inn)),
      x2: r2(Math.cos(a) * (R + out)), y2: r2(Math.sin(a) * (R + out)),
      stroke: INK2, 'stroke-width': geo.lw.thin, 'stroke-dasharray': (R * .012) + ' ' + (R * .008)
    }));
    const lp = pt(R + out * 1.30, a);
    gg.appendChild(tangentLabel(geo, lp, a, geo.fs.note, 700,
      (label || 'MH') + (it.hideDims ? '' : (it.dn ? ' DN' + it.dn : '')) + (it.note ? '  ' + it.note : '')));
    return gg;
  }

  function drawNozzle(geo, it, label) {
    const R = geo.R;
    const p = Item.pos(it);
    const r = Math.max((it.dn || 150) / 2, R * 0.016);
    const gg = S('g');
    gg.appendChild(S('circle', { cx: r2(p.x), cy: r2(p.y), r: r2(r), fill: '#fff', stroke: INK, 'stroke-width': geo.lw.outline * .85 }));
    gg.appendChild(S('path', {
      d: 'M' + r2(p.x - r) + ' ' + r2(p.y) + 'h' + r2(r * 2) + 'M' + r2(p.x) + ' ' + r2(p.y - r) + 'v' + r2(r * 2),
      stroke: INK2, 'stroke-width': geo.lw.thin
    }));
    const a = it.free ? Math.atan2(p.y, p.x) : it.angle;
    const lp = { x: p.x + Math.cos(a) * r * 1.9, y: p.y + Math.sin(a) * r * 1.9 };
    gg.appendChild(tangentLabel(geo, lp, a, geo.fs.note * .95, 600,
      (label || 'S') + (it.hideDims ? '' : (it.dn ? ' DN' + it.dn : '')) + (it.note ? '  ' + it.note : '')));
    return gg;
  }

  /** text som följer cisternkanten och alltid är läsbar */
  function tangentLabel(geo, at, ang, fs, weight, txt) {
    let deg = ang * R2D + 90;
    let scr = ((deg + geo.vrot) % 360 + 360) % 360;
    if (scr > 90 && scr < 270) deg += 180;
    return S('text', {
      x: 0, y: 0, 'font-family': FONT, 'font-size': r2(fs), 'font-weight': weight, fill: INK,
      'text-anchor': 'middle', stroke: '#fff', 'stroke-width': fs * .32, 'paint-order': 'stroke',
      transform: 'translate(' + r2(at.x) + ' ' + r2(at.y) + ') rotate(' + r2(deg) + ') translate(0 ' + r2(fs * .36) + ')'
    }, txt);
  }

  /* ================= överlagring ================= */
  let overlayExtra = null;
  function setOverlayExtra(node) { overlayExtra = node; drawOverlayRefresh(); }

  function drawOverlayRefresh() {
    if (!State.geo) return;
    while (gOverlay.firstChild) gOverlay.removeChild(gOverlay.firstChild);
    const hs = screenSize(4.5);

    State.sel.forEach(id => {
      const it = State.byId(id); if (!it) return;
      const g = S('g', { class: 'sel' });
      if (it.type === 'split') {
        const s = Item.seg(it);
        g.appendChild(S('line', { x1: s.a.x, y1: s.a.y, x2: s.b.x, y2: s.b.y, stroke: BLUE, 'stroke-width': screenSize(2.5), opacity: .8 }));
        [s.a, s.b].forEach(p => g.appendChild(handle(p.x, p.y, hs)));
      } else if (it.shape && it.shape.kind === 'arc') {
        g.appendChild(S('path', {
          d: shapePath(it.shape), fill: 'none', stroke: BLUE, 'stroke-width': screenSize(1.8),
          'stroke-dasharray': screenSize(6) + ' ' + screenSize(4)
        }));
        handlePoints(it).forEach(h => g.appendChild(handle(h.x, h.y, hs)));
      } else {
        const b = Item.bbox(it);
        const pad = screenSize(3);
        g.appendChild(S('rect', {
          x: b.x - pad, y: b.y - pad, width: b.w + pad * 2, height: b.h + pad * 2,
          fill: 'none', stroke: BLUE, 'stroke-width': screenSize(1.2),
          'stroke-dasharray': screenSize(5) + ' ' + screenSize(4)
        }));
        if (resizable(it)) handlePoints(it).forEach(h => g.appendChild(handle(h.x, h.y, hs)));
      }
      gOverlay.appendChild(g);
    });

    if (overlayExtra) gOverlay.appendChild(overlayExtra);
  }
  function handle(x, y, s) {
    return S('rect', { x: x - s, y: y - s, width: s * 2, height: s * 2, fill: '#fff', stroke: BLUE, 'stroke-width': s * .45 });
  }
  function resizable(it) { return it.type === 'sump' || (it.shape && typeof it.shape === 'object'); }
  function handlePoints(it) {
    if (!resizable(it)) return [];
    const sh = it.shape;
    if (sh && sh.kind === 'arc') {
      const rm = (sh.r0 + sh.r1) / 2;
      return [
        { x: Math.cos(sh.a0) * rm, y: Math.sin(sh.a0) * rm, arcEnd: 0 },
        { x: Math.cos(sh.a1) * rm, y: Math.sin(sh.a1) * rm, arcEnd: 1 }
      ];
    }
    const b = Item.bbox(it);
    const hx = [b.x, b.x + b.w / 2, b.x + b.w], hy = [b.y, b.y + b.h / 2, b.y + b.h];
    const out = [];
    for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) {
      if (i === 1 && j === 1) continue;
      out.push({ x: hx[i], y: hy[j], hx: i, hy: j });
    }
    return out;
  }

  /* ================= tecknande ================= */
  function drawSheet() {
    const geo = State.geo, doc = State.doc;
    while (gSheet.firstChild) gSheet.removeChild(gSheet.firstChild);
    gSheet.appendChild(buildDefs(geo));
    if (doc.view.sheet) gSheet.appendChild(buildSheet(geo, doc));
    gSheet.appendChild(buildDims(geo, doc));
  }
  function drawStatic() {
    const geo = State.geo, doc = State.doc;
    drawSheet();
    while (gTank.firstChild) gTank.removeChild(gTank.firstChild);
    gTank.appendChild(buildTank(geo, doc));
    applyView();
  }
  function drawItems() {
    const geo = State.geo, doc = State.doc;
    while (gItems.firstChild) gItems.removeChild(gItems.firstChild);
    gItems.appendChild(buildItems(geo, doc));
    drawOverlayRefresh();
  }
  function drawAll() { drawStatic(); drawItems(); }

  /** fristående SVG för export/utskrift */
  function exportSVG(scale) {
    const geo = State.geo, doc = State.doc;
    const sh = doc.view.sheet ? geo.sheet : { x: -geo.R * 1.25, y: -geo.R * 1.25, w: geo.R * 2.5, h: geo.R * 2.5 };
    const px = 1600 * (scale || 1);
    const out = S('svg', {
      xmlns: SVGNS, version: '1.1',
      viewBox: r2(sh.x) + ' ' + r2(sh.y) + ' ' + r2(sh.w) + ' ' + r2(sh.h),
      width: Math.round(px), height: Math.round(px * sh.h / sh.w)
    });
    out.appendChild(S('rect', { x: sh.x, y: sh.y, width: sh.w, height: sh.h, fill: '#ffffff' }));
    out.appendChild(buildDefs(geo));
    if (doc.view.sheet) out.appendChild(buildSheet(geo, doc));
    out.appendChild(buildDims(geo, doc));
    const rg = S('g', { transform: 'rotate(' + (doc.view.rot || 0) + ')' });
    rg.appendChild(buildTank(geo, doc));
    rg.appendChild(buildItems(geo, doc));
    out.appendChild(rg);
    return out;
  }

  return {
    init: init, fit: fit, zoomAt: zoomAt, pan: pan, applyView: applyView, centerOn: centerOn,
    toModel: toModel, toSheet: toSheet, modelToSheet: modelToSheet, screenSize: screenSize,
    drawStatic: drawStatic, drawSheet: drawSheet, drawItems: drawItems, drawAll: drawAll,
    drawOverlayRefresh: drawOverlayRefresh, setOverlayExtra: setOverlayExtra,
    handlePoints: handlePoints, resizable: resizable, northPos: northPos,
    exportSVG: exportSVG,
    __drawManhole: drawManhole, __drawNozzle: drawNozzle,
    get view() { return view; },
    get svg() { return svg; }
  };
})();

/** vilka randplåtar en båge täcker, t.ex. "R4–R6" */
function ringSpan(s) {
  const geo = State.geo;
  if (!geo || !geo.ring.length) return 'Randplåt';
  const span = s.a1 - s.a0;
  const inArc = a => { let d = a - s.a0; while (d < 0) d += TAU; while (d >= TAU) d -= TAU; return d <= span; };
  const hits = geo.ring.filter(r => inArc(r.mid));
  if (!hits.length) {
    const mid = (s.a0 + s.a1) / 2, rm = (s.r0 + s.r1) / 2;
    const pl = geo.ring.find(r => pointInPoly({ x: Math.cos(mid) * rm, y: Math.sin(mid) * rm }, r.poly));
    return pl ? pl.label : 'Randplåt';
  }
  return hits.length === 1 ? hits[0].label : hits[0].label + '–' + hits[hits.length - 1].label;
}

/* ---------- rapportrader (tabell på ritning, sidopanel, CSV) ---------- */
function reportRows() {
  const doc = State.doc, geo = State.geo;
  if (!geo) return [];
  const num = State.numbers();
  const rows = [];
  const plateOf = p => { const pl = Geo.plateAt(geo, p); return pl ? pl.label : '–'; };
  /* etiketten läses ur geometrin, så tabellen följer numreringen även när den vänds */
  const plateOr = (p, stored) => { const pl = Geo.plateAt(geo, p); return pl ? pl.label : (stored || '–'); };

  doc.items.filter(i => i.type === 'repair').sort((a, b) => a.seq - b.seq).forEach(it => {
    const b = shapeBBox(it.shape), c = shapeCenter(it.shape);
    rows.push({
      id: it.id, n: num[it.id], typ: it.kindName || 'Reparation',
      plat: it.shape.kind === 'arc' ? ringSpan(it.shape) : plateOr(c, it.plateId),
      matt: it.hideDims ? '' : (it.shape.kind === 'arc' ? mm(arcLen(it.shape)) + ' båg'
        : it.shape.kind === 'circle' ? 'Ø' + mm(it.shape.r * 2) : mm(b.w) + '×' + mm(b.h)),
      tj: it.hideDims ? '' : (it.thickness || ''), anm: it.note || ''
    });
  });
  State.groups().forEach((grp, gi) => {
    const b = shapesBBox(grp.shapes);
    const c = shapeCenter(grp.shapes[0]);
    const hide = grp.items.every(i => i.hideDims);
    const notes = grp.items.map(i => i.note).filter(Boolean).join('; ');
    const th = grp.items.map(i => i.thickness).filter(Boolean)[0] || '';
    rows.push({
      id: grp.ids[0], n: 'D' + (gi + 1), typ: 'Dubbling', plat: plateOf(c),
      matt: hide ? '' : mm(b.w) + '×' + mm(b.h), tj: hide ? '' : th, anm: notes
    });
  });
  doc.items.filter(i => i.type === 'point').sort((a, b) => a.seq - b.seq).forEach(it => {
    rows.push({
      id: it.id, n: '•', typ: 'Mätpunkt', plat: plateOf({ x: it.x, y: it.y }),
      matt: '', tj: it.hideDims ? '' : (it.value || ''), anm: it.note || ''
    });
  });
  doc.items.filter(i => i.type === 'sump').sort((a, b) => a.seq - b.seq).forEach(it => {
    rows.push({
      id: it.id, n: num[it.id], typ: 'Potta', plat: plateOf(Item.pos(it)),
      matt: it.hideDims ? '' : (it.shape === 'rect' ? mm(it.w) + '×' + mm(it.h) : 'Ø' + mm((it.w + it.h) / 2)),
      tj: it.hideDims ? '' : (it.depth ? mm(it.depth) : ''), anm: it.note || ''
    });
  });
  doc.items.filter(i => i.type === 'manhole' || i.type === 'nozzle').sort((a, b) => a.seq - b.seq).forEach(it => {
    rows.push({
      id: it.id, n: num[it.id], typ: it.type === 'manhole' ? 'Mannlucka' : 'Anslutning',
      plat: '', matt: it.hideDims ? '' : (it.dn ? 'DN' + it.dn : ''), tj: '', anm: it.note || ''
    });
  });
  return rows;
}
