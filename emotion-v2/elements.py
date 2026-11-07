# Galion Voice — Perpetual Open MIT License
# (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
"""Demo 13 v2 and the 62-row universal lookup.

Label data only. Importing this module must not change pitch, pace,
energy, or any other acoustic parameter. Spectral hex and display hex
are separate fields. The visualiser renders DEMO, not UNIVERSAL.
"""

from __future__ import annotations

TABLE_ID = "galion-nebula-elements/demo-13-v2"

# emotion, doc_name, gas, element_id, wavelength_nm, spectral_hex, display_hex,
# motion, pulse, intrinsic, lab_line, samantha_only, evidence, physics
DEMO = (
    ('neutral', 'Neutral', 'He II', 'HeII-541.2', 541.2, '#00FF00', '#9DA3A8', 0.30, 0.00, 0.22, False, False, 'none', 'He II 541.2 is a weak high-excitation recombination line; the green carrier is fully desaturated.'),
    ('calm', 'Calm / calmness', 'H I H-beta', 'H-beta', 486.1, '#00C0FF', '#47B5D1', 0.22, 0.00, 0.90, False, False, 'moderate', 'H-beta is a strong permitted Balmer line and the usual optical reference.'),
    ('happy', 'Happy / joy', '[N II]', 'NII-575.5', 575.5, '#FFD300', '#FFCF40', 0.72, 0.00, 0.28, False, False, 'strong', '[N II] 575.5 is a weak temperature-sensitive auroral line, not a dominant nebular feature.'),
    ('amused', 'Amused / amusement', 'He I', 'HeI-587.6', 587.6, '#FF8700', '#FF9C64', 0.80, 0.00, 0.70, False, False, 'strong', 'He I 587.6 is usually the strongest optical helium recombination line, still below the Balmer lines.'),
    ('excited', 'Excited / excitement', 'Ne I (lab)', 'NeI-594.5', 594.5, '#FF6300', '#FF6300', 1.40, 0.00, 0.35, True, False, 'moderate', 'Ne I 594.5 is a laboratory discharge line, not a typical ionised-nebula emission line.'),
    ('tender', 'Tender / tenderness', 'He I', 'HeI-667.8', 667.8, '#FF0000', '#FFB5C5', 0.30, 0.00, 0.26, False, False, 'weak', 'He I 667.8 is a weak permitted recombination line; pink is a white mix, not a wavelength.'),
    ('sad', 'Sad / sadness', 'He II', 'HeII-468.6', 468.6, '#0017FF', '#354766', 0.24, 0.00, 0.50, False, False, 'strong', 'He II 468.6 can be strong in high-excitation planetary nebulae and is weak in ordinary H II regions.'),
    ('disappointed', 'Disappointed / disappointment', '[Cl III]', 'ClIII-551.7', 551.7, '#00FF00', '#676A70', 0.34, 0.00, 0.24, False, False, 'strong', '[Cl III] 551.7 is a weak forbidden density diagnostic.'),
    ('angry', 'Angry / anger', '[S III]', 'SIII-631.2', 631.2, '#FF0000', '#D90000', 1.45, 0.00, 0.30, False, False, 'strong', '[S III] 631.2 is a weak auroral line, far below the strong near-infrared [S III] lines.'),
    ('fearful', 'Fearful / fear', 'H I H-delta', 'H-delta', 410.2, '#7000FF', '#171027', 1.25, 0.00, 0.42, False, False, 'strong', 'H-delta is a moderate Balmer line, weaker than H-gamma and H-beta.'),
    ('surprised', 'Surprised / surprise', '[O III]', 'OIII-495.9', 495.9, '#00FFCB', '#00FFCB', 1.15, 1.00, 0.85, False, False, 'none', '[O III] 495.9 is the strong but weaker companion of the 500.7 line.'),
    ('disgusted', 'Disgusted / disgust', '[Cl III]', 'ClIII-553.8', 553.8, '#39FF00', '#8CAF24', 0.78, 0.00, 0.28, False, False, 'moderate', '[Cl III] 553.8 is the density-sensitive partner of 551.7 and is a weak forbidden line.'),
    ('seductive', 'Seductive (Samantha only)', 'H I H-alpha', 'H-alpha', 656.3, '#FF0000', '#C42F63', 0.28, 0.00, 1.00, False, True, 'weak', 'H-alpha is often the strongest Balmer line; rose is a dimmed mix, not a different wavelength.'),
)

