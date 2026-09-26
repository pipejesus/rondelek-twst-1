#version 330

// Vowel Runner's water: a playful, cartoon sea along the front of the world.
//
//   * toon colour steps from shallow turquoise at the bank to deep blue;
//   * wobbly light-lines (a moving Voronoi web — the "sunlight on water" net);
//   * pale crest bands where the swell peaks;
//   * a frothy foam edge and a row of bubbles lapping at the bank;
//   * twinkles — little "+" stars, the same shape the score sun throws —
//     that multiply while the child makes sound (uVoice);
//   * a few goldfish gliding under the surface.
//
// Everything tunable is a uniform, set from WaterParams (game/src/water.rs).
// Anything sampled along x repeats on PERIOD, the distance uScroll wraps on
// (see water.vs), so the scrolling water never jumps.

in vec3  fragWorld;
in vec3  fragNormal;
in vec2  fragPattern;
in float fragCrest;

out vec4 finalColor;

// Set by the game each frame.
uniform vec3  uEye;
uniform float uTime;
uniform float uVoice;

// Rows of WaterParams.
uniform vec3  uShallow;
uniform vec3  uDeep;
uniform float uDeepDist;
uniform float uBands;
uniform vec3  uLine;
uniform float uCellScale;
uniform float uCellSpeed;
uniform float uLineWidth;
uniform float uLineGain;
uniform float uCrestGain;
uniform vec3  uFoam;
uniform float uFoamWidth;
uniform float uFoamWobble;
uniform float uBubbleGain;
uniform vec3  uSparkle;
uniform float uSparkleScale;
uniform float uSparkleDensity;
uniform float uSparkleSize;
uniform float uSparkleGain;
uniform float uVoiceSparkle;
uniform float uFishOn;
uniform vec3  uFish;
uniform float uFishGain;
uniform float uFishSpeed;
uniform float uFishSize;
uniform vec3  uSkyTint;
uniform float uFresPow;
uniform float uReflGain;
uniform float uAlpha;

const float TAU    = 6.2831853;
const float PERIOD = 64.0;

float kx(float turns) { return TAU * turns / PERIOD; }

// Cells per world unit, snapped so a whole number of cells fits PERIOD.
float fitScale(float perUnit) { return max(floor(PERIOD * perUnit + 0.5), 1.0) / PERIOD; }

vec2 hash2(vec2 c) {
    c = vec2(dot(c, vec2(127.1, 311.7)), dot(c, vec2(269.5, 183.3)));
    return fract(sin(c) * 43758.5453);
}

// Cell id wrapped on the period along x, so cell n and cell n + cols match.
vec2 wrapCell(vec2 c, float cols) { return vec2(mod(c.x, cols), c.y); }

// Distance to the nearest border of a gently moving Voronoi web (F2 - F1).
float web(vec2 p, float perUnit) {
    float sc = fitScale(perUnit);
    float cols = PERIOD * sc;
    vec2 g = p * sc;
    vec2 cell = floor(g);
    vec2 f = fract(g);
    float f1 = 8.0, f2 = 8.0;
    for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
        vec2 o = vec2(float(i), float(j));
        vec2 h = hash2(wrapCell(cell + o, cols));
        vec2 pt = o + 0.5 + 0.38 * sin(uTime * uCellSpeed + TAU * h);
        float d = length(pt - f);
        if (d < f1) { f2 = f1; f1 = d; } else if (d < f2) { f2 = d; }
    }
    return f2 - f1;
}

// A "+" twinkle centred at 0, arms `size` long: 1 on the arms, 0 off them.
float plusStar(vec2 d, float size) {
    vec2 a = abs(d) / size;
    float armX = (1.0 - smoothstep(0.7, 1.0, a.x)) * (1.0 - smoothstep(0.12, 0.2, a.y));
    float armY = (1.0 - smoothstep(0.7, 1.0, a.y)) * (1.0 - smoothstep(0.12, 0.2, a.x));
    return max(armX, armY);
}

float twinkles(vec2 p) {
    float sc = fitScale(uSparkleScale);
    float cols = PERIOD * sc;
    vec2 g = p * sc;
    vec2 cell = floor(g);
    vec2 h = hash2(wrapCell(cell, cols) + 17.0);
    // More of the cells light up while the child is making sound.
    float density = clamp(uSparkleDensity + uVoiceSparkle * uVoice, 0.0, 1.0);
    if (h.x > density) return 0.0;
    vec2 centre = 0.2 + 0.6 * hash2(wrapCell(cell, cols) + 3.0);
    // Short flashes: mostly dark, a quick bright peak.
    float flash = pow(max(sin(uTime * (1.5 + 2.0 * h.y) + TAU * h.x * 7.0), 0.0), 10.0);
    return plusStar((fract(g) - centre) / sc, uSparkleSize) * flash;
}

