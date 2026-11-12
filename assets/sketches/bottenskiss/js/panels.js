/* ==========================================================================
   panels.js – sidopaneler: inställningar, radtabell, objektlista, egenskaper
   ========================================================================== */
'use strict';

const Panels = (function () {

  let building = false;
  const VMAP = {
    '#v-ids': 'ids', '#v-refs': 'refs', '#v-sheet': 'sheet', '#v-table': 'table',
    '#v-north': 'north', '#v-dia': 'dimDia', '#v-plate': 'dimPlate', '#v-snap': 'snap'
  };
  function markRotQuick() {
    const r = State.doc.view.rot || 0;
    $$('#v-rot-q button').forEach(b => b.classList.toggle('on', +b.dataset.r === r));
  }

  /* ---------------- bindningar ---------------- */
  function init() {
    const doc = State.doc;

    /* meta */
    bindText('#m-objekt', v => State.doc.meta.objekt = v, () => State.doc.meta.objekt, true);
    bindText('#m-cistern', v => State.doc.meta.cistern = v, () => State.doc.meta.cistern, true);
    bindText('#m-datum', v => State.doc.meta.datum = v, () => State.doc.meta.datum, true);
    bindText('#m-ritad', v => State.doc.meta.ritad = v, () => State.doc.meta.ritad, true);
    bindText('#c-tjock', v => State.doc.meta.tjocklek = v, () => State.doc.meta.tjocklek, true);

    /* cistern */
    bindNum('#c-dia', v => State.doc.tank.diameter = clamp(v, 500, 200000), () => State.doc.tank.diameter);
    $$('#c-orient button').forEach(b => b.addEventListener('click', () => {
      $$('#c-orient button').forEach(x => x.classList.toggle('on', x === b));
      State.doc.tank.orientation = b.dataset.v;
      regen('riktning');
    }));

    /* vy-vridning */
    $('#v-rot').addEventListener('input', e => {
      State.doc.view.rot = +e.target.value;
      $('#v-rot-v').textContent = e.target.value + '°';
      markRotQuick();
      State.regen();
    });
    $('#v-rot').addEventListener('change', () => State.commit('vrid vy'));
    $$('#v-rot-q button').forEach(b => b.addEventListener('click', () => {
      State.doc.view.rot = +b.dataset.r;
      $('#v-rot').value = b.dataset.r;
      $('#v-rot-v').textContent = b.dataset.r + '°';
      markRotQuick();
      State.regen(); State.commit('vrid vy');
    }));

    /* randplåt */
    $('#r-on').addEventListener('change', e => {
      State.doc.ring.on = e.target.checked;
      $('#r-body').style.opacity = e.target.checked ? '1' : '.4';
      $('#r-body').style.pointerEvents = e.target.checked ? '' : 'none';
      regen('randplåt');
    });
    bindNum('#r-count', v => { State.doc.ring.count = clamp(v, 0, 400); State.doc.ring.angles = null; }, () => State.doc.ring.count);
    bindNum('#r-width', v => State.doc.ring.width = Math.max(50, v), () => State.doc.ring.width);
    $('#r-rot').addEventListener('input', e => {
      State.doc.ring.rot = +e.target.value;
      $('#r-rot-v').textContent = e.target.value + '°';
      State.regen();
    });
    $('#r-rot').addEventListener('change', () => State.commit('vridning'));

    /* läge: antal eller mått */
    $$('#l-mode button').forEach(b => b.addEventListener('click', () => {
      const v = b.dataset.v;
      const d = State.doc.layout;
      if (v === 'size' && d.mode !== 'size') {
        d.plateL = Math.round(State.geo.L / 10) * 10;
        d.plateW = Math.round(State.geo.bandH / 10) * 10;
      }
      d.mode = v;
      d.rowY = null; d.rowSeams = {}; d.rowOverrides = {};
      syncMode();
      regen('läge');
    }));
    bindNum('#l-plate-l', v => { State.doc.layout.plateL = Math.max(100, v); clearManual(); }, () => State.doc.layout.plateL);
    bindNum('#l-plate-w', v => { State.doc.layout.plateW = Math.max(100, v); clearManual(); }, () => State.doc.layout.plateW);
    $$('#l-align button').forEach(b => b.addEventListener('click', () => {
      State.doc.layout.rowAlign = b.dataset.v;
      $$('#l-align button').forEach(x => x.classList.toggle('on', x === b));
      clearManual();
      regen('radläge');
    }));

    /* layout */
    bindNum('#l-rows', v => {
      State.doc.layout.rows = clamp(v, 1, 60);
      State.doc.layout.rowY = null; State.doc.layout.rowSeams = {};
    }, () => State.doc.layout.rows);
    bindNum('#l-mid', v => State.doc.layout.mid = clamp(v, 1, 60), () => State.doc.layout.mid);
    bindNum('#l-first', v => State.doc.layout.first = clamp(v, 1, 60), () => State.doc.layout.first);
    bindNum('#l-last', v => State.doc.layout.last = clamp(v, 1, 60), () => State.doc.layout.last);
    $('#l-stagger').addEventListener('input', e => {
      State.doc.layout.stagger = (+e.target.value) / 100;
      $('#l-stag-v').textContent = e.target.value + ' %';
      State.regen();
    });
    $('#l-stagger').addEventListener('change', () => State.commit('förskjutning'));
    $('#l-merge').addEventListener('change', e => { State.doc.layout.mergeSlivers = e.target.checked; regen('kantplåtar'); });

    /* visning */
    Object.keys(VMAP).forEach(sel => {
      $(sel).addEventListener('change', e => {
        State.doc.view[VMAP[sel]] = e.target.checked;
        Render.drawAll(); State.commit('visning');
      });
    });
    /* vänd numreringen – etiketterna sätts i geometrin, därför regen */
    $('#v-flip-rows').addEventListener('change', e => {
      State.doc.view.flipRows = e.target.checked;
      regen('bokstavsordning');
    });
    $('#v-flip-cols').addEventListener('change', e => {
      State.doc.view.flipCols = e.target.checked;
      regen('sifferordning');
    });
    $('#v-north-rot').addEventListener('input', e => {
      State.doc.view.northRot = +e.target.value;
      $('#v-north-rot-v').textContent = e.target.value + '°';
      Render.drawSheet();
    });
    $('#v-north-rot').addEventListener('change', () => State.commit('norrpil'));

    $('#l-reset-seams').addEventListener('click', () => {
      State.doc.layout.rowY = null;
      State.doc.layout.rowSeams = {};
      State.doc.ring.angles = null;
      regen('nollställ skarvar');
      toast('Handjusterade skarvar nollställda');
    });
    $('#v-north-reset').addEventListener('click', () => {
      State.doc.view.northPos = null; State.doc.view.northRot = 0;
      Render.drawStatic(); State.commit('norrpil'); toast('Norrpilen återställd');
    });

    State.on('geo', () => { rowTable(); derived(); noteTable(); props(); });
    State.on('items', () => { objList(); noteTable(); props(); });
    State.on('sel', () => { objList(); props(true); });
    State.on('meta', () => syncInputs());
  }

  function clearManual() {
    const d = State.doc.layout;
    d.rowY = null; d.rowSeams = {};
  }
  function syncMode() {
    const d = State.doc.layout;
    const size = d.mode === 'size';
    $$('#l-mode button').forEach(b => b.classList.toggle('on', (b.dataset.v === 'size') === size));
    $('#l-count-body').hidden = size;
    $('#l-size-body').hidden = !size;
    $$('#l-align button').forEach(b => b.classList.toggle('on', b.dataset.v === (d.rowAlign || 'center')));
    const mchk = $('#l-merge').closest('.chk');
    if (mchk) mchk.style.display = size ? 'none' : '';
    $('#l-plate-l').value = Math.round(d.plateL || 3000);
    $('#l-plate-w').value = Math.round(d.plateW || 1500);
  }

  function regen(label) {
    State.regen();
    State.commit(label);
  }

  function bindText(sel, set, get, metaOnly) {
    const el = $(sel);
    el.addEventListener('input', () => { set(el.value); if (metaOnly) Render.drawStatic(); });
    el.addEventListener('change', () => State.commit('uppgift'));
  }
  function bindNum(sel, set, get) {
    const el = $(sel);
    let t = null;
    el.addEventListener('input', () => {
      const v = parseFloat(el.value);
      if (isNaN(v)) return;
      set(v);
      clearTimeout(t);
      t = setTimeout(() => { State.regen(); }, 60);
    });
    el.addEventListener('change', () => { State.regen(); State.commit('mått'); });
  }

  function syncInputs() {
    building = true;
    const d = State.doc;
    $('#m-objekt').value = d.meta.objekt || '';
    $('#m-cistern').value = d.meta.cistern || '';
    $('#m-datum').value = d.meta.datum || '';
    $('#m-ritad').value = d.meta.ritad || '';
    $('#c-tjock').value = d.meta.tjocklek || '';
    $('#c-dia').value = d.tank.diameter;
    $$('#c-orient button').forEach(b => b.classList.toggle('on', b.dataset.v === d.tank.orientation));
    $('#r-on').checked = !!d.ring.on;
    $('#r-body').style.opacity = d.ring.on ? '1' : '.4';
    $('#r-body').style.pointerEvents = d.ring.on ? '' : 'none';
    $('#r-count').value = d.ring.count;
    $('#r-width').value = d.ring.width;
    $('#r-rot').value = d.ring.rot; $('#r-rot-v').textContent = d.ring.rot + '°';
    $('#l-rows').value = d.layout.rows;
    $('#l-mid').value = d.layout.mid;
    $('#l-first').value = d.layout.first;
    $('#l-last').value = d.layout.last;
    $('#l-stagger').value = Math.round(d.layout.stagger * 100);
    $('#l-stag-v').textContent = Math.round(d.layout.stagger * 100) + ' %';
    $('#l-merge').checked = d.layout.mergeSlivers !== false;
    syncMode();
    Object.keys(VMAP).forEach(k => $(k).checked = !!d.view[VMAP[k]]);
    $('#v-flip-rows').checked = !!d.view.flipRows;
    $('#v-flip-cols').checked = !!d.view.flipCols;
    $('#v-north-rot').value = Math.round(d.view.northRot || 0);
    $('#v-north-rot-v').textContent = Math.round(d.view.northRot || 0) + '°';
    $('#v-rot').value = d.view.rot || 0;
    $('#v-rot-v').textContent = (d.view.rot || 0) + '°';
    markRotQuick();
    building = false;
  }

  /* ---------------- härledd info ---------------- */
  function derived() {
    const geo = State.geo, d = State.doc;
    const box = $('#l-derived');
    let html = '';
    html += 'Plåtmått <b>' + mm(geo.L) + ' × ' + mm(geo.bandH) + ' mm</b><br>';
    if (geo.sizeMode) html += 'Ger <b>' + geo.N + ' rader</b> · mittraden <b>' + geo.rows[geo.midIdx].plates.length + ' plåtar</b><br>';
    html += 'Antal bottenplåtar <b>' + geo.plates.length + '</b>';
    if (geo.ringOn) html += geo.ringWhole ? ' · randplåt <b>odelad</b>' : ' · randplåtar <b>' + geo.ring.length + '</b>';
    const area = Math.PI * Math.pow(geo.R / 1000, 2);
    html += '<br>Bottenyta <b>' + num(area, 1) + ' m²</b>';
    if (geo.notes.length) {
      html += '<br><span class="warn">';
      html += geo.notes.map(n => 'Rad ' + (n.ref || alpha(n.row)) + ': ' + n.n + ' plåtar à ' + mm(n.w) + ' mm (avvikande bredd)').join('<br>');
      html += '</span>';
    }
    box.innerHTML = html;
  }

  /* ---------------- radtabell ---------------- */
  function rowTable() {
    const geo = State.geo, d = State.doc;
    const host = $('#rowtable');
    host.innerHTML = '';
    geo.rows.forEach(r => {
      const isFirst = r.i === 0, isLast = r.i === geo.N - 1, isMid = r.i === geo.midIdx;
      const ov = d.layout.rowOverrides[r.i] != null || (d.layout.rowSeams && d.layout.rowSeams[r.i]);
      const row = H('div', { class: 'rrow' + (ov ? ' locked' : '') });
      row.appendChild(H('span', { class: 'rl', text: r.label }));
      const extra = r.plates.length - r.base;
      row.appendChild(H('span', {
        class: 'rc',
        text: r.base + ' plåtar' + (extra > 0 ? ' +' + extra + ' kluvna' : '') +
          (r.manual ? ' · justerad' : r.even ? ' · egen bredd' : '') +
          (isMid ? ' · mitt' : isFirst ? ' · första' : isLast ? ' · sista' : '')
      }));
      const dec = H('button', { text: '−', title: 'Färre plåtar' });
      const inc = H('button', { text: '+', title: 'Fler plåtar – plåtbredden i raden minskar' });
      const lk = H('button', { class: 'lk', text: ov ? '⌫' : '·', title: ov ? 'Återställ raden' : 'Automatisk' });
      dec.onclick = () => setRow(r.i, r.base - 1);
      inc.onclick = () => setRow(r.i, r.base + 1);
      lk.onclick = () => {
        delete d.layout.rowOverrides[r.i];
        if (d.layout.rowSeams) delete d.layout.rowSeams[r.i];
        regen('rad');
      };
      row.appendChild(dec); row.appendChild(inc); row.appendChild(lk);
      host.appendChild(row);
    });
  }
  function setRow(i, n) {
    const d = State.doc, geo = State.geo;
    n = Math.max(1, n);
    if (d.layout.rowSeams) delete d.layout.rowSeams[i];
    if (i === 0) { d.layout.first = n; $('#l-first').value = n; }
    else if (i === geo.N - 1) { d.layout.last = n; $('#l-last').value = n; }
    else if (i === geo.midIdx) { d.layout.mid = n; $('#l-mid').value = n; }
    else d.layout.rowOverrides[i] = n;
    regen('rad');
  }

  /* ---------------- objektlista ---------------- */
  const TYPE_INFO = {
    doubler: { t: 'Dubbling', c: '#8b98a8' }, repair: { t: 'Reparation', c: '#ff5a5a' },
    point: { t: 'Mätpunkt', c: '#ff5a5a' }, split: { t: 'Klyvning', c: '#cfd8e3' },
    manhole: { t: 'Mannlucka', c: '#4a9eff' }, nozzle: { t: 'Anslutning', c: '#4a9eff' },
    sump: { t: 'Potta', c: '#38c793' }, text: { t: 'Anteckning', c: '#8a94a3' }
  };

  function objList() {
    const host = $('#objlist'); host.innerHTML = '';
    const items = State.doc.items.slice().sort((a, b) => b.seq - a.seq);
    $('#obj-count').textContent = items.length;
    if (!items.length) { host.appendChild(H('p', { class: 'empty', text: 'Inga objekt inritade ännu.' })); return; }
    const num = State.numbers();
    items.forEach(it => {
      const info = TYPE_INFO[it.type] || { t: it.type, c: '#888' };
      const row = H('div', { class: 'orow' + (State.isSel(it.id) ? ' on' : '') });
      row.appendChild(H('span', { class: 'swatch', style: 'background:' + info.c }));
      const nm = (num[it.id] ? num[it.id] + ' · ' : '') + info.t;
      row.appendChild(H('span', { class: 'nm', text: nm }));
      const meta = it.type === 'point' ? (it.value || '') :
        it.type === 'text' ? String(it.text || '').slice(0, 14) :
          (it.thickness || (it.dn ? 'DN' + it.dn : ''));
      row.appendChild(H('span', { class: 'meta', text: meta }));
      const del = H('button', { class: 'del', text: '✕', title: 'Ta bort' });
      del.onclick = e => { e.stopPropagation(); State.remove(it.id); };
      row.appendChild(del);
      row.onclick = e => { State.select(it.id, e.shiftKey); zoomTo(it); };
      host.appendChild(row);
    });
  }
  function zoomTo(it) {
    const b = Item.bbox(it);
    Render.centerOn({ x: b.x + b.w / 2, y: b.y + b.h / 2 });
  }

  /* ---------------- egenskaper ---------------- */
  function focusProps() { $('#pnl-props').scrollIntoView({ behavior: 'smooth', block: 'nearest' }); }

  function props(force) {
    const host = $('#props');
    /* skriver man i ett fält just nu får panelen inte byggas om – då tappas fokus */
    if (!force && host.contains(document.activeElement)) return;
    host.innerHTML = '';
    const sel = State.sel;
    if (!sel.length) { host.appendChild(H('p', { class: 'empty', text: 'Inget valt. Klicka på ett objekt i skissen, eller dubbelklicka på en plåt för närbild.' })); return; }
    if (sel.length > 1) {
      host.appendChild(H('p', { class: 'empty', text: sel.length + ' objekt valda. Flytta med musen eller piltangenter.' }));
      host.appendChild(actionRow(sel));
      return;
    }
    const it = State.byId(sel[0]); if (!it) return;
    const num = State.numbers();
    const geo = State.geo;
    const info = TYPE_INFO[it.type] || { t: it.type };

    host.appendChild(H('div', { class: 'derived', html: '<b>' + (num[it.id] ? num[it.id] + ' – ' : '') + info.t + '</b>' + propSummary(it) }));

    const add = (label, node, hint) => {
      const l = H('label', { class: 'fld' }, [H('span', { text: label }), node]);
      if (hint) l.appendChild(H('small', { class: 'hint', text: hint }));
      host.appendChild(l);
    };
    const txt = (val, on, ph) => { const i = H('input', { type: 'text', value: val || '', placeholder: ph || '' }); i.addEventListener('input', () => on(i.value)); i.addEventListener('change', () => State.commit('egenskap')); return i; };
    const numf = (val, on, step) => { const i = H('input', { type: 'number', value: val, step: step || 1 }); i.addEventListener('input', () => { const v = parseFloat(i.value); if (!isNaN(v)) on(v); }); i.addEventListener('change', () => State.commit('egenskap')); return i; };
    const area = (val, on) => { const i = H('textarea', {}); i.value = val || ''; i.addEventListener('input', () => on(i.value)); i.addEventListener('change', () => State.commit('egenskap')); return i; };
    const upd = () => { State.emit('items'); };

    if (it.type === 'doubler' || it.type === 'repair') {
      const s = it.shape;
      if (s.kind === 'rect') {
        const g = H('div', { class: 'grid2' });
        g.appendChild(H('label', { class: 'fld' }, [H('span', { text: 'Bredd (mm)' }), numf(Math.round(s.w), v => { s.w = v; upd(); }, 10)]));
        g.appendChild(H('label', { class: 'fld' }, [H('span', { text: 'Höjd (mm)' }), numf(Math.round(s.h), v => { s.h = v; upd(); }, 10)]));
        host.appendChild(g);
      } else if (s.kind === 'circle') {
        add('Diameter (mm)', numf(Math.round(s.r * 2), v => { s.r = Math.max(1, v / 2); upd(); }, 10));
      } else if (s.kind === 'arc') {
        const seg = H('div', { class: 'segmented' });
        [['edge', 'Ytterkant'], ['ring', 'Hela bredden'], ['inner', 'Innerkant']].forEach(k => {
          const cur = s.band || 'ring';
          const b = H('button', { class: cur === k[0] ? 'on' : '', text: k[1] });
          b.onclick = () => {
            s.band = k[0];
            it.kindName = k[0] === 'edge' ? 'Kant/svets ytterkant'
              : k[0] === 'inner' ? 'Kant/svets innerkant' : 'Reparation randplåt';
            State.regen(); State.commit('läge'); props();
          };
          seg.appendChild(b);
        });
        add('Läge i ringen', seg, 'Ytterkant = linjen mot mantelsvetsen');
        add('Båglängd (mm)', numf(Math.round(arcLen(s)), v => {
          const rm = (s.r0 + s.r1) / 2, mid = (s.a0 + s.a1) / 2;
          const span = clamp(v / rm, TAU * 0.004, TAU * 0.985);
          s.a0 = mid - span / 2; s.a1 = mid + span / 2; upd();
        }, 50), 'Täcker ' + ringSpan(s));
        const deg = Math.round((((s.a0 + s.a1) / 2 * R2D) % 360 + 360) % 360);
        const sl = H('input', { type: 'range', min: 0, max: 359, value: deg });
        const lbl = H('b', { text: deg + '°' });
        sl.addEventListener('input', () => {
          const span = s.a1 - s.a0, mid = (+sl.value) * D2R;
          s.a0 = mid - span / 2; s.a1 = mid + span / 2;
          lbl.textContent = sl.value + '°'; upd();
        });
        sl.addEventListener('change', () => State.commit('placering'));
        host.appendChild(H('label', { class: 'fld' }, [H('span', {}, ['Placering på randplåten ', lbl]), sl]));
      }
      add('Tjocklek / plåt', txt(it.thickness, v => { it.thickness = v; upd(); }, it.type === 'doubler' ? 't.ex. 6 mm' : 't.ex. 3,6 mm'));
      add(it.type === 'doubler' ? 'Anmärkning' : 'Beskrivning av fynd', area(it.note, v => { it.note = v; upd(); }));
      if (it.type === 'doubler') {
        const grp = State.groups().find(g => g.ids.indexOf(it.id) >= 0);
        if (grp && grp.ids.length > 1) host.appendChild(H('p', { class: 'empty', text: 'Ingår i en sammanslagen dubbling med ' + grp.ids.length + ' delar. Dra isär delen för att dela upp den.' }));
      }
    } else if (it.type === 'point') {
      add('Godstjocklek', txt(it.value, v => { it.value = v; upd(); }, 't.ex. 4,0 mm'));
      add('Anmärkning', area(it.note, v => { it.note = v; upd(); }));
    } else if (it.type === 'sump') {
      const seg = H('div', { class: 'segmented' });
      ['circle', 'rect'].forEach(k => {
        const on = (k === 'rect') ? (it.shape === 'rect') : (it.shape !== 'rect');
        const b = H('button', { class: on ? 'on' : '', text: k === 'rect' ? 'Fyrkantig' : 'Rund' });
        b.onclick = () => { it.shape = k; upd(); State.commit('potta'); props(); };
        seg.appendChild(b);
      });
      add('Form', seg);
      if (it.shape === 'rect') {
        const g = H('div', { class: 'grid2' });
        g.appendChild(H('label', { class: 'fld' }, [H('span', { text: 'Bredd (mm)' }), numf(Math.round(it.w), v => { it.w = v; upd(); }, 10)]));
        g.appendChild(H('label', { class: 'fld' }, [H('span', { text: 'Längd (mm)' }), numf(Math.round(it.h), v => { it.h = v; upd(); }, 10)]));
        host.appendChild(g);
      } else {
        add('Diameter (mm)', numf(Math.round((it.w + it.h) / 2), v => {
          const cx = it.x + it.w / 2, cy = it.y + it.h / 2;
          it.w = v; it.h = v; it.x = cx - v / 2; it.y = cy - v / 2; upd();
        }, 50));
      }
      add('Djup (mm)', numf(it.depth || 0, v => { it.depth = v; upd(); }, 10));
      add('Text', txt(it.note, v => { it.note = v; upd(); }, 't.ex. Ø600 djup 300'));
    } else if (it.type === 'manhole' || it.type === 'nozzle') {
      add('Dimension DN', numf(it.dn, v => { it.dn = v; upd(); }, 25));
      const deg = Math.round(((it.angle * R2D) % 360 + 360) % 360);
      const sl = H('input', { type: 'range', min: 0, max: 359, value: deg });
      const lbl = H('b', { text: deg + '°' });
      sl.addEventListener('input', () => { it.angle = (+sl.value) * D2R; lbl.textContent = sl.value + '°'; it.free = null; upd(); });
      sl.addEventListener('change', () => State.commit('placering'));
      const l = H('label', { class: 'fld' }, [H('span', {}, ['Placering på manteln ', lbl]), sl]);
      host.appendChild(l);
      add('Text', txt(it.note, v => { it.note = v; upd(); }, it.type === 'manhole' ? 't.ex. MH1 600×800' : 't.ex. inlopp'));
    } else if (it.type === 'text') {
      add('Text', area(it.text, v => { it.text = v; upd(); }));
    } else if (it.type === 'split') {
      const len = dist(it.a, it.b);
      host.appendChild(H('p', { class: 'empty', text: 'Längd ' + mm(len) + ' mm. Dra i ändpunkterna för att justera.' }));
    }
    if (it.type !== 'split') {
      const cb = H('input', { type: 'checkbox' });
      cb.checked = !!it.hideDims;
      cb.addEventListener('change', () => { it.hideDims = cb.checked; upd(); State.commit('dölj mått'); });
      const lab = H('label', { class: 'chk' }, [cb, H('span', { text: 'Dölj mått och text på ritningen' })]);
      lab.style.marginTop = '6px';
      host.appendChild(lab);
      host.appendChild(H('small', { class: 'hint', text: 'Objektet ritas men utan siffror – måtten utelämnas även i anmärkningstabellen.' }));
    }
    host.appendChild(actionRow([it.id]));
  }

  function propSummary(it) {
    const geo = State.geo;
    const c = Item.pos(it);
    const pl = Geo.plateAt(geo, c);
    let s = '';
    if (it.shape && it.shape.kind === 'arc') s += '<br>Randplåt <b>' + ringSpan(it.shape) + '</b>';
    else if (pl) s += '<br>Plåt <b>' + pl.label + '</b>' + (pl.kind === 'ring' ? ' (randplåt)' : '');
    if (it.shape && typeof it.shape === 'object') {
      const b = shapeBBox(it.shape);
      if (it.shape.kind === 'arc') s += '<br>Båglängd <b>' + mm(arcLen(it.shape)) + ' mm</b>';
      else s += '<br>Yta <b>' + num(shapeArea(it.shape) / 1e6, 2) + ' m²</b>';
      if (pl && pl.kind !== 'ring') {
        const l = Geo.toLocal(pl, c);
        s += '<br>Från plåtkant <b>' + mm(l.u) + ' / ' + mm(l.v) + ' mm</b>';
      }
    }
    return s;
  }

  function actionRow(ids) {
    const row = H('div', { style: 'display:flex;gap:6px;margin-top:10px' });
    const dup = H('button', { class: 'btn ghost', text: 'Duplicera', style: 'flex:1' });
    dup.onclick = () => Tools.duplicate();
    const del = H('button', { class: 'btn ghost', text: 'Ta bort', style: 'flex:1;color:#ff6b6b' });
    del.onclick = () => State.remove(ids.slice());
    row.appendChild(dup); row.appendChild(del);
    return row;
  }

  /* ---------------- anmärkningstabell ---------------- */
  function noteTable() {
    const host = $('#notetable'); host.innerHTML = '';
    const rows = reportRows();
    if (!rows.length) { host.appendChild(H('p', { class: 'empty', text: 'Reparationer, dubblingar och mätpunkter listas här och skrivs ut på ritningen.' })); return; }
    const t = H('table');
    const thead = H('thead', {}, [H('tr', {}, ['Nr', 'Typ', 'Plåt', 'Mått', 'Tj.', 'Anm.'].map(h => H('th', { text: h })))]);
    const tb = H('tbody');
    rows.forEach(r => {
      const tr = H('tr', {}, [
        H('td', { class: 'n', text: r.n }), H('td', { text: r.typ }), H('td', { text: r.plat }),
        H('td', { text: r.matt }), H('td', { text: r.tj }), H('td', { text: (r.anm || '').slice(0, 24) })
      ]);
      tr.onclick = () => { State.select(r.id); const it = State.byId(r.id); if (it) zoomTo(it); };
      tb.appendChild(tr);
    });
    t.appendChild(thead); t.appendChild(tb);
    host.appendChild(t);
  }

  return {
    init: init, syncInputs: syncInputs, rowTable: rowTable, derived: derived,
    objList: objList, props: props, noteTable: noteTable, focusProps: focusProps, zoomTo: zoomTo
  };
})();
