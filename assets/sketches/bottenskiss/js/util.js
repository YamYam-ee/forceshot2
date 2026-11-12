/* ==========================================================================
   util.js – små hjälpare
   ========================================================================== */
'use strict';

const SVGNS = 'http://www.w3.org/2000/svg';

function $(sel, root) { return (root || document).querySelector(sel); }
function $$(sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); }

/** Skapa SVG-element. attrs: objekt med attributnamn. */
function S(tag, attrs, kids) {
  const n = document.createElementNS(SVGNS, tag);
  if (attrs) for (const k in attrs) {
    const v = attrs[k];
    if (v === null || v === undefined || v === false) continue;
    n.setAttribute(k, v);
  }
  if (kids) (Array.isArray(kids) ? kids : [kids]).forEach(k => {
    if (k == null) return;
    n.appendChild(typeof k === 'string' ? document.createTextNode(k) : k);
  });
  return n;
}

/** Skapa HTML-element. */
function H(tag, attrs, kids) {
  const n = document.createElement(tag);
  if (attrs) for (const k in attrs) {
    const v = attrs[k];
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') n.className = v;
    else if (k === 'html') n.innerHTML = v;
    else if (k === 'text') n.textContent = v;
    else if (k.slice(0, 2) === 'on') n.addEventListener(k.slice(2), v);
    else n.setAttribute(k, v);
  }
  if (kids) (Array.isArray(kids) ? kids : [kids]).forEach(k => {
    if (k == null) return;
    n.appendChild(typeof k === 'string' ? document.createTextNode(k) : k);
  });
  return n;
}

const clamp = (v, a, b) => v < a ? a : (v > b ? b : v);
const lerp = (a, b, t) => a + (b - a) * t;
const TAU = Math.PI * 2;
const D2R = Math.PI / 180, R2D = 180 / Math.PI;

let _uidn = 0;
function uid(p) { _uidn++; return (p || 'i') + '_' + Date.now().toString(36).slice(-4) + _uidn.toString(36); }

/** 12345.6 -> "12 346"  (mm, heltal, tunt mellanslag som tusentalsavgränsare) */
function mm(v, dec) {
  const d = dec === undefined ? 0 : dec;
  const s = Math.abs(v).toFixed(d).replace('.', ',');
  const parts = s.split(',');
  parts[0] = parts[0].replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
  return (v < 0 ? '-' : '') + parts.join(',');
}
/** "4.0" -> "4,0" */
function num(v, dec) {
  if (v === '' || v === null || v === undefined || isNaN(v)) return '';
  return Number(v).toFixed(dec === undefined ? 1 : dec).replace('.', ',');
}

function today() {
  const d = new Date();
  const p = n => String(n).padStart(2, '0');
  return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate());
}

/** A, B, ... Z, AA, AB ... */
function alpha(i) {
  let s = '';
  i = i | 0;
  do { s = String.fromCharCode(65 + (i % 26)) + s; i = Math.floor(i / 26) - 1; } while (i >= 0);
  return s;
}

/* ---------- geometri-primitiver ---------- */
const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
const dist2 = (ax, ay, bx, by) => Math.hypot(ax - bx, ay - by);

/** Närmaste punkt på linjesegment a-b från p. Returnerar {x,y,t,d} */
function closestOnSeg(p, a, b) {
  const dx = b.x - a.x, dy = b.y - a.y;
  const L2 = dx * dx + dy * dy;
  let t = L2 ? ((p.x - a.x) * dx + (p.y - a.y) * dy) / L2 : 0;
  t = clamp(t, 0, 1);
  const x = a.x + dx * t, y = a.y + dy * t;
  return { x: x, y: y, t: t, d: Math.hypot(p.x - x, p.y - y) };
}

/** Punkt i polygon (ray casting) */
function pointInPoly(p, poly) {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const xi = poly[i].x, yi = poly[i].y, xj = poly[j].x, yj = poly[j].y;
    if (((yi > p.y) !== (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi)) inside = !inside;
  }
  return inside;
}

function polyBBox(poly) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const p of poly) { if (p.x < x0) x0 = p.x; if (p.y < y0) y0 = p.y; if (p.x > x1) x1 = p.x; if (p.y > y1) y1 = p.y; }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0, x1: x1, y1: y1 };
}

function polyArea(poly) {
  let a = 0;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++)
    a += (poly[j].x + poly[i].x) * (poly[j].y - poly[i].y);
  return Math.abs(a / 2);
}

function polyCentroid(poly) {
  let a = 0, cx = 0, cy = 0;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const f = poly[j].x * poly[i].y - poly[i].x * poly[j].y;
    a += f; cx += (poly[j].x + poly[i].x) * f; cy += (poly[j].y + poly[i].y) * f;
  }
  if (Math.abs(a) < 1e-9) { const b = polyBBox(poly); return { x: b.x + b.w / 2, y: b.y + b.h / 2 }; }
  a *= 0.5;
  return { x: cx / (6 * a), y: cy / (6 * a) };
}

