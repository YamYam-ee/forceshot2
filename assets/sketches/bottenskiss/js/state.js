/* ==========================================================================
   state.js – dokument, historik (ångra/gör om) och händelser
   ========================================================================== */
'use strict';

function defaultDoc() {
  return {
    v: 1,
    meta: { objekt: '', cistern: '', datum: today(), ritad: '', tjocklek: '', anm: '' },
    tank: { diameter: 12000, orientation: 'h' },
    ring: { on: true, count: 20, width: 1000, rot: 0, angles: null },
    layout: {
      mode: 'count',
      rows: 9, mid: 7, first: 4, last: 4,
      plateL: 3000, plateW: 1500, rowAlign: 'center',
      stagger: 0.5, mergeSlivers: true,
      rowOverrides: {}, rowSeams: {}, rowY: null
    },
    view: {
      ids: true, refs: true, sheet: true, table: true, north: true, snap: true,
      dimDia: true, dimPlate: true, rot: 0, northPos: null, northRot: 0,
      flipRows: false, flipCols: false
    },
    items: [],
    seq: 1
  };
}

const State = (function () {
  const listeners = {};
  let doc = defaultDoc();
  let geo = null;
  let sel = [];
  let tool = 'select';
  let hist = [], hidx = -1;
  let suspend = false;

  function on(ev, fn) { (listeners[ev] = listeners[ev] || []).push(fn); }
  function emit(ev, data) { (listeners[ev] || []).forEach(f => f(data)); }

  function regen() {
    geo = Geo.build(doc);
    api.geo = geo;
    emit('geo', geo);
  }

  /* ---------- historik ---------- */
  function snap() { return JSON.stringify(doc); }
  function resetHistory() { hist = [snap()]; hidx = 0; emit('history'); }
  function commit(label) {
    if (suspend) return;
    const s = snap();
    if (hist[hidx] === s) return;
    hist = hist.slice(0, hidx + 1);
    hist.push(s);
    if (hist.length > 120) hist.shift();
    hidx = hist.length - 1;
    emit('history', label);
  }
  function restore(s) {
    doc = JSON.parse(s);
    api.doc = doc;
    sel = sel.filter(id => byId(id));
    regen();
    emit('items'); emit('meta'); emit('sel');
  }
  function undo() {
    if (hidx <= 0) { toast('Inget mer att ångra'); return; }
    hidx--; restore(hist[hidx]); emit('history');
    toast('Ångrade');
  }
  function redo() {
    if (hidx >= hist.length - 1) { toast('Inget mer att göra om'); return; }
    hidx++; restore(hist[hidx]); emit('history');
    toast('Gjorde om');
  }
  const canUndo = () => hidx > 0;
  const canRedo = () => hidx < hist.length - 1;

  /* ---------- objekt ---------- */
  function byId(id) { return doc.items.find(i => i.id === id) || null; }
  function add(item, quiet) {
    item.id = item.id || uid(item.type);
    item.seq = doc.seq++;
    doc.items.push(item);
    if (item.type === 'split') regen();
    if (!quiet) { emit('items'); commit('lägg till ' + item.type); }
    return item;
  }
  function remove(ids) {
    const set = new Set(Array.isArray(ids) ? ids : [ids]);
    const n = doc.items.length;
    const hadSplit = doc.items.some(i => set.has(i.id) && i.type === 'split');
    doc.items = doc.items.filter(i => !set.has(i.id));
    if (hadSplit) regen();
    sel = sel.filter(id => !set.has(id));
    if (doc.items.length !== n) { emit('items'); emit('sel'); commit('ta bort'); }
  }
  function touch(commitLabel) {
    emit('items');
    if (commitLabel) commit(commitLabel);
  }

  /* ---------- markering ---------- */
  function select(ids, addTo) {
    const arr = ids == null ? [] : (Array.isArray(ids) ? ids : [ids]);
    if (addTo) { arr.forEach(id => { if (sel.indexOf(id) < 0) sel.push(id); }); }
    else sel = arr.slice();
    emit('sel', sel);
  }
  function isSel(id) { return sel.indexOf(id) >= 0; }

  /* ---------- verktyg ---------- */
  function setTool(t) { tool = t; emit('tool', t); }

  /* ---------- härledda etiketter & grupper ---------- */
  const PREFIX = { doubler: 'D', repair: '', point: '', manhole: 'MH', nozzle: 'S', sump: 'P', text: 'N', split: '' };

  /** numrering per typ i skapandeordning */
  function numbers() {
    const map = {};
    const byType = {};
    doc.items.slice().sort((a, b) => a.seq - b.seq).forEach(i => {
      byType[i.type] = (byType[i.type] || 0);
    });
    // dubblingar numreras per grupp
    const gs = groups();
    gs.forEach((g, gi) => g.ids.forEach(id => { map[id] = 'D' + (gi + 1); }));
    let rn = 0, mh = 0, nz = 0, sp = 0, tx = 0;
    doc.items.slice().sort((a, b) => a.seq - b.seq).forEach(i => {
      if (i.type === 'repair') map[i.id] = String(++rn);
      else if (i.type === 'manhole') map[i.id] = 'MH' + (++mh);
      else if (i.type === 'nozzle') map[i.id] = 'S' + (++nz);
      else if (i.type === 'sump') map[i.id] = 'P' + (++sp);
      else if (i.type === 'text') map[i.id] = 'N' + (++tx);
    });
    return map;
  }

  /** sammanhängande dubblingar -> grupper (för sammanslagen kontur) */
  function groups() {
    const ds = doc.items.filter(i => i.type === 'doubler').sort((a, b) => a.seq - b.seq);
    const parent = ds.map((_, i) => i);
    const find = a => { while (parent[a] !== a) { parent[a] = parent[parent[a]]; a = parent[a]; } return a; };
    const uni = (a, b) => { a = find(a); b = find(b); if (a !== b) parent[Math.max(a, b)] = Math.min(a, b); };
    const tol = geo ? Math.max(2, geo.R * 0.0012) : 2;
    for (let i = 0; i < ds.length; i++)
      for (let j = i + 1; j < ds.length; j++)
        if (shapesTouch(ds[i].shape, ds[j].shape, tol)) uni(i, j);
    const buckets = {};
    ds.forEach((d, i) => {
      const r = find(i);
      (buckets[r] = buckets[r] || { ids: [], items: [], shapes: [] });
      buckets[r].ids.push(d.id); buckets[r].items.push(d); buckets[r].shapes.push(d.shape);
    });
    return Object.keys(buckets).map(k => buckets[k]).sort((a, b) => a.items[0].seq - b.items[0].seq);
  }

  const api = {
    get doc() { return doc; },
    set doc(d) { doc = d; },
    get geo() { return geo; },
    set geo(g) { geo = g; },
    get sel() { return sel; },
    get tool() { return tool; },
    on: on, emit: emit, regen: regen,
    commit: commit, undo: undo, redo: redo, canUndo: canUndo, canRedo: canRedo, resetHistory: resetHistory,
    byId: byId, add: add, remove: remove, touch: touch,
    select: select, isSel: isSel, setTool: setTool,
    numbers: numbers, groups: groups,
    load(d) {
      doc = Object.assign(defaultDoc(), d);
      doc.layout = Object.assign(defaultDoc().layout, d.layout || {});
      doc.view = Object.assign(defaultDoc().view, d.view || {});
      if (d.view && d.view.dims !== undefined && d.view.dimDia === undefined) {
        doc.view.dimDia = !!d.view.dims; doc.view.dimPlate = !!d.view.dims;
      }
      doc.ring = Object.assign(defaultDoc().ring, d.ring || {});
      doc.tank = Object.assign(defaultDoc().tank, d.tank || {});
      doc.meta = Object.assign(defaultDoc().meta, d.meta || {});
      sel = [];
      regen(); emit('items'); emit('meta'); emit('sel'); resetHistory();
    },
    reset() { doc = defaultDoc(); sel = []; regen(); emit('items'); emit('meta'); emit('sel'); resetHistory(); },
    suspend(v) { suspend = v; }
  };
  return api;
})();