// One goldfish: a soft body, a flicking tail and an eye, gliding along x.
// Returns (coverage, eye).
vec2 fish(vec2 p, float seed, float lane) {
    float dir = seed > 0.5 ? 1.0 : -1.0;
    float x0 = mod(seed * PERIOD * 7.0 + uTime * uFishSpeed * (0.7 + seed) * dir, PERIOD);
    vec2 r = vec2(p.x - x0, p.y - lane);
    r.x = mod(r.x + 0.5 * PERIOD, PERIOD) - 0.5 * PERIOD;   // nearest copy on the wrap
    r.x *= dir;                                             // face the way it swims
    r /= uFishSize;
    r.y += 0.035 * sin(r.x * 7.0 - uTime * 9.0);            // a swimmy wiggle
    float body = 1.0 - smoothstep(0.85, 1.0, length(r / vec2(0.42, 0.17)));
    // Tail: a fan that opens behind the body, flicking side to side.
    float tx = -r.x - 0.34;                                 // distance behind the body
    float flick = 0.05 * sin(uTime * 9.0);
    float tail = step(0.0, tx) * step(tx, 0.24)
               * (1.0 - smoothstep(tx * 0.9, tx * 0.9 + 0.02, abs(r.y - flick * tx * 4.0)));
    float eye = 1.0 - smoothstep(0.03, 0.045, length(r - vec2(0.22, 0.04)));
    return vec2(max(body, tail), eye);
}

void main() {
    vec2 p = fragPattern;
    vec3 n = normalize(fragNormal);
    vec3 v = normalize(uEye - fragWorld);

    // Toon depth colour: soft steps from the bank out.
    float t = clamp(p.y / uDeepDist, 0.0, 1.0);
    if (uBands > 0.5) {
        float b = t * uBands;
        t = (floor(b) + smoothstep(0.4, 0.6, fract(b))) / uBands;
    }
    vec3 col = mix(uShallow, uDeep, t);

    // Goldfish, under the surface — so drawn first and half melted into it.
    if (uFishOn > 0.5) {
        for (int i = 0; i < 6; i++) {
            float fi = float(i);
            // Spread out along the shore (golden-ratio seeds) and at different
            // distances from the bank.
            vec2 f = fish(p, fract(0.37 + fi * 0.618), 1.6 + fract(fi * 0.43) * 3.6);
            col = mix(col, uFish, f.x * uFishGain);
            col = mix(col, vec3(0.1, 0.1, 0.15), f.y * uFishGain);
        }
    }

    // The sky, reflected where the water is seen at a slant.
    float fres = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), uFresPow);
    col = mix(col, uSkyTint, clamp(fres * uReflGain, 0.0, 1.0));

    // Crest bands where the swell peaks.
    col = mix(col, uLine, smoothstep(0.55, 0.75, fragCrest) * uCrestGain);

    // The light-line web, anti-aliased by its own screen-space rate.
    float d = web(p, uCellScale);
    float aa = fwidth(d);
    col = mix(col, uLine, (1.0 - smoothstep(uLineWidth, uLineWidth + aa, d)) * uLineGain);

    // Foam at the bank: a wobbly frothy edge, and a row of bubbles beyond it.
    float edge = uFoamWidth * (1.0 + uFoamWobble * (0.6 * sin(p.x * kx(22.0) + uTime * 2.1)
                                                  + 0.4 * sin(p.x * kx(57.0) - uTime * 3.3)));
    float foam = 1.0 - smoothstep(edge, edge + 0.03, p.y);
    {
        float sc = fitScale(3.0);
        vec2 g = vec2(p.x * sc, (p.y - edge - 0.12) * 3.0);
        vec2 cell = floor(g);
        if (cell.y == 0.0) {
            vec2 h = hash2(wrapCell(cell, PERIOD * sc) + 41.0);
            float r = 0.18 + 0.14 * h.y + 0.05 * sin(uTime * 3.0 + TAU * h.x);
            float dist = length(fract(g) - vec2(0.5, 0.5));
            float ring = 1.0 - smoothstep(0.03, 0.08, abs(dist - r));
            foam = max(foam, ring * uBubbleGain * step(0.35, h.x));
        }
    }
    col = mix(col, uFoam, foam);

    // Twinkles on top of everything.
    col = mix(col, uSparkle, clamp(twinkles(p) * uSparkleGain, 0.0, 1.0));

    finalColor = vec4(col, uAlpha);
}
