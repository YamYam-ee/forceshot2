/* ==========================================================================
   geometry.js – genererar plåtindelningen för cisternbottnen
   Modellkoordinater = millimeter. Cisterncentrum = (0,0). y växer nedåt.
   Vyvridningen (doc.view.rot) påverkar INTE modellen – bara hur den ritas.
   ========================================================================== */
'use strict';

const Geo = (function () {

  /* -------- grundläggande -------- */
  function halfW(R, y) { const v = R * R - y * y; return v > 0 ? Math.sqrt(v) : 0; }

  /** Halva kordan för ett radband (bredaste stället i bandet) */
  function bandHalf(Ri, y0, y1) {
    const yNear = (y0 <= 0 && y1 >= 0) ? 0 : (Math.abs(y0) < Math.abs(y1) ? y0 : y1);
    return halfW(Ri, yNear);
  }

  /** Skarvpositioner (x) för en rad givet fas p och plåtlängd L */
  function seamsFor(p, L, c) {
    const out = [];
    let k = Math.floor((-c - p) / L);
    let s = p + k * L;
    while (s > -c + 1e-6) { k--; s = p + k * L; }
    for (let guard = 0; guard < 600; guard++) {
      out.push(s);
      if (s >= c - 1e-6) break;
      k++; s = p + k * L;
    }
    return out;
  }

  /** Plåtsegment i en rad, med hopslagning av smala kantplåtar */
  function segsFor(p, L, c, merge) {
    const s = seamsFor(p, L, c);
    let segs = [];
    for (let i = 0; i < s.length - 1; i++) segs.push({ a: s[i], b: s[i + 1] });
    const vis = g => Math.max(0, Math.min(g.b, c) - Math.max(g.a, -c));
    segs = segs.filter(g => vis(g) > L * 0.02);
    if (!segs.length) segs = [{ a: -c, b: c }];
    if (merge && segs.length > 1 && vis(segs[0]) < L * 0.3) { segs[1].a = segs[0].a; segs.shift(); }
    if (merge && segs.length > 1 && vis(segs[segs.length - 1]) < L * 0.3) {
      segs[segs.length - 2].b = segs[segs.length - 1].b; segs.pop();
    }
    return segs;
  }

  /** Fas som ger önskat antal plåtar med standardlängd, så symmetriskt som möjligt */
  function solvePhase(c, L, target, merge) {
    let best = null;
    const N = 240;
    for (let i = 0; i < N; i++) {
      const p = -c + (i / N) * L;
      const segs = segsFor(p, L, c, merge);
      const n = segs.length;
      const vis = g => Math.max(0, Math.min(g.b, c) - Math.max(g.a, -c));
      const asym = Math.abs(vis(segs[0]) - vis(segs[n - 1]));
      const score = Math.abs(n - target) * 1e7 + asym;
      if (!best || score < best.score) best = { p: p, n: n, score: score };
    }
    return best;
  }

  /** Jämn delning av raden i n lika breda plåtar (när standardlängden inte går ihop) */
  function evenSegs(c, n) {
    const w = (2 * c) / n, out = [];
    for (let k = 0; k < n; k++) out.push({ a: -c + k * w, b: -c + (k + 1) * w });
    return out;
  }

  /* -------- polygonverktyg -------- */
  function circlePoly(R, n) {
    const out = [];
    for (let i = 0; i < n; i++) { const a = i / n * TAU; out.push({ x: Math.cos(a) * R, y: Math.sin(a) * R }); }
    return out;
  }

  function clipConvex(subject, clip) {
    let out = subject;
    for (let i = 0; i < clip.length && out.length; i++) {
      const A = clip[i], B = clip[(i + 1) % clip.length];
      const ex = B.x - A.x, ey = B.y - A.y;
      const input = out; out = [];
      for (let j = 0; j < input.length; j++) {
        const P = input[j], Q = input[(j + 1) % input.length];
        const sp = ex * (P.y - A.y) - ey * (P.x - A.x);
        const sq = ex * (Q.y - A.y) - ey * (Q.x - A.x);
        if (sp >= 0) out.push(P);
        if ((sp >= 0) !== (sq >= 0)) {
          const t = sp / (sp - sq);
          out.push({ x: P.x + (Q.x - P.x) * t, y: P.y + (Q.y - P.y) * t });
        }
      }
    }
    return out;
  }

  /** Delar en polygon i två med den oändliga linjen genom a–b */
  function cutPoly(poly, a, b) {
    const dx = b.x - a.x, dy = b.y - a.y;
    const side = p => dx * (p.y - a.y) - dy * (p.x - a.x);
    const lft = [], rgt = [];
    for (let i = 0; i < poly.length; i++) {
      const P = poly[i], Q = poly[(i + 1) % poly.length];
      const sp = side(P), sq = side(Q);
      if (sp >= 0) lft.push(P);
      if (sp <= 0) rgt.push(P);
      if ((sp > 0 && sq < 0) || (sp < 0 && sq > 0)) {
        const t = sp / (sp - sq);
        const I = { x: P.x + (Q.x - P.x) * t, y: P.y + (Q.y - P.y) * t };
        lft.push(I); rgt.push(I);
      }
    }
    return [lft, rgt];
  }

  /** Den del av linjen a–b som ligger inne i polygonen (den ritade skarven) */
  function lineInPoly(poly, a, b) {
    const dx = b.x - a.x, dy = b.y - a.y;
    const len = Math.hypot(dx, dy) || 1;
    const ux = dx / len, uy = dy / len;
    const side = p => dx * (p.y - a.y) - dy * (p.x - a.x);
    const hits = [];
    for (let i = 0; i < poly.length; i++) {
      const P = poly[i], Q = poly[(i + 1) % poly.length];
      const sp = side(P), sq = side(Q);
      if ((sp > 0 && sq < 0) || (sp < 0 && sq > 0)) {
        const t = sp / (sp - sq);
        hits.push({ x: P.x + (Q.x - P.x) * t, y: P.y + (Q.y - P.y) * t });
      } else if (Math.abs(sp) < 1e-9) hits.push({ x: P.x, y: P.y });
    }
    if (hits.length < 2) return null;
    let lo = hits[0], hi = hits[0], tlo = Infinity, thi = -Infinity;
    hits.forEach(h => {
      const t = (h.x - a.x) * ux + (h.y - a.y) * uy;
      if (t < tlo) { tlo = t; lo = h; }
      if (t > thi) { thi = t; hi = h; }
    });
    if (thi - tlo < 1) return null;
    return { a: lo, b: hi };
  }

  /* -------- huvudbygge -------- */
  function build(doc) {
    const R = Math.max(200, doc.tank.diameter / 2);
    const ringOn = !!doc.ring.on;
    const ringCount = ringOn ? clamp(doc.ring.count | 0, 0, 400) : 0;
    /* 0 randplåtar = randen ligger kvar, men i ett obrutet stycke utan skarvar och beteckning */
    const ringWhole = ringOn && ringCount === 0;
    const ringW = ringOn ? clamp(doc.ring.width, 50, R * 0.45) : 0;
    const Ri = R - ringW;

    const rot = doc.tank.orientation === 'v' ? -Math.PI / 2 : 0;
    const cs = Math.cos(rot), sn = Math.sin(rot);
    const TX = p => ({ x: p.x * cs - p.y * sn, y: p.x * sn + p.y * cs });
    const IX = p => ({ x: p.x * cs + p.y * sn, y: -p.x * sn + p.y * cs });

    const sizeMode = doc.layout.mode === 'size';
    /* i mått-läget har plåten en bestämd längd – då får kantbitar inte slås ihop */
    const merge = !sizeMode && doc.layout.mergeSlivers !== false;
    const plateW = clamp(doc.layout.plateW || 1500, 100, 2 * Ri);
    const plateL = clamp(doc.layout.plateL || 3000, 100, 8 * Ri);
    const rowY = doc.layout.rowY;

    /* radband: handjusterade, från plåtbredd, eller jämnt fördelade på antal rader */
    const bandDefs = [];
    if (Array.isArray(rowY) && rowY.length) {
      const bs = rowY.slice().sort((a, b) => a - b).map(v => clamp(v, -Ri + 1, Ri - 1));
      let prev = -Ri;
      bs.forEach(v => { bandDefs.push({ y0: prev, y1: v }); prev = v; });
      bandDefs.push({ y0: prev, y1: Ri });
    } else if (sizeMode) {
      const align = doc.layout.rowAlign === 'seam' ? 0 : plateW / 2;
      const k0 = Math.floor((-Ri - align) / plateW), k1 = Math.ceil((Ri - align) / plateW);
      for (let k = k0; k <= k1 && bandDefs.length < 120; k++) {
        const y0 = align + k * plateW, y1 = y0 + plateW;
        if (y1 <= -Ri + 1 || y0 >= Ri - 1) continue;
        bandDefs.push({ y0: y0, y1: y1 });
      }
      if (!bandDefs.length) bandDefs.push({ y0: -Ri, y1: Ri });
    } else {
      const n = clamp(doc.layout.rows | 0, 1, 80);
      const h = 2 * Ri / n;
      for (let i = 0; i < n; i++) bandDefs.push({ y0: -Ri + i * h, y1: -Ri + (i + 1) * h });
    }

    const N = bandDefs.length;
    const bounds = bandDefs.slice(0, -1).map(b => b.y1);

    /* numreringen går att vända: bokstäverna från andra änden, siffrorna från andra sidan */
    const flipRows = !!(doc.view && doc.view.flipRows);
    const flipCols = !!(doc.view && doc.view.flipCols);
    const rowRef = i => alpha(flipRows ? N - 1 - i : i);
    const colRef = (i, n) => flipCols ? n - i : i + 1;

    const bands = bandDefs.map((b, i) => ({
      i: i, ref: rowRef(i), y0: b.y0, y1: b.y1, h: b.y1 - b.y0, c: bandHalf(Ri, b.y0, b.y1)
    }));

    let midIdx = Math.floor((N - 1) / 2);
    if (sizeMode || (Array.isArray(rowY) && rowY.length)) {
      const k = bands.findIndex(b => b.y0 <= 0 && b.y1 > 0);
      if (k >= 0) midIdx = k;
    }
    const bandH = sizeMode ? plateW : (2 * Ri / N);

    /* plåtlängd: given i mått-läget, annars satt av mittraden */
    const cMid = bands[midIdx].c || Ri;
    const L = sizeMode ? plateL : Math.max(50, (2 * cMid) / clamp(doc.layout.mid | 0, 1, 80));

    /* i mått-läget läggs skarvarna symmetriskt kring centrum */
    let basePhase = -cMid;
    if (sizeMode) {
      const c0 = bands[midIdx].c || Ri;
      const nSeam = segsFor(0, L, c0, merge).length;
      const nMid = segsFor(L / 2, L, c0, merge).length;
      basePhase = nMid <= nSeam ? L / 2 : 0;
    }

    const stag = clamp(doc.layout.stagger, 0, 0.5);
    const overrides = doc.layout.rowOverrides || {};
    const notes = [];

    const manual = doc.layout.rowSeams || {};

    bands.forEach(b => {
      if (b.c > 0 && manual[b.i]) {
        /* skarvarna är justerade för hand i den här raden */
        const xs = manual[b.i].filter(x => x > -b.c + 1 && x < b.c - 1).slice().sort((p, q) => p - q);
        b.segs = [];
        let prev = -b.c;
        xs.forEach(x => { b.segs.push({ a: prev, b: x }); prev = x; });
        b.segs.push({ a: prev, b: b.c });
        b.even = false; b.manual = true;
        return;
      }
      let target = null;
      if (overrides[b.i] != null) target = overrides[b.i] | 0;
      else if (!sizeMode) {
        if (b.i === 0 && N > 1) target = doc.layout.first | 0;
        else if (b.i === N - 1 && N > 1) target = doc.layout.last | 0;
        else if (b.i === midIdx) target = doc.layout.mid | 0;
      }

      if (b.c <= 0) { b.segs = []; b.even = false; return; }

      if (target != null) {
        const n = Math.max(1, target);
        const sol = solvePhase(b.c, L, n, merge);
        if (sol.n === n) {
          /* antalet går ihop med standardplåtens längd – behåll förbandet */
          b.segs = segsFor(sol.p, L, b.c, merge);
          b.even = false;
        } else {
          /* går inte ihop – dela raden jämnt, plåtarna blir smalare */
          b.segs = evenSegs(b.c, n);
          b.even = true;
          b.plateW = (2 * b.c) / n;
          notes.push({ row: b.i, ref: rowRef(b.i), n: n, w: b.plateW });
        }
      } else {
        const phase = basePhase + (b.i - midIdx) * stag * L;
        b.segs = segsFor(phase, L, b.c, merge);
        b.even = false;
      }
    });

    /* plåtar */
    const cp = circlePoly(Ri, 512);
    const rows = [];
    bands.forEach(b => {
      const row = {
        i: b.i, label: rowRef(b.i), y0: b.y0, y1: b.y1, h: b.h,
        even: !!b.even, manual: !!b.manual, plateW: b.plateW, plates: []
      };
      (b.segs || []).forEach(g => {
        const rect = { x: g.a, y: b.y0, w: g.b - g.a, h: b.y1 - b.y0 };
        const corners = rectPoly(rect);
        const allIn = corners.every(p => p.x * p.x + p.y * p.y <= Ri * Ri);
        const poly = allIn ? corners : clipConvex(corners, cp);
        if (poly.length < 3) return;
        const area = polyArea(poly);
        if (area < (rect.w * bandH) * 0.02) return;
        row.plates.push(mkPlate(b.i, rect, poly, area, allIn, TX, IX));
      });
      row.base = row.plates.length;
      rows.push(row);
    });

    /* randplåtar */
    const ring = [];
    if (ringOn) {
      const base = (doc.ring.rot || 0) * D2R + rot;
      if (ringWhole) {
        /* hela randen i ett stycke: en obruten ring utan skarvar */
        const whole = mkRing(base, base + TAU, Ri, R);
        whole.whole = true;
        ring.push(whole);
      } else {
        const n = ringCount;
        const step = TAU / n;
        const man = doc.ring.angles;
        const offs = [];
        if (Array.isArray(man) && man.length === n) man.slice().sort((a, b) => a - b).forEach(v => offs.push(v));
        else for (let i = 0; i < n; i++) offs.push(i * step);
        for (let i = 0; i < n; i++) {
          const a0 = base + offs[i];
          const a1 = base + (i === n - 1 ? offs[0] + TAU : offs[i + 1]);
          ring.push(mkRing(a0, a1, Ri, R));
        }
      }
    }
    const ringBase = ring.length;

    /* bågar hålls kvar på sin plats i ringen: hela bredden, ytterkanten eller innerkanten */
    const edgeW = R * 0.013;
    (doc.items || []).forEach(it => {
      const sh = it.shape;
      if (!sh || sh.kind !== 'arc') return;
      if (sh.band === 'edge') { sh.r0 = R - edgeW / 2; sh.r1 = R + edgeW / 2; }
      else if (sh.band === 'inner') { sh.r0 = Ri - edgeW / 2; sh.r1 = Ri + edgeW / 2; }
      else { sh.r0 = Ri; sh.r1 = R; }
    });

    /* klyvningar delar plåtarna på riktigt */
    const splitSegs = {};
    (doc.items || []).filter(i => i.type === 'split').forEach(sp => {
      applySplit(sp, rows, ring, splitSegs, TX, IX);
    });

    /* numrering efter delning */
    rows.forEach(r => {
      r.plates.sort((a, b) => a.lx - b.lx);
      const nc = r.plates.length;
      r.plates.forEach((p, i) => { p.col = i; p.id = p.label = r.label + colRef(i, nc); });
    });
    ring.sort((a, b) => a.mid - b.mid);
    ring.forEach((p, i) => { p.i = i; p.id = p.label = p.whole ? 'R' : 'R' + (i + 1); });

    const plates = [];
    rows.forEach(r => r.plates.forEach(p => plates.push(p)));

    /* skarvlinjer */
    const seams = [];
    const addSeam = (a, b, type, extra) => {
      if (dist(a, b) < 1) return;
      seams.push(Object.assign({ a: TX(a), b: TX(b), type: type }, extra || {}));
    };
    for (let i = 1; i < N; i++) {
      const y = bounds[i - 1], hw = halfW(Ri, y);
      if (hw > 1) addSeam({ x: -hw, y: y }, { x: hw, y: y }, 'row', { bound: i - 1, ly: y });
    }
    bands.forEach(b => {
      (b.segs || []).forEach((g, j) => {
        if (!j) return;
        const x = g.a;
        if (Math.abs(x) >= Ri) return;
        const yl = halfW(Ri, x);
        const ya = Math.max(b.y0, -yl), yb = Math.min(b.y1, yl);
        if (yb - ya > 1) addSeam({ x: x, y: ya }, { x: x, y: yb }, 'joint', { row: b.i, idx: j - 1, lx: x });
      });
    });
    ring.forEach((r, i) => {
      if (r.whole) return;
      seams.push({
        a: { x: Math.cos(r.a0) * r.r0, y: Math.sin(r.a0) * r.r0 },
        b: { x: Math.cos(r.a0) * r.r1, y: Math.sin(r.a0) * r.r1 },
        type: 'ring', ringIdx: i, la: r.a0
      });
    });

    const snapSegs = seams.slice();
    Object.keys(splitSegs).forEach(k => snapSegs.push(splitSegs[k]));

    /* ritningsblad (A3-liggande proportion) */
    const padLbl = R * 0.11;
    const mt = R * 0.06, mb = R * 0.22, ml = R * 0.045, mr = R * 0.045, gap = R * 0.06;
    const shH = 2 * R + 2 * padLbl + mt + mb;
    const shW = shH * 1.414;
    const rcW = shW * 0.265;
    const availW = shW - ml - mr - rcW - gap;
    const sx = -(ml + availW / 2);
    const sy = -(R + padLbl + mt);
    const sheet = {
      x: sx, y: sy, w: shW, h: shH,
      pad: R * 0.028,
      col: { x: sx + shW - mr - rcW, y: sy + R * 0.075, w: rcW, h: shH - R * 0.15 },
      area: { x: sx + ml, y: sy + R * 0.05, w: availW, h: shH - R * 0.10 }
    };

    const base = R / 1200;
    const lw = {
      base: base, seam: base * 1.05, outline: base * 2.2, ring: base * 1.15,
      doubler: base * 2.3, repair: base * 1.9, thin: base * 0.7,
      hatch: base * 0.75, hatchGap: R * 0.014
    };
    const fs = {
      ref: R * 0.036, note: R * 0.030, title: R * 0.032,
      titleBig: R * 0.052, tiny: R * 0.022
    };

    return {
      R: R, Ri: Ri, ringW: ringW, ringOn: ringOn, ringWhole: ringWhole,
      flipRows: flipRows, flipCols: flipCols,
      rot: rot, TX: TX, IX: IX,
      vrot: (doc.view && doc.view.rot) || 0,
      L: L, bandH: bandH, N: N, midIdx: midIdx, sizeMode: sizeMode,
      bands: bands, bounds: bounds, rows: rows, plates: plates, ring: ring, ringBase: ringBase,
      seams: seams, snapSegs: snapSegs, splitSegs: splitSegs,
      sheet: sheet, lw: lw, fs: fs, notes: notes, padLbl: padLbl
    };
  }

  /* -------- plåtobjekt -------- */
  function mkPlate(rowIdx, rect, poly, area, allIn, TX, IX) {
    const tpoly = poly.map(TX);
    const c = polyCentroid(tpoly);
    return {
      kind: 'plate', row: rowIdx, col: 0, id: '', label: '',
      rect: rect, quad: rectPoly(rect).map(TX), poly: tpoly, area: area,
      org: TX({ x: rect.x, y: rect.y }), ux: TX({ x: 1, y: 0 }), vy: TX({ x: 0, y: 1 }),
      w: rect.w, h: rect.h, c: c, lx: IX(c).x, clipped: !allIn
    };
  }

  function mkRing(a0, a1, r0, r1) {
    const pts = [];
    const STEPS = Math.max(3, Math.ceil((a1 - a0) / 0.03));
    for (let k = 0; k <= STEPS; k++) { const a = lerp(a0, a1, k / STEPS); pts.push({ x: Math.cos(a) * r0, y: Math.sin(a) * r0 }); }
    for (let k = STEPS; k >= 0; k--) { const a = lerp(a0, a1, k / STEPS); pts.push({ x: Math.cos(a) * r1, y: Math.sin(a) * r1 }); }
    const mid = (a0 + a1) / 2, rm = (r0 + r1) / 2;
    return {
      kind: 'ring', i: 0, id: '', label: '',
      a0: a0, a1: a1, r0: r0, r1: r1, mid: mid,
      poly: pts, c: { x: Math.cos(mid) * rm, y: Math.sin(mid) * rm }
    };
  }

  /** delplåt av en kluven plåt */
  function mkPiece(host, poly, TX, IX) {
    const c = polyCentroid(poly);
    if (host.kind === 'ring') {
      let a0 = Infinity, a1 = -Infinity, r0 = Infinity, r1 = -Infinity;
      const ref = host.mid;
      poly.forEach(p => {
        let a = Math.atan2(p.y, p.x);
        while (a - ref > Math.PI) a -= TAU;
        while (ref - a > Math.PI) a += TAU;
        const r = Math.hypot(p.x, p.y);
        a0 = Math.min(a0, a); a1 = Math.max(a1, a);
        r0 = Math.min(r0, r); r1 = Math.max(r1, r);
      });
      return {
        kind: 'ring', i: 0, id: '', label: '',
        a0: a0, a1: a1, r0: r0, r1: r1, mid: (a0 + a1) / 2,
        poly: poly, c: c, split: true
      };
    }
    const local = poly.map(IX);
    const lb = polyBBox(local);
    const rect = { x: lb.x, y: lb.y, w: lb.w, h: lb.h };
    return {
      kind: 'plate', row: host.row, col: 0, id: '', label: '',
      rect: rect, quad: rectPoly(rect).map(TX), poly: poly, area: polyArea(poly),
      org: TX({ x: rect.x, y: rect.y }), ux: host.ux, vy: host.vy,
      w: rect.w, h: rect.h, c: c, lx: IX(c).x, clipped: true, split: true
    };
  }

  function applySplit(sp, rows, ring, splitSegs, TX, IX) {
    const mid = { x: (sp.a.x + sp.b.x) / 2, y: (sp.a.y + sp.b.y) / 2 };
    let list = null, idx = -1;
    for (const r of rows) {
      const i = r.plates.findIndex(p => pointInPoly(mid, p.poly));
      if (i >= 0) { list = r.plates; idx = i; break; }
    }
    if (!list) {
      const i = ring.findIndex(p => pointInPoly(mid, p.poly));
      if (i >= 0) { list = ring; idx = i; }
    }
    if (!list) return;
    const host = list[idx];
    const seg = lineInPoly(host.poly, sp.a, sp.b);
    if (seg) splitSegs[sp.id] = seg;
    const parts = cutPoly(host.poly, sp.a, sp.b).filter(p => p.length >= 3);
    if (parts.length !== 2) return;
    const aH = polyArea(host.poly);
    if (polyArea(parts[0]) < aH * 0.02 || polyArea(parts[1]) < aH * 0.02) return;
    list.splice(idx, 1, mkPiece(host, parts[0], TX, IX), mkPiece(host, parts[1], TX, IX));
  }

  /* -------- hjälp till verktyg -------- */
  function plateAt(geo, p) {
    for (const pl of geo.plates) if (pointInPoly(p, pl.poly)) return pl;
    for (const r of geo.ring) if (pointInPoly(p, r.poly)) return r;
    return null;
  }

  function toLocal(pl, p) {
    if (pl.kind === 'ring') {
      const a = Math.atan2(p.y, p.x), rr = Math.hypot(p.x, p.y);
      let da = a - pl.a0; while (da < -Math.PI) da += TAU; while (da > Math.PI) da -= TAU;
      return { u: da * ((pl.r0 + pl.r1) / 2), v: rr - pl.r0 };
    }
    const dx = p.x - pl.org.x, dy = p.y - pl.org.y;
    return { u: dx * pl.ux.x + dy * pl.ux.y, v: dx * pl.vy.x + dy * pl.vy.y };
  }
  function toModel(pl, l) {
    if (pl.kind === 'ring') {
      const rm = (pl.r0 + pl.r1) / 2;
      const a = pl.a0 + l.u / rm, rr = pl.r0 + l.v;
      return { x: Math.cos(a) * rr, y: Math.sin(a) * rr };
    }
    return {
      x: pl.org.x + l.u * pl.ux.x + l.v * pl.vy.x,
      y: pl.org.y + l.u * pl.ux.y + l.v * pl.vy.y
    };
  }

  function insideTank(geo, p, pad) { return Math.hypot(p.x, p.y) <= geo.R + (pad || 0); }

  return {
    build: build, plateAt: plateAt, toLocal: toLocal, toModel: toModel,
    insideTank: insideTank, halfW: halfW, clipConvex: clipConvex,
    circlePoly: circlePoly, cutPoly: cutPoly, lineInPoly: lineInPoly
  };
})();
