/* ==========================================================================
   render.js – ritar tanken i sidovy.

   Ritningen görs i millimeter och skalas av en viewBox. Samma kod ritar både
   det man ser på skärmen och det som exporteras, så exporten kan inte hamna
   ur fas med skissen.
   ========================================================================== */
'use strict';

const Render = (function () {

  const NS = 'http://www.w3.org/2000/svg';

  /** Marginal nedåt, i millimeter. Uppåt och i sidled räknas den ut. */
  const MARGIN = { bottom: 800 };

  /** Vänstermarginalen ska rymma diametermåttet med sin stående text. */
  function leftMargin() { return 560 + textSize() * 2.2; }

  /** Till höger finns bara måttlinjens ände att ge plats åt. */
  function rightMargin() { return textSize() * 2.4; }

  /** Mått skrivs utan tusentalsavgränsare, som på en ritning. */
  function mm(value) { return String(Math.round(value)); }

  function el(name, attrs, parent) {
    const node = document.createElementNS(NS, name);
    for (const key in attrs) {
      if (attrs[key] === null || attrs[key] === undefined) continue;
      node.setAttribute(key, attrs[key]);
    }
    if (parent) parent.appendChild(node);
    return node;
  }

  /**
   * Höjden ovanför tanken räknas fram ur namnrutans verkliga mått i stället för
   * att vara en fast siffra. Rutan växer med innehållet, och en fast marginal
   * skulle förr eller senare låta den gå in i måttlinjen.
   */
  function topMargin() {
    const block = titleBlockLayout();
    return (-Model.radius() - block.y) + textSize() * 1.8;
  }

  function viewBox() {
    const d = Model.doc;
    const dish = Model.dishDepth();
    const top = topMargin();
    const block = titleBlockLayout();

    const x = -dish - leftMargin();

    // Pa en kort tank ar namnrutan bredare an tanken sjalv. Arket far folja
    // det som racker langst at hoger, annars hamnar rutan utanfor kanten.
    const right = Math.max(d.length + dish, block.x + block.width) + rightMargin();

    return {
      x, y: -Model.radius() - top,
      width: right - x,
      height: d.diameter + top + MARGIN.bottom
    };
  }

  /** Linjetjocklek i mm, så att strecken ser lika grova ut oavsett tankstorlek. */
  function stroke() { return Math.max(6, Model.doc.diameter / 260); }

  // ---------- delarna ----------

  function drawShell(g) {
    const d = Model.doc;
    const r = Model.radius();
    const dish = Model.dishDepth();
    const w = stroke();

    // Manteln som en rektangel, gavlarna som halva ellipser utanför.
    el('path', {
      d: `M 0 ${-r} L ${d.length} ${-r}
          A ${dish} ${r} 0 0 1 ${d.length} ${r}
          L 0 ${r}
          A ${dish} ${r} 0 0 1 0 ${-r} Z`,
      fill: '#ffffff', stroke: '#1a1a1a', 'stroke-width': w,
      'stroke-linejoin': 'round'
    }, g);
  }

  function drawRings(g, interactive) {
    const r = Model.radius();
    const w = stroke();

    Model.doc.rings.forEach((x, i) => {
      const line = el('line', {
        x1: x, y1: -r, x2: x, y2: r,
        stroke: '#1a1a1a', 'stroke-width': w
      }, g);

      if (!interactive) return;

      const edge = i === 0 || i === Model.doc.rings.length - 1;
      if (edge) return;

      line.dataset.ring = i;
      line.classList.add('grip');
      line.style.cursor = 'ew-resize';

      // Ett brett osynligt band gor skarven latt att traffa med musen.
      const grab = el('line', {
        x1: x, y1: -r, x2: x, y2: r,
        stroke: 'transparent', 'stroke-width': Math.max(w * 6, 60)
      }, g);
      grab.dataset.ring = i;
      grab.classList.add('grip');
      grab.style.cursor = 'ew-resize';
    });
  }

  function drawLongs(g, interactive) {
    const w = stroke();

    Model.doc.longs.forEach((seam, i) => {
      const span = Model.courseSpan(i);
      if (!span || span.to === undefined) return;

      const y = seam.side === 'up' ? -seam.offset : seam.offset;

      el('line', {
        x1: span.from, y1: y, x2: span.to, y2: y,
        stroke: '#1a1a1a', 'stroke-width': w
      }, g);

      if (!interactive) return;

      const grab = el('line', {
        x1: span.from, y1: y, x2: span.to, y2: y,
        stroke: 'transparent', 'stroke-width': Math.max(w * 6, 60)
      }, g);
      grab.dataset.long = i;
      grab.classList.add('grip');
      grab.style.cursor = 'ns-resize';
    });
  }

  /** Anslutningar, påstick och manluckor ritas som symboler på manteln. */
  function drawFittings(g, interactive, selected) {
    const r = Model.radius();
    const w = stroke();

    Model.doc.fittings.forEach((item, i) => {
      const up = item.side === 'up';
      const y = up ? -r : r;
      const dir = up ? -1 : 1;

      const size = {
        flans:    { neck: r * 0.16, width: r * 0.26, flange: r * 0.42 },
        pastick:  { neck: r * 0.14, width: r * 0.15, flange: r * 0.22 },
        manlucka: { neck: r * 0.18, width: r * 0.52, flange: r * 0.68 }
      }[item.kind] || { neck: r * 0.16, width: r * 0.26, flange: r * 0.42 };

      const group = el('g', {}, g);
      if (selected === i) group.classList.add('selected');

      // Halsen
      el('rect', {
        x: item.x - size.width / 2,
        y: up ? y - size.neck : y,
        width: size.width,
        height: size.neck,
        fill: '#ffffff', stroke: '#1a1a1a', 'stroke-width': w
      }, group);

      // Flänsen ytterst
      el('line', {
        x1: item.x - size.flange / 2, y1: y + dir * size.neck,
        x2: item.x + size.flange / 2, y2: y + dir * size.neck,
        stroke: '#1a1a1a', 'stroke-width': w * 1.6, 'stroke-linecap': 'round'
      }, group);

      if (item.kind === 'manlucka') {
        el('line', {
          x1: item.x - size.width / 2, y1: y + dir * size.neck * 0.45,
          x2: item.x + size.width / 2, y2: y + dir * size.neck * 0.45,
          stroke: '#1a1a1a', 'stroke-width': w
        }, group);
      }

      if (!interactive) return;

      const grab = el('rect', {
        x: item.x - size.flange / 2 - 30,
        y: up ? y - size.neck - 60 : y - 20,
        width: size.flange + 60,
        height: size.neck + 80,
        fill: 'transparent'
      }, group);
      grab.dataset.fitting = i;
      grab.classList.add('grip');
    });
  }

  /** Textstorlek för mått och namnruta, i millimeter. */
  function textSize() {
    return Math.min(Math.max(Model.radius() * 0.13, 90), 210);
  }

  /** Ett måttstreck med korta vinkelmarkeringar i ändarna. */
  function tickMark(g, x, y, angle, size, width) {
    const rad = angle * Math.PI / 180;
    const dx = Math.cos(rad) * size, dy = Math.sin(rad) * size;
    el('line', { x1: x - dx, y1: y - dy, x2: x + dx, y2: y + dy,
                 stroke: '#1a1a1a', 'stroke-width': width }, g);
  }

  /** Måttlinjens höjd över manteln. */
  function lengthDimY() { return -Model.radius() - textSize() * 3; }

  /** Längden, från första till sista rundskarv. */
  function drawLengthDimension(g) {
    const d = Model.doc;
    const r = Model.radius();
    const y = lengthDimY();
    const w = stroke() * 0.65;
    const size = textSize();

    el('line', { x1: 0, y1: y, x2: d.length, y2: y,
                 stroke: '#1a1a1a', 'stroke-width': w }, g);

    [0, d.length].forEach(x => {
      tickMark(g, x, y, 60, size * 0.45, w * 1.4);
      el('line', { x1: x, y1: y + size * 0.35, x2: x, y2: -r,
                   stroke: '#1a1a1a', 'stroke-width': w * 0.8 }, g);
    });

    const text = el('text', {
      x: d.length / 2, y: y - size * 0.42,
      'text-anchor': 'middle',
      'font-family': 'Segoe UI, Arial, sans-serif',
      'font-size': size, fill: '#1a1a1a'
    }, g);
    text.textContent = mm(d.length);
  }

  /** Diametern, till vänster om tanken. */
  function drawDiameterDimension(g) {
    const d = Model.doc;
    const r = Model.radius();
    const dish = Model.dishDepth();
    const x = -dish - 560;
    const w = stroke() * 0.65;
    const size = textSize();

    el('line', { x1: x, y1: -r, x2: x, y2: r,
                 stroke: '#1a1a1a', 'stroke-width': w }, g);

    [-r, r].forEach(y => {
      tickMark(g, x, y, -30, size * 0.45, w * 1.4);
      // Hjalplinjen gar fram till manteln, forbi gaveln.
      el('line', { x1: x - size * 0.35, y1: y, x2: 0, y2: y,
                   stroke: '#1a1a1a', 'stroke-width': w * 0.8 }, g);
    });

    // Texten star upp och ner langs mattlinjen, som pa en ritning.
    const text = el('text', {
      x: 0, y: 0,
      'text-anchor': 'middle',
      'font-family': 'Segoe UI, Arial, sans-serif',
      'font-size': size, fill: '#1a1a1a',
      transform: `translate(${x - size * 0.42} 0) rotate(-90)`
    }, g);
    text.textContent = 'Ø' + mm(d.diameter);
  }

  // ---------- namnrutan ----------

  /**
   * Namnrutan byggs som en riktig ritningsstämpel: beteckningen på en rad och
   * måtten i egna fack under, var och en med sin rubrik. Ett fack per uppgift
   * gör den läsbar på en utskrift, till skillnad från en rad löptext.
   *
   * Bredderna uppskattas ur teckenantalet. Att mäta texten skulle kräva att
   * den redan låg i dokumentet, och samma kod ritar både skärm och export.
   */
  function titleBlockLayout() {
    const d = Model.doc;
    const size = textSize();
    const caption = size * 0.58;
    const value = size * 1.02;
    const pad = size * 0.6;

    const cells = [
      { caption: 'DIAMETER',    value: 'Ø' + mm(d.diameter), unit: 'mm' },
      { caption: 'LÄNGD', value: mm(d.length),               unit: 'mm' },
      { caption: 'RUNDSKARVAR', value: String(d.seamCount) },
      { caption: 'LÅNGSKARVAR', value: String(Model.longCount()) },
      { caption: 'KORSPUNKTER', value: String(Model.crossPoints()) }
    ];

    cells.forEach(cell => {
      const valueText = cell.unit ? cell.value + ' ' + cell.unit : cell.value;
      cell.text = valueText;
      cell.width = Math.max(cell.caption.length * caption * 0.68,
                            valueText.length * value * 0.6) + pad * 2;
    });

    const label = d.title || '';
    const header = label ? size * 1.75 : 0;
    const row = caption * 1.5 + value * 1.6;

    let width = cells.reduce((sum, c) => sum + c.width, 0);
    const needed = label.length * size * 0.55 + pad * 2;
    if (needed > width) {
      // Bred beteckning: bredda facken jamnt sa att ramen halls rak.
      const extra = (needed - width) / cells.length;
      cells.forEach(c => { c.width += extra; });
      width = needed;
    }

    const height = header + row;

    // Rutan hangs ovanfor mattlinjen med luft emellan, sa att siffrorna aldrig
    // moter varandra hur mycket texten an vaxer.
    const y = lengthDimY() - size * 2.2 - height;

    return { x: -Model.dishDepth(), y, width, height,
             header, row, cells, size, caption, value, pad };
  }

  function drawTitleBlock(g) {
    const b = titleBlockLayout();
    const w = stroke();
    const frame = '#1a1a1a';

    const box = el('g', {}, g);

    // Rubrikbandet far en svag ton sa att beteckningen loser sig fran facken.
    if (b.header > 0) {
      el('rect', { x: b.x, y: b.y, width: b.width, height: b.header,
                   fill: '#eef1f5' }, box);
    }

    el('rect', { x: b.x, y: b.y + b.header, width: b.width, height: b.row,
                 fill: '#ffffff' }, box);

    // Facken
    let x = b.x;
    b.cells.forEach((cell, i) => {
      if (i > 0) {
        el('line', { x1: x, y1: b.y + b.header, x2: x, y2: b.y + b.height,
                     stroke: '#c3c9d2', 'stroke-width': w * 0.35 }, box);
      }

      const caption = el('text', {
        x: x + b.pad, y: b.y + b.header + b.caption * 1.25,
        'font-family': 'Segoe UI, Arial, sans-serif',
        'font-size': b.caption, 'font-weight': '600',
        'letter-spacing': b.caption * 0.08,
        fill: '#6b7280'
      }, box);
      caption.textContent = cell.caption;

      const value = el('text', {
        x: x + b.pad, y: b.y + b.height - b.value * 0.42,
        'font-family': 'Segoe UI, Arial, sans-serif',
        'font-size': b.value, 'font-weight': '600', fill: '#1a1a1a'
      }, box);
      value.textContent = cell.text;

      x += cell.width;
    });

    // Beteckningen overst
    if (b.header > 0) {
      const title = el('text', {
        x: b.x + b.pad, y: b.y + b.header - b.size * 0.46,
        'font-family': 'Segoe UI, Arial, sans-serif',
        'font-size': b.size, 'font-weight': '600', fill: '#1a1a1a'
      }, box);
      title.textContent = Model.doc.title;

      el('line', { x1: b.x, y1: b.y + b.header, x2: b.x + b.width, y2: b.y + b.header,
                   stroke: frame, 'stroke-width': w * 0.5 }, box);
    }

    // Ramen sist, sa att den ligger over fyllningarna.
    el('rect', { x: b.x, y: b.y, width: b.width, height: b.height,
                 fill: 'none', stroke: frame, 'stroke-width': w * 0.7 }, box);
  }

  // ---------- ritning ----------

  function draw(svg, options) {
    const interactive = options && options.interactive;
    const selected = options ? options.selected : -1;

    while (svg.firstChild) svg.removeChild(svg.firstChild);

    const box = viewBox();
    svg.setAttribute('viewBox', `${box.x} ${box.y} ${box.width} ${box.height}`);
    svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');

    el('rect', { x: box.x, y: box.y, width: box.width, height: box.height,
                 fill: '#ffffff' }, svg);

    const g = el('g', {}, svg);

    drawTitleBlock(g);
    drawLengthDimension(g);
    drawDiameterDimension(g);
    drawShell(g);
    drawRings(g, interactive);
    drawLongs(g, interactive);
    drawFittings(g, interactive, selected);

    return box;
  }

  return { draw, viewBox, NS };
})();
