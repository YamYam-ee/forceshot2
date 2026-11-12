/* ==========================================================================
   io.js – spara, öppna, exportera (SVG / PNG / utskrift / CSV)
   ========================================================================== */
'use strict';

const IO = (function () {

  function baseName() {
    const m = State.doc.meta;
    const s = [m.objekt, m.cistern].filter(Boolean).join('_') || 'bottenskiss';
    return s.replace(/[^\wåäöÅÄÖ\-]+/g, '_').slice(0, 60);
  }

  function download(blob, name) {
    const url = URL.createObjectURL(blob);
    const a = H('a', { href: url, download: name });
    document.body.appendChild(a); a.click();
    setTimeout(() => { URL.revokeObjectURL(url); a.remove(); }, 400);
  }

  /* ---------- projekt ---------- */
  function save() {
    const blob = new Blob([JSON.stringify(State.doc, null, 1)], { type: 'application/json' });
    download(blob, baseName() + '.botten.json');
    toast('Projekt sparat', 'ok');
  }
  function openDialog() { $('#file-input').click(); }
  function handleFile(file) {
    const fr = new FileReader();
    fr.onload = () => {
      try {
        const d = JSON.parse(fr.result);
        if (!d.tank || !d.layout) throw new Error('fel format');
        State.load(d);
        Panels.syncInputs();
        Render.drawAll(); Render.fit();
        toast('Projekt öppnat', 'ok');
      } catch (err) { toast('Kunde inte läsa filen: ' + err.message, 'warn'); }
    };
    fr.readAsText(file);
  }

  /* ---------- export ---------- */
  function svgString(scale) {
    const node = Render.exportSVG(scale || 1);
    node.setAttribute('xmlns', SVGNS);
    node.setAttribute('xmlns:xlink', 'http://www.w3.org/1999/xlink');
    return new XMLSerializer().serializeToString(node);
  }

  function exportSVG() {
    const blob = new Blob(['<?xml version="1.0" encoding="UTF-8"?>\n' + svgString(1)], { type: 'image/svg+xml;charset=utf-8' });
    download(blob, baseName() + '.svg');
    toast('SVG exporterad', 'ok');
  }

  function exportPNG() {
    const px = 4000;
    const node = Render.exportSVG(1);
    const vb = node.getAttribute('viewBox').split(' ').map(Number);
    const w = px, h = Math.round(px * vb[3] / vb[2]);
    node.setAttribute('width', w); node.setAttribute('height', h);
    const str = new XMLSerializer().serializeToString(node);
    const img = new Image();
    img.onload = () => {
      const cv = document.createElement('canvas');
      cv.width = w; cv.height = h;
      const ctx = cv.getContext('2d');
      ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, w, h);
      ctx.drawImage(img, 0, 0, w, h);
      cv.toBlob(b => { download(b, baseName() + '.png'); toast('PNG exporterad (' + w + '×' + h + ')', 'ok'); }, 'image/png');
    };
    img.onerror = () => toast('Kunde inte skapa PNG – prova SVG', 'warn');
    img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(str);
  }

  function print() {
    let host = $('#print-host');
    if (!host) { host = H('div', { id: 'print-host' }); document.body.appendChild(host); }
    host.innerHTML = '';
    const node = Render.exportSVG(1);
    node.removeAttribute('width'); node.removeAttribute('height');
    node.setAttribute('style', 'width:100%;height:100%');
    host.appendChild(node);
    host.style.display = 'block';
    setTimeout(() => {
      window.print();
      setTimeout(() => { host.style.display = 'none'; host.innerHTML = ''; }, 500);
    }, 60);
  }

  function exportCSV() {
    const rows = reportRows();
    if (!rows.length) { toast('Inga anmärkningar att exportera', 'warn'); return; }
    const m = State.doc.meta;
    const head = ['Nr', 'Typ', 'Plåt', 'Mått (mm)', 'Tjocklek', 'Anmärkning'];
    const esc = v => '"' + String(v == null ? '' : v).replace(/"/g, '""') + '"';
    let out = 'sep=;\n';
    out += [esc('Objekt'), esc(m.objekt), esc('Cistern'), esc(m.cistern), esc('Datum'), esc(m.datum)].join(';') + '\n\n';
    out += head.map(esc).join(';') + '\n';
    rows.forEach(r => { out += [r.n, r.typ, r.plat, r.matt, r.tj, r.anm].map(esc).join(';') + '\n'; });
    download(new Blob(['﻿' + out], { type: 'text/csv;charset=utf-8' }), baseName() + '_anmarkningar.csv');
    toast('Tabell exporterad', 'ok');
  }

  return {
    save: save, openDialog: openDialog, handleFile: handleFile,
    exportSVG: exportSVG, exportPNG: exportPNG, print: print, exportCSV: exportCSV
  };
})();
