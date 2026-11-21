// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
// Local report viewer. No network. No audio.

(function () {
  var file = document.getElementById("file");
  var status = document.getElementById("status");
  var out = document.getElementById("out");

  file.addEventListener("change", function () {
    var picked = file.files && file.files[0];
    if (!picked) return;
    var reader = new FileReader();
    reader.onload = function () {
      try {
        render(JSON.parse(String(reader.result)));
      } catch (err) {
        status.textContent = "That file is not the report JSON.";
        out.textContent = "";
      }
    };
    reader.readAsText(picked);
  });

  function render(report) {
    if (!report || report.schema !== "galion-eval-golden/1") {
      status.textContent = "This is not a galion-eval-golden/1 report.";
      out.textContent = "";
      return;
    }
    status.textContent = "claimed=" + String(report.claimed) + ". scores_fabricated=" + String(report.scores_fabricated) + ".";
    var html = "";
    html += "<h2>Rollout</h2>";
    html += "<table><thead><tr><th>Check</th><th>Verdict</th><th>Value</th><th>Gate</th><th>Detail</th></tr></thead><tbody>";
    var trip = report.roundtrip || {};
    ["wer", "ecapa", "emotion_match", "tts_first_audio", "asr_first_partial"].forEach(function (name) {
      html += checkRow(trip[name]);
    });
    html += "</tbody></table>";
    var roll = trip.rollout || {};
    html += "<p>Combined rollout: <span class=\"badge " + esc(roll.status) + "\">" + esc(roll.status) + "</span> pass=" + esc(String(roll.pass)) + ". " + esc(roll.reason || "") + "</p>";
    html += "<p>" + esc(trip.voice_note || "") + "</p>";
    html += "<h2>Golden scripts</h2>";
    html += "<table><thead><tr><th>Script</th><th>Verdict</th><th>User</th><th>Reply</th></tr></thead><tbody>";
    (report.golden || []).forEach(function (turn) {
      var hits = (turn.hits || []).map(function (hit) { return hit.rule + ": " + hit.detail; }).join(" ");
      html += "<tr><td>" + esc(turn.script) + " / " + esc(turn.kind) + "</td><td class=\"badge " + esc(turn.verdict) + "\">" + esc(turn.verdict) + "</td><td>" + esc(turn.user) + "</td><td><span class=\"reply\">" + esc(turn.reply) + "</span>" + (hits ? "<br>" + esc(hits) : "") + "</td></tr>";
    });
    html += "</tbody></table>";
    html += "<h2>Round-trip lines</h2>";
    html += "<table><thead><tr><th>Reference</th><th>Heard</th><th>WER</th><th>First audio ms</th></tr></thead><tbody>";
    (trip.utterances || []).forEach(function (utt) {
      html += "<tr><td>" + esc(utt.reference) + "</td><td>" + esc(utt.hypothesis || "") + "</td><td>" + num(utt.wer) + "</td><td>" + num(utt.first_audio_ms) + "</td></tr>";
    });
    html += "</tbody></table>";
    if (trip.notes && trip.notes.length) {
      html += "<h2>Notes</h2><pre>" + esc(trip.notes.join("\n")) + "</pre>";
    }
    out.innerHTML = html;
  }

  function checkRow(check) {
    if (!check) return "";
    var gate = (check.comparison || "") + " " + num(check.threshold);
    return "<tr><td>" + esc(check.name) + "</td><td class=\"badge " + esc(check.status) + "\">" + esc(check.status) + "</td><td>" + num(check.value) + "</td><td>" + esc(gate) + "</td><td>" + esc(check.detail || "") + "</td></tr>";
  }

  function num(value) {
    if (value === null || value === undefined || value === "") return "null";
    if (typeof value === "number") return String(Math.round(value * 10000) / 10000);
    return esc(String(value));
  }

  function esc(value) {
    return String(value == null ? "" : value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }
})();
