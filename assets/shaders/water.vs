#version 330

// Vowel Runner's water: the surface. A flat grid (GenMeshPlane) lifted by a
// gentle swell that rolls in toward the bank, so the water laps the dirt as it
// goes — and swells a little more while the child is making sound (uVoice).
//
// Every x-frequency here is a whole number of turns per PERIOD, the distance
// uScroll wraps on: the water scrolls with the ground, and a frequency that
// didn't fit the wrap would make the whole surface jump once per wrap.

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 matModel;

// Set by the game each frame.
uniform float uTime;
uniform float uScroll;   // world x travelled, wrapped on PERIOD
uniform float uVoice;    // the child's voice level, smoothed, 0..1
uniform float uShoreZ;   // world z of the bank's face

// Rows of WaterParams.
uniform float uSwellAmp;
uniform float uSwellSpeed;
uniform float uSwellLength;
uniform float uVoiceSwell;

out vec3  fragWorld;
out vec3  fragNormal;
out vec2  fragPattern;  // (x along the shore, scrolled; distance out from the bank)
out float fragCrest;    // the swell, -1..1, for the toon crest bands

const float TAU    = 6.2831853;
const float PERIOD = 64.0;

float kx(float turns) { return TAU * turns / PERIOD; }

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
    vec3 w = (matModel * vec4(vertexPosition, 1.0)).xyz;
    vec2 p = vec2(w.x + uScroll, w.z - uShoreZ);

    float amp = uSwellAmp * (1.0 + uVoiceSwell * uVoice);
    float s = swell(p);
    float h = s * amp;

    // Normal off the height field, by central differences.
    float e = 0.05;
    float dx = (swell(p + vec2(e, 0.0)) - swell(p - vec2(e, 0.0))) * amp / (2.0 * e);
    float dz = (swell(p + vec2(0.0, e)) - swell(p - vec2(0.0, e))) * amp / (2.0 * e);

    vec3 pos = vertexPosition + vec3(0.0, h, 0.0);
    fragWorld   = (matModel * vec4(pos, 1.0)).xyz;
    fragNormal  = normalize(vec3(-dx, 1.0, -dz));
    fragPattern = p;
    fragCrest   = s;
    gl_Position = mvp * vec4(pos, 1.0);
}
