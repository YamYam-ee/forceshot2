/* ==========================================================================
   app.js – panelen, dragen och exporten.
   ========================================================================== */
'use strict';

(function () {

  const svg = document.getElementById('sheet');
  const status = document.getElementById('status');

  let selected = -1;        // markerat tillbehör
  let pendingKind = null;   // tillbehör som väntar på att placeras
  let drag = null;

  /** Exportens bredd i bildpunkter. Ger en läsbar skiss utan orimlig fil. */
  const EXPORT_WIDTH = 2600;

  // ---------------------------------------------------------------

  function redraw() {
    Render.draw(svg, { interactive: true, selected });
    updateStatus();
  }

  function updateStatus() {
    const d = Model.doc;
    const longs = Model.longCount();
    const up = d.longs.filter(s => s.side === 'up').length;

    status.textContent =
      `Ø${d.diameter} mm · ${d.length} mm mellan gavelsvetsarna · ` +
      `${d.seamCount} rundskarvar, ${longs} långskarvar, ` +
      `${Model.crossPoints()} korspunkter · ` +
      `${up} långskarvar uppe, ${longs - up} nere`;

    document.getElementById('course-hint').textContent = `ger ${longs} långskarvar`;
    document.getElementById('scale-note').textContent =
      `Skickas med skalan ${pixelsPerCm().toFixed(2)} bildpunkter/cm, ` +
      'så filmmåtten stämmer direkt i redigeraren.';
  }

  /**
   * Hur många bildpunkter en centimeter blir i den exporterade bilden.
   * Bildredigeraren använder det för att lägga filmerna i rätt storlek utan
   * att någon behöver kalibrera för hand.
   */
  function pixelsPerCm() {
    const box = Render.viewBox();
    const pixelsPerMm = EXPORT_WIDTH / box.width;
    return pixelsPerMm * 10;
  }

  // ---------------------------------------------------------------
  // Panelen
  // ---------------------------------------------------------------

  function bindNumber(id, apply) {
    const input = document.getElementById(id);
    input.addEventListener('change', () => {
      const value = parseFloat(input.value);
      if (!isFinite(value)) return;
      apply(value);
      redraw();
    });
  }

  bindNumber('diameter', v => {
    Model.doc.diameter = Math.max(200, v);
    Model.rebuild('alternate');
  });

  bindNumber('length', v => {
    Model.doc.length = Math.max(500, v);
    Model.rebuild('alternate');
  });

  bindNumber('seams', v => Model.setSeamCount(v));

  bindNumber('dish', v => { Model.doc.dishPercent = Math.max(5, Math.min(50, v)); });

  bindNumber('offset', v => {
    Model.doc.offsetPercent = Math.max(0, Math.min(95, v));
    const base = Model.doc.offsetPercent / 100 * Model.radius();
    Model.doc.longs.forEach(seam => { seam.offset = base; });
  });

  document.getElementById('start-side').addEventListener('change', e => {
    Model.doc.startSide = e.target.value;
    Model.rebuild('alternate');
    redraw();
  });

  document.getElementById('title').addEventListener('input', e => {
    Model.doc.title = e.target.value;
    redraw();
  });

  document.getElementById('alt-every').addEventListener('click', () => {
    Model.rebuild('alternate'); redraw();
  });
  document.getElementById('alt-all-up').addEventListener('click', () => {
    Model.setAllSides('up'); redraw();
  });
  document.getElementById('alt-all-down').addEventListener('click', () => {
    Model.setAllSides('down'); redraw();
  });
  document.getElementById('reset').addEventListener('click', () => {
    Model.resetRings(); redraw();
  });

  document.querySelectorAll('[data-add]').forEach(button => {
    button.addEventListener('click', () => {
      const kind = button.dataset.add;
      pendingKind = pendingKind === kind ? null : kind;

      document.querySelectorAll('[data-add]').forEach(other => {
        other.classList.toggle('active', other.dataset.add === pendingKind);
      });
    });
  });

  // ---------------------------------------------------------------
  // Musen
  // ---------------------------------------------------------------

  /** Skärmkoordinat till millimeter i modellen. */
  function toModel(event) {
    const point = svg.createSVGPoint();
    point.x = event.clientX;
    point.y = event.clientY;
    return point.matrixTransform(svg.getScreenCTM().inverse());
  }

  svg.addEventListener('pointerdown', event => {
    const at = toModel(event);
    const data = event.target.dataset || {};

    if (pendingKind) {
      const side = at.y < 0 ? 'up' : 'down';
      selected = Model.addFitting(pendingKind, at.x, side);
      pendingKind = null;
      document.querySelectorAll('[data-add]').forEach(b => b.classList.remove('active'));
      redraw();
      return;
    }

    if (data.ring !== undefined) {
      drag = { kind: 'ring', index: +data.ring };
    } else if (data.long !== undefined) {
      drag = { kind: 'long', index: +data.long };
    } else if (data.fitting !== undefined) {
      drag = { kind: 'fitting', index: +data.fitting };
      selected = +data.fitting;
      redraw();
    } else {
      selected = -1;
      redraw();
      return;
    }

    svg.setPointerCapture(event.pointerId);
    event.preventDefault();
  });

  svg.addEventListener('pointermove', event => {
    if (!drag) return;
    const at = toModel(event);

    if (drag.kind === 'ring') Model.moveRing(drag.index, at.x);
    else if (drag.kind === 'long') Model.moveLong(drag.index, at.y, event.shiftKey);
    else if (drag.kind === 'fitting') {
      Model.moveFitting(drag.index, at.x, at.y < 0 ? 'up' : 'down');
    }

    redraw();
  });

  svg.addEventListener('pointerup', event => {
    if (!drag) return;
    drag = null;
    try { svg.releasePointerCapture(event.pointerId); } catch { /* redan slappt */ }
  });

  window.addEventListener('keydown', event => {
    if (event.key !== 'Delete' && event.key !== 'Backspace') return;
    if (document.activeElement && document.activeElement.tagName === 'INPUT') return;
    if (selected < 0) return;

    Model.removeFitting(selected);
    selected = -1;
    redraw();
    event.preventDefault();
  });

  // ---------------------------------------------------------------
  // Export
  // ---------------------------------------------------------------

  function exportPng() {
    // Ritas om utan handtag - de hor till skissverktyget, inte till bilden.
    const clean = document.createElementNS(Render.NS, 'svg');
    const box = Render.draw(clean, { interactive: false });

    const width = EXPORT_WIDTH;
    const height = Math.round(width * box.height / box.width);

    clean.setAttribute('xmlns', Render.NS);
    clean.setAttribute('width', width);
    clean.setAttribute('height', height);

    const markup = new XMLSerializer().serializeToString(clean);
    const image = new Image();

    image.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;

      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, width, height);
      ctx.drawImage(image, 0, 0, width, height);

      canvas.toBlob(blob => {
        tellHost();
        download(blob);
      }, 'image/png');
    };

    image.onerror = () => {
      status.textContent = 'Bilden kunde inte skapas.';
    };

    image.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(markup);
  }

  /**
   * Talar om skalan för ForceShot innan bilden kommer.
   *
   * Utan den skulle användaren behöva dra i den första filmen tills den
   * stämmer mot skissen. Nu vet redigeraren redan hur många bildpunkter en
   * centimeter är, och filmerna hamnar rätt direkt.
   */
  function tellHost() {
    try {
      // ForceShot 2 lägger in en egen brygga som fungerar likadant på Windows och Mac.
      if (window.forceshot && window.forceshot.post) {
        window.forceshot.post({
          forceshot: 'skala',
          bildpunkterPerCm: pixelsPerCm(),
          namn: Model.doc.title
        });
      }
    } catch { /* korsen utanfor ForceShot fungerar anda */ }
  }

  function download(blob) {
    const name = (Model.doc.title || 'tankskiss')
      .replace(/[^\wåäöÅÄÖ\- ]+/g, '_')
      .slice(0, 60);

    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = name + '.png';
    document.body.appendChild(link);
    link.click();

    setTimeout(() => { URL.revokeObjectURL(url); link.remove(); }, 500);
  }

  document.getElementById('export').addEventListener('click', exportPng);

  // ---------------------------------------------------------------

  window.addEventListener('resize', redraw);
  redraw();
})();
