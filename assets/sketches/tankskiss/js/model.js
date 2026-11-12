/* ==========================================================================
   model.js – tankens mått och skarvar.

   Allt räknas i millimeter med origo i tankens centrumlinje, vid den första
   rundskarven. Det gör att en flyttad skarv aldrig påverkar var något annat
   sitter, och att måtten som skickas till bildredigeraren är verkliga mått.
   ========================================================================== */
'use strict';

const Model = (function () {

  const doc = {
    diameter: 2400,
    length: 6560,          // första till sista rundskarv
    seamCount: 5,          // rundskarvar, inklusive de två gavelsvetsarna
    dishPercent: 25,       // gaveldjup i procent av diametern
    offsetPercent: 45,     // långskarvens avstånd från centrum, i procent av radien
    startSide: 'up',
    title: 'Tank T3 LR 125 vakuum, Y-7641',

    /** Rundskarvarnas x-läge i mm. Auto tills användaren dragit i dem. */
    rings: [],

    /** En per mantelring: { side: 'up'|'down', offset: mm från centrum } */
    longs: [],

    /** { kind, x, side } där side är 'up' | 'down'. */
    fittings: []
  };

  function radius() { return doc.diameter / 2; }
  function dishDepth() { return doc.diameter * doc.dishPercent / 100; }
  function courseCount() { return Math.max(1, doc.seamCount - 1); }

  /** En långskarv per mantelring. */
  function longCount() { return doc.longs.length || courseCount(); }

  /**
   * Korspunkter: varje långskarv slutar mot en rundskarv i var ände, så det
   * blir två per långskarv. Det är de punkterna som ska provas extra.
   */
  function crossPoints() { return longCount() * 2; }

  /** Jämnt fördelade rundskarvar. */
  function autoRings() {
    const n = doc.seamCount;
    const step = doc.length / (n - 1);
    return Array.from({ length: n }, (_, i) => Math.round(i * step));
  }

  /**
   * Långskarvarna, en per mantelring. Sidan alternerar från startsidan –
   * så ligger svetsen aldrig i linje med grannringens, vilket är hela
   * poängen med att förskjuta dem.
   */
  function autoLongs(pattern) {
    const n = courseCount();
    const base = doc.offsetPercent / 100 * radius();

    return Array.from({ length: n }, (_, i) => {
      let side;
      if (pattern === 'up') side = 'up';
      else if (pattern === 'down') side = 'down';
      else {
        const flip = i % 2 === 1;
        side = (doc.startSide === 'up') === !flip ? 'up' : 'down';
      }
      return { side, offset: base };
    });
  }

  /** Bygger om skarvarna från inställningarna. */
  function rebuild(pattern) {
    doc.rings = autoRings();
    doc.longs = autoLongs(pattern || 'alternate');
    clampFittings();
  }

  /** Behåller sidor och offset men lägger ut rundskarvarna jämnt igen. */
  function resetRings() {
    doc.rings = autoRings();
    clampFittings();
  }

  function setSeamCount(count) {
    doc.seamCount = Math.max(2, Math.min(24, Math.round(count)));
    rebuild('alternate');
  }

  /** Flyttar en rundskarv. Ändarna sitter fast – de är gavelsvetsarna. */
  function moveRing(index, x) {
    if (index <= 0 || index >= doc.rings.length - 1) return;

    const min = doc.rings[index - 1] + doc.diameter * 0.06;
    const max = doc.rings[index + 1] - doc.diameter * 0.06;
    doc.rings[index] = Math.round(Math.max(min, Math.min(max, x)));
    clampFittings();
  }

  /**
   * Flyttar en långskarv i höjdled. Med `together` följer alla skarvar på
   * samma sida med – de ligger normalt på samma höjd runt hela tanken, och
   * då vill man flytta dem som en enhet.
   */
  function moveLong(index, y, together) {
    const seam = doc.longs[index];
    if (!seam) return;

    const side = y < 0 ? 'up' : 'down';
    const offset = Math.min(Math.abs(y), radius() * 0.95);

    if (together) {
      const from = seam.side;
      doc.longs.forEach(other => {
        if (other.side === from) { other.side = side; other.offset = offset; }
      });
    } else {
      seam.side = side;
      seam.offset = offset;
    }
  }

  function setAllSides(side) {
    doc.longs.forEach(seam => { seam.side = side; });
  }

  function courseSpan(index) {
    return { from: doc.rings[index], to: doc.rings[index + 1] };
  }

  // ---------- tillbehör ----------

  function addFitting(kind, x, side) {
    doc.fittings.push({
      kind,
      x: Math.max(0, Math.min(doc.length, Math.round(x))),
      side: side || 'up'
    });
    return doc.fittings.length - 1;
  }

  function moveFitting(index, x, side) {
    const item = doc.fittings[index];
    if (!item) return;
    item.x = Math.max(0, Math.min(doc.length, Math.round(x)));
    if (side) item.side = side;
  }

  function removeFitting(index) {
    if (index >= 0 && index < doc.fittings.length) doc.fittings.splice(index, 1);
  }

  function clampFittings() {
    doc.fittings.forEach(f => { f.x = Math.max(0, Math.min(doc.length, f.x)); });
  }

  /** Bredden på hela ritningen inklusive gavlar, i mm. */
  function totalWidth() { return doc.length + dishDepth() * 2; }

  rebuild('alternate');

  return {
    doc, radius, dishDepth, courseCount, longCount, crossPoints, courseSpan, totalWidth,
    rebuild, resetRings, setSeamCount, moveRing, moveLong, setAllSides,
    addFitting, moveFitting, removeFitting
  };
})();
