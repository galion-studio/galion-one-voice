#!/usr/bin/env python3
"""Agent 3 rescore harness: OLD (universal, reused) vs NEW (persona scorer).

Own files only under scorer-probe/results-persona/. Does not edit scorers/ or pipeline/.
No deploy / push / merge. Never fabricates numbers.
"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from datetime import datetime, timezone, timedelta
from pathlib import Path

ROOT = Path("/workspace/swarm/voice-v2-sprint/cloning-v2")
PROBE = ROOT / "scorer-probe"
OUT = PROBE / "results-persona"
PERSONA_PY = ROOT / "scorers/persona/py"
UNI_RES = ROOT / "scorers/universal/results"
# torch/transformers live in the universal scorer venv (same as score_universal.py)
PY = str(ROOT / "scorers/universal/venv/bin/python")
ZAGREB = timezone(timedelta(hours=1), name="Europe/Zagreb")
GATE = 0.83
PRESENCE = 0.2

# Probe group -> catalog emotion id used by persona
GROUP_TO_CAT = {
    "crying": "crying",
    "seductive": "seductive",
    "love": "love",
    "laughter": "laughter",
    "joyful": "joyful",
    "aggressive": "aggressive",
    "disgust": "disgust",
    "fear": "fear",
    "sad": "sad",
    "surprise": "surprise",
    "neutral": "neutral",
}

UNIVERSAL_PRIMARY = {
    "crying": ("efficientat_max", "crying"),
    "seductive": ("clap_probs", "seductive"),
    "love": ("clap_probs", "love"),
    "laughter": ("efficientat_max", "laughter"),
    "joyful": ("e2v", "happy"),
    "aggressive": ("e2v", "angry"),
    "disgust": ("e2v", "disgusted"),
    "fear": ("e2v", "fearful"),
    "sad": ("e2v", "sad"),
    "surprise": ("e2v", "surprised"),
    "neutral": ("e2v", "neutral"),
}


def now():
    return datetime.now(ZAGREB).isoformat()


def run(cmd, log_path: Path):
    log_path.parent.mkdir(parents=True, exist_ok=True)
    print(f"[run] {' '.join(str(c) for c in cmd)}", flush=True)
    with open(log_path, "w") as log:
        log.write(f"$ {' '.join(str(c) for c in cmd)}\n")
        log.flush()
        p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        log.write(p.stdout)
        log.write(f"\n[exit {p.returncode}]\n")
    if p.returncode != 0:
        raise RuntimeError(f"command failed rc={p.returncode}: {cmd}\n{p.stdout[-2000:]}")
    return p.stdout


def ensure_old():
    old = OUT / "old" / "OLD.json"
    if not old.exists():
        run([sys.executable, str(OUT / "extract_old.py")], OUT / "logs" / "extract_old.log")
    return json.loads(old.read_text())


def run_persona_match(voice: str) -> Path:
    """Use Agent-3 path-cached wrapper (same math as persona_match.py).
    Avoids re-running love_seductive CLAP hundreds of times."""
    work = UNI_RES / f"{voice}-universal"
    out = OUT / "new" / f"{voice}-persona-match.json"
    cmd = [
        PY, str(OUT / "persona_match_cached.py"),
        "--persona", f"own-{voice}",
        "--reuse-windows", str(work / "base_windows_universal.json"),
        "--reuse-lines", f"merge-v2={work / 'lines_merge-v2.json'}",
        "--reuse-lines", f"coherent={work / 'lines_global-fix-coherent.json'}",
        "--out", str(out),
    ]
    run(cmd, OUT / "logs" / f"persona_match_{voice}.log")
    return out


def run_persona_probe(voice: str) -> Path:
    wav_dir = PROBE / "wav" / voice
    wavs = sorted(wav_dir.glob("*.wav"))
    out = OUT / "new" / f"{voice}-persona-probe.json"
    cmd = [
        PY, str(PERSONA_PY / "score_persona.py"),
        *[str(w) for w in wavs],
        "--persona", f"own-{voice}",
        "--reuse-json", str(PROBE / "results" / "probe_universal_all.json"),
        "--out", str(out),
    ]
    run(cmd, OUT / "logs" / f"persona_probe_{voice}.log")
    return out


def pack_match_block(block: dict | None) -> dict | None:
    if block is None:
        return None
    em = block.get("emotion_match")
    per = {}
    for row in block.get("leaderboard") or []:
        per[row["category"]] = {
            "score": row["score"],
            "strength": row["strength"],
            "source": row.get("source"),
            "best_line": row.get("best_line"),
            "best_match": row.get("best_match"),
            "mean_match": row.get("mean_match"),
        }
    return {
        "emotion_match": em,
        "category_blend": block.get("category_blend"),
        "emb_sim": block.get("emb_sim"),
        "gate_0_83": block.get("pass"),
        "gate": block.get("gate", GATE),
        "judged": block.get("judged"),
        "na_categories": block.get("na_categories"),
        "per_emotion": per,
        "formula": block.get("formula"),
    }


def build_comparison(old: dict, match_paths: dict, probe_paths: dict) -> dict:
    voices = {}
    for voice in ("samantha", "jarvis"):
        old_v = old["voices"][voice]
        match = json.loads(match_paths[voice].read_text())
        probe_new = json.loads(probe_paths[voice].read_text())

        # --- gate sets ---
        sets_cmp = {}
        # ceiling
        old_ceil = old_v["sets"]["ceiling"]
        new_ceil = pack_match_block(match.get("BASE-self-ceiling"))
        sets_cmp["ceiling"] = compare_set("ceiling", old_ceil, new_ceil, old_v["sets"]["ceiling"].get("moments_present"))

        for sname in ("merge-v2", "coherent"):
            old_s = old_v["sets"][sname]
            new_s = pack_match_block((match.get("sets") or {}).get(sname))
            sets_cmp[sname] = compare_set(sname, old_s, new_s, None)

        # --- probe ---
        # index new files by stem
        new_by = {}
        for rec in probe_new.get("files") or []:
            fp = rec.get("file") or rec.get("path") or ""
            new_by[Path(fp).stem] = rec

        persona_judged = set((match.get("persona") or {}).get("judged") or [])
        probe_lines = []
        per_emotion = {}
        for line in old_v["probe"]["lines"]:
            group = line["group"]
            cat = GROUP_TO_CAT[group]
            old_score = line["primary_score"]
            old_det = line["detected_ge_0.2"]
            new_rec = new_by.get(line["id"])
            if new_rec is None:
                new_score = None
                new_det = None
                new_source = None
                persona_val = "MISSING_NEW_SCORE"
                status = "missing_new"
            else:
                det = (new_rec.get("detectors") or {}).get(cat) or {}
                new_score = det.get("value")
                new_source = det.get("source")
                new_det = None if new_score is None else bool(float(new_score) >= PRESENCE)
                ps = new_rec.get("persona_scores") or {}
                persona_val = ps.get(cat, "n/a" if cat not in persona_judged else None)
                if persona_val == "n/a":
                    status = "n/a"
                else:
                    status = "scored"

            entry = {
                "id": line["id"],
                "group": group,
                "category": cat,
                "status": status,
                "old_primary_score": old_score,
                "old_primary_detector": line["primary_detector"],
                "old_detected_ge_0.2": old_det,
                "new_decision_score": new_score,
                "new_source": new_source,
                "new_detected_ge_0.2": new_det,
                "persona_value": persona_val,
                # diagnostic extras kept exact
                "old_e2v_sad": line["e2v_sad"],
                "old_efficientat_crying_max": line["efficientat_crying_max"],
                "old_clap_seductive": line["clap_seductive"],
                "old_clap_love": line["clap_love"],
            }
            probe_lines.append(entry)
            bucket = per_emotion.setdefault(cat, {
                "n": 0, "old_scores": [], "new_scores": [],
                "old_detected_n": 0, "new_detected_n": 0, "status": status,
            })
            bucket["n"] += 1
            if status == "n/a":
                bucket["status"] = "n/a"
            if old_score is not None:
                bucket["old_scores"].append(old_score)
                bucket["old_detected_n"] += int(bool(old_det))
            if status != "n/a" and new_score is not None:
                bucket["new_scores"].append(float(new_score))
                bucket["new_detected_n"] += int(bool(new_det))

        per_emotion_out = {}
        for cat, b in per_emotion.items():
            if b["status"] == "n/a":
                per_emotion_out[cat] = {
                    "n": b["n"],
                    "status": "n/a",
                    "old_mean_primary_score": (sum(b["old_scores"]) / len(b["old_scores"])) if b["old_scores"] else None,
                    "old_detect_rate_0.2": (b["old_detected_n"] / b["n"]) if b["n"] else None,
                    "new_mean_decision_score": "n/a",
                    "new_detect_rate_0.2": "n/a",
                }
            else:
                per_emotion_out[cat] = {
                    "n": b["n"],
                    "status": "scored",
                    "old_mean_primary_score": (sum(b["old_scores"]) / len(b["old_scores"])) if b["old_scores"] else None,
                    "old_detect_rate_0.2": (b["old_detected_n"] / b["n"]) if b["n"] else None,
                    "new_mean_decision_score": (sum(b["new_scores"]) / len(b["new_scores"])) if b["new_scores"] else None,
                    "new_detect_rate_0.2": (b["new_detected_n"] / len(b["new_scores"])) if b["new_scores"] else None,
                }

        voices[voice] = {
            "persona": match.get("persona"),
            "sets": sets_cmp,
            "probe": {
                "n_lines": len(probe_lines),
                "lines": probe_lines,
                "per_emotion": per_emotion_out,
                "new_source_file": str(probe_paths[voice]),
                "old_source_file": old_v["probe"]["source_file"],
                "detectors_loaded": probe_new.get("detectors_loaded"),
            },
            "new_match_file": str(match_paths[voice]),
        }

    return {
        "schema": "galion-persona-rescore-comparison/1",
        "generated_at": now(),
        "timezone": "Europe/Zagreb (UTC+1)",
        "gate": GATE,
        "presence_threshold": PRESENCE,
        "old_scorer": "universal (unchanged); reused existing RESULTS.json + probe_universal_all.json",
        "new_scorer": "persona (opt-in); scorers/persona via INTERFACE.md",
        "no_deploy": True,
        "voices": voices,
    }


def compare_set(name, old_s, new_s, moments_present):
    # union of emotion keys
    old_per = (old_s or {}).get("per_emotion") or {}
    new_per = (new_s or {}).get("per_emotion") or {}
    na_cats = set((new_s or {}).get("na_categories") or [])
    judged = set((new_s or {}).get("judged") or [])
    keys = sorted(set(old_per) | set(new_per) | na_cats | judged)
    per = {}
    for cat in keys:
        if cat in na_cats or (judged and cat not in judged and cat not in new_per):
            # out of persona
            per[cat] = {
                "status": "n/a",
                "old_score": (old_per.get(cat) or {}).get("score"),
                "new_score": "n/a",
                "old_strength": (old_per.get(cat) or {}).get("strength"),
                "new_strength": "n/a",
                "old_source": (old_per.get(cat) or {}).get("source"),
                "new_source": "n/a",
            }
        else:
            o = old_per.get(cat) or {}
            n = new_per.get(cat) or {}
            per[cat] = {
                "status": "scored" if n else ("old_only" if o else "missing"),
                "old_score": o.get("score"),
                "new_score": n.get("score"),
                "old_strength": o.get("strength"),
                "new_strength": n.get("strength"),
                "old_source": o.get("source"),
                "new_source": n.get("source"),
                "old_best_line": o.get("best_line"),
                "new_best_line": n.get("best_line"),
                "old_best_match": o.get("best_match"),
                "new_best_match": n.get("best_match"),
            }
    return {
        "set": name,
        "old_emotion_match": (old_s or {}).get("emotion_match"),
        "new_emotion_match": (new_s or {}).get("emotion_match") if new_s else None,
        "old_category_blend": (old_s or {}).get("category_blend"),
        "new_category_blend": (new_s or {}).get("category_blend") if new_s else None,
        "old_emb_sim": (old_s or {}).get("emb_sim"),
        "new_emb_sim": (new_s or {}).get("emb_sim") if new_s else None,
        "old_gate_0_83": (old_s or {}).get("gate_0_83"),
        "new_gate_0_83": (new_s or {}).get("gate_0_83") if new_s else None,
        "per_emotion": per,
        "new_na_categories": sorted(na_cats) if new_s else None,
        "new_judged": sorted(judged) if new_s else None,
        "old_moments_present": moments_present,
    }


def fmt(x, digits=6):
    if x is None:
        return "—"
    if x == "n/a":
        return "n/a"
    if isinstance(x, bool):
        return "PASS" if x else "FAIL"
    if isinstance(x, (int, float)):
        return f"{x:.{digits}f}".rstrip("0").rstrip(".") if digits else str(x)
    return str(x)


# Agent-2 note: Samantha toml love target was measured with universal CLAP.
TOML_LOVE_TARGET = 0.8109382317899566  # own-samantha.toml (universal-clap)
NEW_DETECTOR_LOVE_WINDOW = 0.9228583200845187  # Agent 2 measured on original love window


def recompute_em_with_love_strength(block: dict | None, love_strength: float) -> dict | None:
    """Reweight category_blend using an alternate love moment strength. Scores unchanged."""
    if not block:
        return None
    lb = block.get("leaderboard") or []
    if not lb:
        return {
            "love_strength_used": love_strength,
            "emotion_match": block.get("emotion_match"),
            "category_blend": block.get("category_blend"),
            "emb_sim": block.get("emb_sim"),
            "pass": block.get("pass"),
            "note": "no leaderboard",
        }
    num = den = 0.0
    love_row = None
    for row in lb:
        cat = row["category"]
        strength = float(love_strength) if cat == "love" else float(row["strength"])
        w = max(strength, 1e-12)
        num += w * float(row["score"])
        den += w
        if cat == "love":
            love_row = row
    cat_blend = (num / den) if den > 0 else None
    emb = block.get("emb_sim")
    em = None
    if cat_blend is not None and emb is not None:
        em = 0.70 * cat_blend + 0.30 * float(emb)
    elif cat_blend is not None:
        em = cat_blend
    gate = block.get("gate", GATE)
    return {
        "love_strength_used": love_strength,
        "love_row_present": love_row is not None,
        "love_score_unchanged": None if love_row is None else love_row.get("score"),
        "category_blend": cat_blend,
        "emb_sim": emb,
        "emotion_match": em,
        "pass": None if em is None else bool(em + 1e-12 >= gate),
        "gate": gate,
    }


def attach_love_target_info(cmp: dict, match_paths: dict) -> None:
    """Add dual love-target gate info for Samantha (toml unchanged)."""
    sam_match = json.loads(match_paths["samantha"].read_text())
    info = {
        "note": (
            "own-samantha.toml love target remains 0.8109382317899566 "
            "(measured with universal CLAP; target_measured_by=universal-clap). "
            "Toml was NOT edited. Agent 2 final love_seductive reports "
            f"{NEW_DETECTOR_LOVE_WINDOW} on Samantha original love window. "
            "Below: gate recomputed by swapping only the love strength weight "
            "in category_blend; per-line match scores unchanged."
        ),
        "toml_love_target": TOML_LOVE_TARGET,
        "new_detector_love_window": NEW_DETECTOR_LOVE_WINDOW,
        "love_seductive_methods": {
            "love": "gender_mean_vs_neu_mean_t20_sadgate_align",
            "seductive": "gender_mean_vs_neu_mean_t20_sadgate",
            "detail": "gender-conditioned prompt bank vs neutral bank, softmax temp 20; seductive × (1 - e2v sad); love gated at mean cosine > 0.20",
        },
        "sets": {},
    }
    for key, block_key in (("ceiling", "BASE-self-ceiling"), ("merge-v2", None), ("coherent", None)):
        if key == "ceiling":
            block = sam_match.get("BASE-self-ceiling")
        else:
            block = (sam_match.get("sets") or {}).get(key)
        info["sets"][key] = {
            "as_run_detector_strengths": {
                "emotion_match": None if not block else block.get("emotion_match"),
                "category_blend": None if not block else block.get("category_blend"),
                "emb_sim": None if not block else block.get("emb_sim"),
                "pass": None if not block else block.get("pass"),
                "love_strength_in_leaderboard": None if not block else next(
                    (r.get("strength") for r in (block.get("leaderboard") or []) if r.get("category") == "love"),
                    None,
                ),
            },
            "with_toml_love_target_0_8109": recompute_em_with_love_strength(block, TOML_LOVE_TARGET),
            "with_new_detector_love_0_9228583200845187": recompute_em_with_love_strength(block, NEW_DETECTOR_LOVE_WINDOW),
        }
    cmp["samantha_love_target_info"] = info


def write_md(cmp: dict, path: Path):
    lines = []
    lines.append("# Persona-aware rescore comparison (Agent 3)")
    lines.append("")
    lines.append(f"Generated: {cmp['generated_at']} ({cmp['timezone']}).")
    lines.append(f"Gate: emotion_match ≥ {cmp['gate']}. Presence: {cmp['presence_threshold']}.")
    lines.append("No deploy / push / merge. Opt-in persona scorer only.")
    lines.append("")
    lines.append("## OLD column source (reused, not re-scored)")
    lines.append("")
    lines.append("- Probe: `scorer-probe/results/SUMMARY.json` + `probe_universal_all.json`")
    lines.append("- Ceilings + merge-v2 + coherent: `scorers/universal/results/{samantha,jarvis}-universal/RESULTS.json`")
    lines.append("  (same wav dirs as share `pkg/renders/*`; t01 sha matched for merge-v2).")
    lines.append("")
    lines.append("## NEW column source")
    lines.append("")
    lines.append("- `scorers/persona/INTERFACE.md` CLIs: `persona_match.py`, `score_persona.py`")
    lines.append("- Persona files: `pipeline/personas/own-{samantha,jarvis}.toml`")
    lines.append("- Crying decision: emotion2vec+ sad. Love/seductive: `detectors/love_seductive.py` plugin.")
    lines.append("")
    lines.append("## Gate table (emotion_match)")
    lines.append("")
    lines.append("| voice | set | old emotion_match | new emotion_match | old gate | new gate |")
    lines.append("|---|---|---|---|---|---|")
    for voice, v in cmp["voices"].items():
        for sname, s in v["sets"].items():
            lines.append(
                f"| {voice} | {sname} | {fmt(s['old_emotion_match'])} | {fmt(s['new_emotion_match'])} "
                f"| {fmt(s['old_gate_0_83'])} | {fmt(s['new_gate_0_83'])} |"
            )
    lines.append("")
    lines.append("### Samantha 0.83 gate verdict")
    lines.append("")
    sam = cmp["voices"]["samantha"]["sets"]
    for sname in ("merge-v2", "coherent"):
        s = sam[sname]
        verdict = "PASS" if s["new_gate_0_83"] else ("FAIL" if s["new_gate_0_83"] is False else "UNKNOWN")
        lines.append(
            f"- **{sname}**: new emotion_match={fmt(s['new_emotion_match'])} → **{verdict}** "
            f"(old was {fmt(s['old_emotion_match'])} / {fmt(s['old_gate_0_83'])})"
        )
    lines.append("")

    for voice, v in cmp["voices"].items():
        lines.append(f"## {voice}")
        lines.append("")
        judged = (v.get("persona") or {}).get("judged")
        lines.append(f"Persona judged: `{judged}`")
        lines.append("")
        for sname, s in v["sets"].items():
            lines.append(f"### Set `{sname}` — per emotion")
            lines.append("")
            lines.append(
                f"old em={fmt(s['old_emotion_match'])} cat={fmt(s['old_category_blend'])} emb={fmt(s['old_emb_sim'])} | "
                f"new em={fmt(s['new_emotion_match'])} cat={fmt(s['new_category_blend'])} emb={fmt(s['new_emb_sim'])}"
            )
            if s.get("new_na_categories"):
                lines.append(f"new n/a (dropped from blend): `{s['new_na_categories']}`")
            lines.append("")
            lines.append("| emotion | status | old score | new score | old strength | new strength | old source | new source |")
            lines.append("|---|---|---|---|---|---|---|---|")
            for cat, row in s["per_emotion"].items():
                lines.append(
                    f"| {cat} | {row['status']} | {fmt(row['old_score'],4)} | {fmt(row['new_score'],4)} | "
                    f"{fmt(row['old_strength'],4)} | {fmt(row['new_strength'],4)} | "
                    f"{row.get('old_source') or '—'} | {row.get('new_source') or '—'} |"
                )
            lines.append("")

        lines.append(f"### Probe (original-voice synthesized lines) — {voice}")
        lines.append("")
        lines.append("| emotion | n | status | old mean primary | old detect@0.2 | new mean decision | new detect@0.2 |")
        lines.append("|---|---|---|---|---|---|---|")
        for cat, row in sorted(v["probe"]["per_emotion"].items()):
            lines.append(
                f"| {cat} | {row['n']} | {row['status']} | {fmt(row['old_mean_primary_score'],4)} | "
                f"{fmt(row['old_detect_rate_0.2'],4)} | {fmt(row['new_mean_decision_score'],4)} | "
                f"{fmt(row['new_detect_rate_0.2'],4)} |"
            )
        lines.append("")
        lines.append("Per-line detail (crying / seductive / love / neutral):")
        lines.append("")
        lines.append("| id | group | status | old score | old det | new score | new det | persona | new source |")
        lines.append("|---|---|---|---|---|---|---|---|---|")
        for L in v["probe"]["lines"]:
            if L["group"] not in ("crying", "seductive", "love", "neutral", "laughter"):
                continue
            lines.append(
                f"| {L['id']} | {L['group']} | {L['status']} | {fmt(L['old_primary_score'],4)} | "
                f"{L['old_detected_ge_0.2']} | {fmt(L['new_decision_score'],4)} | {L['new_detected_ge_0.2']} | "
                f"{fmt(L['persona_value'],4) if not isinstance(L['persona_value'], str) or L['persona_value']!='n/a' else 'n/a'} | "
                f"{(L.get('new_source') or '—')[:60]} |"
            )
        lines.append("")


    # Samantha love-target sensitivity
    info = cmp.get("samantha_love_target_info")
    if info:
        lines.append("## Samantha love-target sensitivity (toml unchanged)")
        lines.append("")
        lines.append(info["note"])
        lines.append("")
        lines.append(f"- Toml love target: `{info['toml_love_target']}`")
        lines.append(f"- New-detector love window: `{info['new_detector_love_window']}`")
        lines.append(f"- Plugin methods: `{info['love_seductive_methods']}`")
        lines.append("")
        lines.append("| set | as-run em (detector strengths) | as-run gate | em @ toml love 0.8109 | gate @ 0.8109 | em @ new-det 0.9228583200845187 | gate @ 0.9228 |")
        lines.append("|---|---|---|---|---|---|---|")
        for sname, s in info["sets"].items():
            a = s["as_run_detector_strengths"]
            t = s["with_toml_love_target_0_8109"] or {}
            n = s["with_new_detector_love_0_9228583200845187"] or {}
            lines.append(
                f"| {sname} | {fmt(a.get('emotion_match'))} | {fmt(a.get('pass'))} | "
                f"{fmt(t.get('emotion_match'))} | {fmt(t.get('pass'))} | "
                f"{fmt(n.get('emotion_match'))} | {fmt(n.get('pass'))} |"
            )
        lines.append("")


    lines.append("## Notes")
    lines.append("")
    lines.append("- Out-of-persona emotions are **n/a**, never scored as 0.")
    lines.append("- Samantha probe is missing aggressive/love/laughter wavs (20 lines); Jarvis has 26.")
    lines.append("- Old probe `primary_detect_rate` in SUMMARY.json for Samantha seductive said 0.0 but per-line CLAP caught sed01+sed04 (≥0.2); this comparison uses per-line primary scores.")
    lines.append("- Clone emotion_match only includes emotions with a present moment on the original (≥ presence) that are also in the persona set.")
    path.write_text("\n".join(lines) + "\n")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "new").mkdir(exist_ok=True)
    (OUT / "logs").mkdir(exist_ok=True)
    t0 = time.time()
    old = ensure_old()

    # Wait markers already satisfied by caller; re-check
    iface = ROOT / "scorers/persona/INTERFACE.md"
    plugin = list((ROOT / "scorers/persona/detectors").glob("love_seductive*.py"))
    if not iface.exists() or not plugin:
        missing = []
        if not iface.exists():
            missing.append(str(iface))
        if not plugin:
            missing.append("scorers/persona/detectors/love_seductive*.py")
        report = {
            "schema": "galion-persona-rescore-comparison/1",
            "generated_at": now(),
            "status": "incomplete",
            "missing": missing,
            "old_gates": json.loads((OUT / "old" / "OLD-gates.json").read_text()),
        }
        (OUT / "COMPARISON.json").write_text(json.dumps(report, indent=2))
        (OUT / "COMPARISON.md").write_text(
            "# Incomplete\n\nMissing: " + ", ".join(missing) + "\n\nOLD gates preserved in old/OLD-gates.json.\n"
        )
        print("INCOMPLETE", missing)
        return 2

    match_paths = {}
    probe_paths = {}
    for voice in ("samantha", "jarvis"):
        match_paths[voice] = run_persona_match(voice)
        probe_paths[voice] = run_persona_probe(voice)

    cmp = build_comparison(old, match_paths, probe_paths)
    attach_love_target_info(cmp, match_paths)
    cmp["timing_wall_s"] = time.time() - t0
    cmp["love_seductive_plugin_mtime"] = str(Path("/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/persona/detectors/love_seductive.py").stat().st_mtime)
    (OUT / "COMPARISON.json").write_text(json.dumps(cmp, indent=2))
    write_md(cmp, OUT / "COMPARISON.md")
    print(OUT / "COMPARISON.json")
    print(OUT / "COMPARISON.md")
    # print gate summary
    for voice, v in cmp["voices"].items():
        for sname, s in v["sets"].items():
            print(f"  {voice:9s} {sname:10s} old={s['old_emotion_match']} new={s['new_emotion_match']} "
                  f"old_gate={s['old_gate_0_83']} new_gate={s['new_gate_0_83']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