/** normaliserad rektangel från två punkter */
function rectFrom(a, b) {
  return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(a.x - b.x), h: Math.abs(a.y - b.y) };
}
function rectPoly(r) {
  return [{ x: r.x, y: r.y }, { x: r.x + r.w, y: r.y }, { x: r.x + r.w, y: r.y + r.h }, { x: r.x, y: r.y + r.h }];
}
function rectsOverlap(a, b, pad) {
  const p = pad || 0;
  return !(a.x + a.w < b.x - p || b.x + b.w < a.x - p || a.y + a.h < b.y - p || b.y + b.h < a.y - p);
}
function pointInRect(p, r, pad) {
  const q = pad || 0;
  return p.x >= r.x - q && p.x <= r.x + r.w + q && p.y >= r.y - q && p.y <= r.y + r.h + q;
}

/** Snittest mellan två former (rect|circle) för sammanslagning av dubblingar */
function shapesTouch(a, b, tol) {
  const t = tol || 0;
  if (a.kind === 'arc' || b.kind === 'arc') return rectsOverlap(shapeBBox(a), shapeBBox(b), t);
  if (a.kind === 'rect' && b.kind === 'rect') return rectsOverlap(a, b, t);
  if (a.kind === 'circle' && b.kind === 'circle') return dist2(a.cx, a.cy, b.cx, b.cy) <= a.r + b.r + t;
  const r = a.kind === 'rect' ? a : b, c = a.kind === 'rect' ? b : a;
  const nx = clamp(c.cx, r.x, r.x + r.w), ny = clamp(c.cy, r.y, r.y + r.h);
  return dist2(nx, ny, c.cx, c.cy) <= c.r + t;
}

/** punkter längs en bågform (annulussektor) */
function arcPts(s, n) {
  const N = n || clamp(Math.ceil(Math.abs(s.a1 - s.a0) / 0.05), 6, 240);
  const out = [];
  for (let i = 0; i <= N; i++) { const a = lerp(s.a0, s.a1, i / N); out.push({ x: Math.cos(a) * s.r0, y: Math.sin(a) * s.r0 }); }
  for (let i = N; i >= 0; i--) { const a = lerp(s.a0, s.a1, i / N); out.push({ x: Math.cos(a) * s.r1, y: Math.sin(a) * s.r1 }); }
  return out;
}
const arcLen = s => Math.abs(s.a1 - s.a0) * ((s.r0 + s.r1) / 2);

function shapeBBox(s) {
  if (s.kind === 'arc') return polyBBox(arcPts(s));
  if (s.kind === 'circle') return { x: s.cx - s.r, y: s.cy - s.r, w: s.r * 2, h: s.r * 2 };
  if (s.kind === 'poly') return polyBBox(s.pts);
  return { x: s.x, y: s.y, w: s.w, h: s.h };
}
function shapesBBox(list) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  list.forEach(s => { const b = shapeBBox(s); x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y); x1 = Math.max(x1, b.x + b.w); y1 = Math.max(y1, b.y + b.h); });
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}
function shapeArea(s) {
  if (s.kind === 'arc') return Math.abs(s.a1 - s.a0) / 2 * (s.r1 * s.r1 - s.r0 * s.r0);
  if (s.kind === 'circle') return Math.PI * s.r * s.r;
  if (s.kind === 'poly') return polyArea(s.pts);
  return Math.abs(s.w * s.h);
}
function shapeCenter(s) {
  if (s.kind === 'arc') {
    const a = (s.a0 + s.a1) / 2, r = (s.r0 + s.r1) / 2;
    return { x: Math.cos(a) * r, y: Math.sin(a) * r };
  }
  if (s.kind === 'circle') return { x: s.cx, y: s.cy };
  if (s.kind === 'poly') return polyCentroid(s.pts);
  return { x: s.x + s.w / 2, y: s.y + s.h / 2 };
}
function shapeHit(s, p, tol) {
  const t = tol || 0;
  if (s.kind === 'arc') {
    const r = Math.hypot(p.x, p.y);
    if (r < s.r0 - t || r > s.r1 + t) return false;
    let a = Math.atan2(p.y, p.x);
    const at = t / Math.max(1, r);
    while (a - s.a0 < -Math.PI) a += TAU;
    while (a - s.a0 > TAU) a -= TAU;
    return a >= s.a0 - at && a <= s.a1 + at;
  }
  if (s.kind === 'circle') return dist2(p.x, p.y, s.cx, s.cy) <= s.r + t;
  if (s.kind === 'poly') return pointInPoly(p, s.pts) || nearPoly(p, s.pts) <= t;
  return pointInRect(p, s, t);
}
function nearPoly(p, pts) {
  let d = Infinity;
  for (let i = 0; i < pts.length; i++) d = Math.min(d, closestOnSeg(p, pts[i], pts[(i + 1) % pts.length]).d);
  return d;
}
function moveShape(s, dx, dy) {
  if (s.kind === 'arc') {
    const c = shapeCenter(s);
    const na = Math.atan2(c.y + dy, c.x + dx);
    const mid = (s.a0 + s.a1) / 2;
    let d = na - mid;
    while (d > Math.PI) d -= TAU;
    while (d < -Math.PI) d += TAU;
    s.a0 += d; s.a1 += d;
    return;
  }
  if (s.kind === 'circle') { s.cx += dx; s.cy += dy; }
  else if (s.kind === 'poly') { s.pts.forEach(p => { p.x += dx; p.y += dy; }); }
  else { s.x += dx; s.y += dy; }
}
function cloneShape(s) { return JSON.parse(JSON.stringify(s)); }

