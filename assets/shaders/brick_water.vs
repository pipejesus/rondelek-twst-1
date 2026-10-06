#version 330

// Vowel Runner's brick water: the vertex stage in front of Lam::pula.
//
// The water is a field of little glass bricks: one column per cell, each a
// short stack of bricks whose top is the water's surface. This stage lifts
// every column on a gentle swell rolling toward the bank and picks its
// colour; the fragment stage is flat-draw's
// lampula.fs, unchanged, which lights it exactly as it lights the clouds.
//
// Two things make that work with a shader written for still drawings:
//
//   * fragWorld is the brick's *resting* place. Lam::pula finds brick edges
//     and corners by running fragWorld back through uBrick onto the lattice,
//     so a column that has bobbed up keeps its own edges and corner sparks
//     instead of sliding against the lattice. (The light barely notices:
//     the swell moves a brick by a few hundredths of a unit.)
//   * fragTexCoord points into a 2×2 palette (game/src/brick_water.rs):
//     shallow → deep across, water → foam down, blended by the texture's own
//     bilinear filter. Every vertex of a column gets the same coordinate, so
//     each brick is one flat colour, like a pixel of a drawing.
//
// Every x-frequency is a whole number of turns per PERIOD, the distance the
// scroll wraps on: the water scrolls with the ground, and a frequency that
// didn't fit the wrap would make the whole surface jump once per wrap.

in vec3 vertexPosition;
in vec2 vertexTexCoord;   // the column's centre (x, z) in mesh space
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 matModel;

// Set by the game each frame.
uniform float uTime;     // shared with lampula.fs, which orbits its lamps by it
uniform float uScroll;   // whole cells travelled, in world units, wrapped on PERIOD
uniform float uVoice;    // the child's voice level, smoothed, 0..1
uniform float uCell;     // one brick's size, world units

// Rows of BrickWaterParams.
uniform float uSwellAmp;
uniform float uSwellSpeed;
uniform float uSwellLength;
uniform float uVoiceSwell;
uniform float uDeepDist;
uniform float uBands;
uniform float uJitter;
uniform float uCrestFoam;
uniform float uFoamWidth;
uniform float uFoamWobble;
uniform float uFoamSpeed;

out vec2 fragTexCoord;
out vec3 fragNormal;
out vec3 fragWorld;

const float TAU    = 6.2831853;
const float PERIOD = 64.0;

float kx(float turns) { return TAU * turns / PERIOD; }

float hash(vec2 c) { return fract(sin(dot(c, vec2(127.1, 311.7))) * 43758.5453); }

// The swell's shape, -1..1: a long roll toward the bank, bent along the shore,
// with a shorter cross-wave on top. sin(k·z + ω·t) moves toward smaller z —
// toward the bank.
float swell(vec2 p) {
    float t = uTime * uSwellSpeed;
    float a = sin(p.y * TAU / uSwellLength + t + 0.8 * sin(p.x * kx(9.0) + 0.3 * t));
    float b = sin(p.y * TAU / (uSwellLength * 0.57) - p.x * kx(13.0) + t * 1.37);
    return 0.65 * a + 0.35 * b;
}

void main() {
    // The column this vertex belongs to, in pattern space: x along the shore
    // (scrolled), y the distance out from the bank.
    vec2 p = vec2(vertexTexCoord.x + uScroll, vertexTexCoord.y);
    // Centres sit mid-cell, so floor() names the cell without a doubt.
    vec2 cell = floor(p / uCell);
    float h = hash(vec2(mod(cell.x, PERIOD / uCell), cell.y));

    float s = swell(p);
    float lift = s * uSwellAmp * (1.0 + uVoiceSwell * uVoice);

    // Colour: toon steps from the bank out, a little jitter per brick so the
    // band edges come out as a pixel-art dither…
    float depth = clamp(p.y / uDeepDist + uJitter * (h - 0.5), 0.0, 1.0);
    if (uBands > 0.5) depth = floor(depth * uBands + 0.5) / uBands;
    // …foam on the bricks riding a crest…
    float foam = smoothstep(0.55, 0.95, s) * uCrestFoam;
    // …and a wobbly foam edge lapping at the bank.
    float ft = uTime * uFoamSpeed;
    float edge = uFoamWidth * (1.0 + uFoamWobble * (0.6 * sin(p.x * kx(22.0) + ft * 2.1)
                                                  + 0.4 * sin(p.x * kx(57.0) - ft * 3.3)));
    foam = max(foam, 1.0 - smoothstep(edge - 0.5 * uCell, edge + 0.5 * uCell, p.y));

    fragTexCoord = vec2(0.25 + 0.5 * depth, 0.25 + 0.5 * clamp(foam, 0.0, 1.0));
    fragNormal   = normalize(mat3(matModel) * vertexNormal);
    fragWorld    = (matModel * vec4(vertexPosition, 1.0)).xyz;
    gl_Position  = mvp * vec4(vertexPosition + vec3(0.0, lift, 0.0), 1.0);
}
