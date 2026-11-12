/* ==========================================================================
   app.js – start och koppling
   ========================================================================== */
'use strict';

(function () {

  function boot() {
    const svg = $('#sheet');

    Render.init(svg);
    State.regen();
    Snap.rebuild(State.geo);

    Panels.init();
    Panels.syncInputs();
    Tools.init(svg);
    Tools.initRailDrag($('#toolrail'));
    PlateView.init();

    /* --- central omritning --- */
    State.on('geo', () => { Snap.rebuild(State.geo); Render.drawAll(); });
    State.on('items', () => {
      Render.drawItems();
      if (State.doc.view.sheet && State.doc.view.table) Render.drawSheet();
    });
    State.on('sel', () => Render.drawOverlayRefresh());
    State.on('meta', () => Render.drawStatic());
    State.on('history', () => {
      $('#b-undo').disabled = !State.canUndo();
      $('#b-redo').disabled = !State.canRedo();
    });
    State.on('tool', t => $$('#toolrail .tool').forEach(b => b.classList.toggle('on', b.dataset.tool === t)));

    /* --- verktygsrad --- */
    $$('#toolrail .tool').forEach(b => b.addEventListener('click', () => State.setTool(b.dataset.tool)));

    /* --- knappar --- */
    $('#b-undo').onclick = () => State.undo();
    $('#b-redo').onclick = () => State.redo();
    $('#b-save').onclick = () => IO.save();
    $('#b-open').onclick = () => IO.openDialog();
    $('#b-new').onclick = () => {
      if (State.doc.items.length && !confirm('Börja om med en tom skiss? Osparat arbete försvinner.')) return;
      State.reset(); Panels.syncInputs(); Render.drawAll(); Render.fit();
    };
    $('#file-input').addEventListener('change', e => {
      if (e.target.files[0]) IO.handleFile(e.target.files[0]);
      e.target.value = '';
    });

    const menu = $('#menu-export');
    $('#b-export').onclick = e => { e.stopPropagation(); menu.hidden = !menu.hidden; };
    document.addEventListener('click', () => menu.hidden = true);
    menu.addEventListener('click', e => {
      const b = e.target.closest('button'); if (!b) return;
      menu.hidden = true;
      const k = b.dataset.ex;
      if (k === 'svg') IO.exportSVG();
      else if (k === 'png') IO.exportPNG();
      else if (k === 'print') IO.print();
      else if (k === 'csv') IO.exportCSV();
    });

    $('#z-in').onclick = () => Render.zoomAt(1.25);
    $('#z-out').onclick = () => Render.zoomAt(1 / 1.25);
    $('#z-fit').onclick = () => Render.fit();

    /* --- första ritning --- */
    Render.drawAll();
    Render.fit();
    State.resetHistory();
    $('#b-undo').disabled = true;
    $('#b-redo').disabled = true;
    Panels.objList(); Panels.noteTable(); Panels.props();
    Panels.rowTable(); Panels.derived();
    State.setTool('select');

    window.addEventListener('resize', () => Render.applyView());
    window.addEventListener('beforeunload', e => {
      if (State.doc.items.length) { e.preventDefault(); e.returnValue = ''; }
    });

    console.log('%cBottenskiss', 'font-weight:700', 'redo. Genvägar: V S D C R P M N O T · Ctrl+Z · 0 = anpassa');
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();
})();