/** SVG path-data för en form */
function shapePath(s) {
  if (s.kind === 'arc') {
    const sw = s.a1 >= s.a0 ? 1 : 0;
    const big = Math.abs(s.a1 - s.a0) > Math.PI ? 1 : 0;
    const P = (r, a) => r2(Math.cos(a) * r) + ' ' + r2(Math.sin(a) * r);
    return 'M' + P(s.r0, s.a0) +
      'A' + r2(s.r0) + ' ' + r2(s.r0) + ' 0 ' + big + ' ' + sw + ' ' + P(s.r0, s.a1) +
      'L' + P(s.r1, s.a1) +
      'A' + r2(s.r1) + ' ' + r2(s.r1) + ' 0 ' + big + ' ' + (1 - sw) + ' ' + P(s.r1, s.a0) + 'Z';
  }
  if (s.kind === 'circle') {
    const r = s.r;
    return 'M' + (s.cx - r) + ' ' + s.cy + 'a' + r + ' ' + r + ' 0 1 0 ' + (r * 2) + ' 0a' + r + ' ' + r + ' 0 1 0 ' + (-r * 2) + ' 0Z';
  }
  if (s.kind === 'poly') return polyPath(s.pts);
  return 'M' + s.x + ' ' + s.y + 'h' + s.w + 'v' + s.h + 'h' + (-s.w) + 'Z';
}
function polyPath(pts, open) {
  if (!pts || !pts.length) return '';
  let d = 'M' + r2(pts[0].x) + ' ' + r2(pts[0].y);
  for (let i = 1; i < pts.length; i++) d += 'L' + r2(pts[i].x) + ' ' + r2(pts[i].y);
  return d + (open ? '' : 'Z');
}
const r2 = v => Math.round(v * 100) / 100;

/* ---------- toast ---------- */
function toast(msg, kind) {
  const host = $('#toasts'); if (!host) return;
  const t = H('div', { class: 'toast ' + (kind || ''), text: msg });
  host.appendChild(t);
  setTimeout(() => {
    t.style.transition = 'opacity .3s, transform .3s';
    t.style.opacity = '0'; t.style.transform = 'translateY(-6px)';
    setTimeout(() => t.remove(), 320);
  }, kind === 'warn' ? 3400 : 2000);
}

/* ---------- enkel dialog ---------- */
const Dlg = {
  open(title, fields, onOk) {
    const box = $('#dlg'), body = $('#dlg-body');
    $('#dlg-title').textContent = title;
    body.innerHTML = '';
    const inputs = {};
    fields.forEach(f => {
      const lab = H('label', { class: 'fld' }, [H('span', { text: f.label })]);
      let inp;
      if (f.type === 'textarea') inp = H('textarea', { placeholder: f.ph || '' });
      else if (f.type === 'select') {
        inp = H('select');
        f.options.forEach(o => inp.appendChild(H('option', { value: o.v, text: o.t })));
      } else inp = H('input', { type: f.type || 'text', placeholder: f.ph || '' });
      inp.value = f.value === undefined ? '' : f.value;
      inputs[f.key] = inp;
      lab.appendChild(inp);
      if (f.hint) lab.appendChild(H('small', { class: 'hint', text: f.hint }));
      body.appendChild(lab);
    });
    box.hidden = false;
    const first = body.querySelector('input,textarea,select');
    if (first) setTimeout(() => { first.focus(); first.select && first.select(); }, 30);

    const close = () => { box.hidden = true; $('#dlg-ok').onclick = null; $('#dlg-cancel').onclick = null; $('#dlg-x').onclick = null; document.removeEventListener('keydown', key, true); };
    const ok = () => { const out = {}; for (const k in inputs) out[k] = inputs[k].value; close(); onOk && onOk(out); };
    const key = e => {
      if (e.key === 'Escape') { e.stopPropagation(); close(); }
      if (e.key === 'Enter' && e.target.tagName !== 'TEXTAREA') { e.stopPropagation(); ok(); }
    };
    document.addEventListener('keydown', key, true);
    $('#dlg-ok').onclick = ok;
    $('#dlg-cancel').onclick = close;
    $('#dlg-x').onclick = close;
  }
};
