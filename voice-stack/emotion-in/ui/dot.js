// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

// One dot. Colour is #F5F5F7 scaled by brightness. The wash is the page
// background. ?debug=1 shows the JSON; the default screen does not.
(function () {
  var params = new URLSearchParams(location.search);
  var debug = params.get("debug") === "1";
  var dot = document.getElementById("dot");
  var dbg = document.getElementById("dbg");
  if (debug) dbg.hidden = false;

  function tick() {
    fetch("/dot", { cache: "no-store" })
      .then(function (r) { return r.json(); })
      .then(function (j) {
        var side = Math.min(window.innerWidth, window.innerHeight);
        var d = Math.max(8, j.radius * side * 2);
        dot.style.width = d + "px";
        dot.style.height = d + "px";
        var k = j.brightness;
        if (!(k >= 0 && k <= 1)) k = 0;
        var r = Math.round(0xf5 * k);
        var g = Math.round(0xf5 * k);
        var b = Math.round(0xf7 * k);
        dot.style.background = "rgb(" + r + "," + g + "," + b + ")";
        if (typeof j.wash === "string") document.body.style.background = j.wash;
        if (debug) dbg.textContent = JSON.stringify(j, null, 2);
      })
      .catch(function () {});
  }

  tick();
  setInterval(tick, 80);
})();
