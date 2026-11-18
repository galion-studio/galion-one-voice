# Feeling core

Perpetual Open MIT License. (c) 2026 Galion Labs.

The speaker's feeling is a point, updated once per 80 ms tick. The point is
not a category. Valence, arousal, dominance and approach are the named axes.
A frame may name more axes. Those are learned dimensions: the core carries
them with the same inertia and does not interpret them.

The 24 names in `configs/emotions-v2.toml` are optional pins. A pin is placed
only where a source gives a direction, and only at the pole of that axis
(+1 or −1). The pole is the end of this state space. It is not a loading
copied from a figure. An omitted axis is unplaced and does not move. Naming
a pin puts that name on the downstream tick. Sitting near a pin without
naming it does not.

80 ms is `LOCKS.toml` `clock.frame_ms`. It is not a feeling time constant.

## Update

One intent frame, then:

1. The frame blends into the held goal. `blend` is how far this frame
   replaces the goal on the axes it sets. An explicit axis replaces a pin
   on that axis. Release sends the goal toward 0 and ignores pins.
2. Energy and fatigue step toward the frame on their own half-life. They
   are not axes. Energy's rest, 0.5, is the neutral point of the lungs
   energy knob, not a measured effort. Fatigue's rest is 0, which leaves
   that lungs term unchanged.
3. The drive is `(1 − linger) · goal + linger · mood`. A coupling is added
   only when its coefficient is a number. The shipped coefficients are
   `unknown` and are not added.
4. The moment steps toward the drive. The mood steps toward the moment.

The unmoved fraction is `0.5^(dt / half_life)`, the same form as
`PersonaMood::tick` in `galion-emotion`. That function is not called. It
is a closed three-axis appraisal of a user. Half-lives here are config.
Kuppens, Oravecz and Tuerlinckx (2010) establish inertia and a home base.
Their estimates are not copied.

After release the goal is the baseline. The mood is still near the recent
moment, so the drive stays partway there, and both decay. With `linger` at
0 the moment only has its own inertia.

## What is placed

| Pin | Valence | Arousal | Approach | Left unplaced |
|---|---|---|---|---|
| anger | − | + | + | dominance (hot / cold) |
| fear | − | | − | arousal (bimodal intensity), dominance (panic / anxiety) |
| sadness | − | − | | dominance, approach |
| disgust | − | | | arousal, approach |
| adoration | | − | | valence. The low-arousal row is Juslin's love-tenderness |
| contentment | + | − | | dominance, approach |

Anger and fear are both unpleasant. Approach separates them (Carver and
Harmon-Jones, 2009). The other 18 names are legal pins with no coordinate.

Old words follow `configs/emotions-v2.toml`. One target is renamed
(`seductive` → desire, `laughter` → burst amusement, `tender` → adoration).
A split (`happy`, `sigh`) is not assigned a winner. `calm` is the baseline.
`breath` is refused.

## Downstream

`controls` is the tick PR #192 reads: `t_ms`, `track`, `arousal`, `valence`,
`dominance`, `intensity`, `blend[{id,w}]`. Intensity is the largest absolute
core axis. It is a readout of distance from the baseline, not a published
strength. Approach is not in that packet.

`lungs` is the moment PR #195 reads: `category`, `arousal`, `energy`,
`fatigue`, and a trajectory of those at `at_s`. The category is the named
pin. A tie, a split, or no pin is `neutral`. Text is not planned here.

Listener distance and the environment label are stored on the feeling.
They do not move an axis while the coefficient is `unknown`. Spec §1.3
has no distance-to-feeling function. Garnier, Henrich and Dubois (2010)
is a loudness effect for the body.

## Stubbed

`tools/symphony/feeling/mne-contract` is schema `galion-mne-contract/0`.
Agent S1 owns the real contract. A frame with another schema is refused.
This layer does not render cloning controls and does not plan breaths.

## Sources

Russell, J. A. (1980). A circumplex model of affect. *Journal of Personality and Social Psychology, 39*(6), 1161–1178.

Russell, J. A., & Mehrabian, A. (1977). Evidence for a three-factor theory of emotions. *Journal of Research in Personality, 11*(3), 273–294. Dominance is kept as an axis. No per-category loading is copied.

Carver, C. S., & Harmon-Jones, E. (2009). Anger is an approach-related affect: Evidence and implications. *Psychological Bulletin, 135*(2), 183–204.

Juslin, P. N., & Laukka, P. (2003). Communication of emotions in vocal expression and music performance. *Psychological Bulletin, 129*(5), 770–814. Table 7.

Cowen, A. S., Elfenbein, H. A., Laukka, P., & Keltner, D. (2019). Mapping 24 emotions conveyed by brief human vocalization. *American Psychologist, 74*(6), 698–712. Figure 3B.

Cowen, A. S., Laukka, P., Elfenbein, H. A., Liu, R., & Keltner, D. (2019). The primacy of categories in the recognition of 12 emotions in speech prosody across two cultures. *Nature Human Behaviour, 3*(4), 369–382.

Kuppens, P., Oravecz, Z., & Tuerlinckx, F. (2010). Feelings change: Accounting for individual differences in the temporal dynamics of affect. *Journal of Personality and Social Psychology, 99*(6), 1042–1060.

Thayer, R. E. (1989). *The biopsychology of mood and arousal.* Oxford University Press.