# emotion, doc_name, gas, wavelength_nm, spectral_hex
UNIVERSAL = (
    ('neutral', 'Neutral', 'He II', 541.2, '#00FF00'),
    ('calm', 'Calm / calmness', 'H I H-beta', 486.1, '#00C0FF'),
    ('happy', 'Happy / joy', '[N II]', 575.5, '#FFD300'),
    ('amused', 'Amused / amusement', 'He I', 587.6, '#FF8700'),
    ('excited', 'Excited / excitement', 'Ne I (lab)', 594.5, '#FF6300'),
    ('tender', 'Tender / tenderness', 'He I', 667.8, '#FF0000'),
    ('sad', 'Sad / sadness', 'He II', 468.6, '#0017FF'),
    ('disappointed', 'Disappointed / disappointment', '[Cl III]', 551.7, '#00FF00'),
    ('angry', 'Angry / anger', '[S III]', 631.2, '#FF0000'),
    ('fearful', 'Fearful / fear', 'H I H-delta', 410.2, '#7000FF'),
    ('surprised', 'Surprised / surprise', '[O III]', 495.9, '#00FFCB'),
    ('disgusted', 'Disgusted / disgust', '[Cl III]', 553.8, '#39FF00'),
    ('seductive', 'Seductive (Samantha only)', 'H I H-alpha', 656.3, '#FF0000'),
    ('shame', 'Shame', '[N II]', 658.4, '#FF0000'),
    ('guilt', 'Guilt', 'He II', 541.2, '#00FF00'),
    ('pride', 'Pride', '[N II]', 575.5, '#FFD300'),
    ('relief', 'Relief', 'He II', 541.2, '#00FF00'),
    ('awe', 'Awe', 'H I H-gamma', 434.0, '#6700FF'),
    ('nostalgia', 'Nostalgia', 'Ne I (lab)', 585.2, '#FF9500'),
    ('longing', 'Longing', 'He I', 447.1, '#5600FF'),
    ('contempt', 'Contempt', 'He II', 541.2, '#00FF00'),
    ('embarrassment', 'Embarrassment', '[O I]', 630.0, '#FF0000'),
    ('hope', 'Hope', '[O III]', 500.7, '#00FF9C'),
    ('despair', 'Despair', 'H I H-delta', 410.2, '#7000FF'),
    ('serenity', 'Serenity', 'H I H-beta', 486.1, '#00C0FF'),
    ('love', 'Love', 'H I H-alpha', 656.3, '#FF0000'),
    ('jealousy', 'Jealousy', '[N II]', 658.4, '#FF0000'),
    ('envy', 'Envy', '[O III]', 500.7, '#00FF9C'),
    ('gratitude', 'Gratitude', 'He I', 587.6, '#FF8700'),
    ('compassion', 'Compassion', 'He I', 667.8, '#FF0000'),
    ('loneliness', 'Loneliness', 'He I', 447.1, '#5600FF'),
    ('boredom', 'Boredom', '[Cl III]', 551.7, '#00FF00'),
    ('anxiety', 'Anxiety', 'H I H-delta', 410.2, '#7000FF'),
    ('interest', 'Interest / curiosity', '[O III]', 495.9, '#00FFCB'),
    ('confusion', 'Confusion', 'H I H-gamma', 434.0, '#6700FF'),
    ('triumph', 'Triumph', '[N II]', 575.5, '#FFD300'),
    ('admiration', 'Admiration', 'He I', 587.6, '#FF8700'),
    ('adoration', 'Adoration', 'He I', 667.8, '#FF0000'),
    ('aesthetic_appreciation', 'Aesthetic appreciation', '[Ar IV]', 474.0, '#0054FF'),
    ('awkwardness', 'Awkwardness', '[O I]', 630.0, '#FF0000'),
    ('craving', 'Craving', '[O I]', 630.0, '#FF0000'),
    ('empathic_pain', 'Empathic pain', 'He I', 447.1, '#5600FF'),
    ('entrancement', 'Entrancement', '[Ar IV]', 474.0, '#0054FF'),
    ('horror', 'Horror', 'H I H-delta', 410.2, '#7000FF'),
    ('romance', 'Romance', 'He I', 667.8, '#FF0000'),
    ('satisfaction', 'Satisfaction', 'H I H-beta', 486.1, '#00C0FF'),
    ('sexual_desire', 'Sexual desire', 'H I H-alpha', 656.3, '#FF0000'),
    ('sympathy', 'Sympathy', 'He I', 667.8, '#FF0000'),
    ('contentment', 'Contentment', 'H I H-beta', 486.1, '#00C0FF'),
    ('pleasure', 'Pleasure', 'He I', 667.8, '#FF0000'),
    ('regret', 'Regret', '[Cl III]', 551.7, '#00FF00'),
    ('hate', 'Hate', '[S II]', 671.6, '#FF0000'),
    ('annoyance', 'Annoyance', '[O I]', 630.0, '#FF0000'),
    ('approval', 'Approval', '[O III]', 500.7, '#00FF9C'),
    ('caring', 'Caring', 'He I', 667.8, '#FF0000'),
    ('desire', 'Desire (general)', 'Ne I (lab)', 585.2, '#FF9500'),
    ('disapproval', 'Disapproval', '[O I]', 630.0, '#FF0000'),
    ('grief', 'Grief', 'He II', 468.6, '#0017FF'),
    ('nervousness', 'Nervousness', 'H I H-delta', 410.2, '#7000FF'),
    ('optimism', 'Optimism', '[N II]', 575.5, '#FFD300'),
    ('realization', 'Realization', '[O III]', 495.9, '#00FFCB'),
    ('remorse', 'Remorse', '[Cl III]', 551.7, '#00FF00'),
)


def _norm(name: str) -> str:
    return name.strip().casefold()


def demo_by_name(name: str):
    key = _norm(name)
    for row in DEMO:
        if key in {_norm(row[0]), _norm(row[1]), _norm(row[3])}:
            return row
    return None


def universal_by_name(name: str):
    key = _norm(name)
    for row in UNIVERSAL:
        if key in {_norm(row[0]), _norm(row[1])}:
            return row
    return None


def element_id(name: str) -> str:
    row = demo_by_name(name)
    return "" if row is None else row[3]


def label_weight(emotion: str, weight: float, element: str | None = None) -> dict[str, object]:
    """Attach the fixed element id. A mismatched id is rejected."""
    row = demo_by_name(emotion)
    if row is None:
        raise KeyError(emotion)
    if element and element.strip().casefold() != row[3].casefold():
        raise ValueError(f"element {element!r} does not match {row[3]}")
    return {"emotion": row[0], "weight": float(weight), "element": row[3]}

