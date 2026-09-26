#version 330

// Copied 1:1 from flat-draw (internal/gpu/lampula.go, lampFS) at flat-draw commit 92d9ac9
// (2026-09-17). Do not edit the shader body here to tune the look: every value
// it reads is a uniform, set from LampulaParams in game/src/lampula.rs.

in vec2 fragTexCoord;
in vec3 fragNormal;
in vec3 fragWorld;

out vec4 finalColor;

uniform sampler2D texture0;
uniform vec4 colDiffuse;

uniform vec3 uEye;

// Seconds. The orbit carries no state between frames — it is a circle — so this is
// the whole of what makes the lamps move.
uniform float uTime;

// The model's bounding box, as a centre and a half-extent. The lamps orbit it, so
// the lighting is the same on a 16-pixel sprite and a 256-pixel one.
uniform vec3 uBlobCentre;
uniform vec3 uBlobRadius;

// The mesher's conversion, run backwards and packed as one affine map: a world
// position through this comes back as (x, y in canvas cells, z in pixels).
//
// That recovers the brick lattice, which this shader needs for one thing only —
// where the corners are. It has to come from a uniform rather than from anything
// per-vertex: the mesher merges a run of same-coloured cells into a single quad,
// so by the time the GPU sees the model a brick is not a piece of geometry any
// more. A matrix rather than a scale and a pivot, because the map includes the
// model's orientation. See brickMatrix.
uniform mat4 uBrick;

// Everything below is a row in lampulaParams. A uniform here with no row is a knob
// nobody can reach; a row with no uniform here is a slider that moves nothing, and
// TestEveryExposedParamHasItsUniform refuses the second.
uniform float uVibrance;
uniform float uAlpha;
uniform float uAmbient;
uniform float uExposure;

uniform float uLamp0On;
uniform float uLamp1On;
uniform float uLamp2On;
uniform vec3  uLamp0;
uniform vec3  uLamp1;
uniform vec3  uLamp2;

uniform float uOrbitOn;
uniform float uOrbitRate;
uniform float uOrbitDist;
uniform float uOrbitLift;
uniform float uFalloff;

uniform float uDiffuse;
uniform float uWrap;
uniform float uTransOn;
uniform float uTransPow;
uniform float uTransGain;

uniform float uSpecOn;
uniform float uSpecPow;
uniform float uSpecGain;
uniform float uEnvOn;
uniform float uFresPow;
uniform float uEnvGain;
uniform vec3  uSky;
uniform vec3  uGround;

uniform float uSparkOn;
uniform float uGlintPow;
uniform float uSparkGain;
uniform float uCornerTilt;
uniform float uRayFall;
uniform float uSpike;

uniform float uEdgeOn;
uniform float uEdge;

// How many lamps circle. Compiled in because it is an array size — see lampCount
// on the Go side, which is why this one is not a uniform.
const int LAMPS = 3;

const vec3  LUMA = vec3(0.299, 0.587, 0.114);
const float TAU  = 6.2831853;

// How far inside its own brick a wall fragment is sampled. Enough to land on the
// right side of a boundary it sits exactly on, far too little to move the picture,
// and so not worth a slider.
const float INSET = 0.002;

// vibrance pushes a colour away from grey by an amount that shrinks as it gets more
// saturated, so what is already vivid is left where it is.
//
// A flat saturation boost instead would clip exactly the colours that had the least
// room, and a picture whose vivid tones have all arrived at the gamut edge has fewer
// distinguishable colours than the one that went in.
vec3 vibrance(vec3 c, float amount) {
    float sat = max(max(c.r, c.g), c.b) - min(min(c.r, c.g), c.b);
    return mix(vec3(dot(c, LUMA)), c, 1.0 + amount * (1.0 - sat));
}

// Where lamp i is now. On a circle around the model, sized from its own bounding
// box so a small sprite and a large one are lit alike.
//
// The phase is passed in rather than read from uTime here, because uOrbitOn stops
// the clock rather than the lamps: at 0 they hold the arrangement they were in,
// which is what lets every other slider on the pane be judged against a picture
// that is holding still.
vec3 lampAt(int i, float orbit, float phase) {
    float a = phase + float(i) * TAU / float(LAMPS);
    return uBlobCentre + vec3(cos(a), uOrbitLift, sin(a)) * orbit;
}

void main() {
    vec4 tex = texture(texture0, fragTexCoord) * colDiffuse;

    // The artwork's own colours, which is the whole difference from Golden Claude —
    // that one reads luminance and takes every colour from a light.
    vec3 base = vibrance(tex.rgb, uVibrance);

    vec3 n = normalize(fragNormal);
    vec3 v = normalize(uEye - fragWorld);

    // Into brick space: the mesher's conversion, run backwards. uBrick's own 3x3
    // is that conversion for a *direction*, and the normal has to make the same
    // crossing as the lattice or a wall's own axis comes out pointing into the
    // brick — see the longer note in golden.go, which is the same crossing and was
    // wrong here in the same way.
    mat3 toBrick = mat3(uBrick);
    vec3 nb = normalize(toBrick * n);
    vec3 b  = (uBrick * vec4(fragWorld, 1.0)).xyz;

    // A wall sits exactly on the boundary between two bricks, so which one it
    // belongs to has to be said rather than rounded to: step inside along the
    // normal first.
    vec3 q = fract(b - nb * INSET);

    // Where this fragment is inside its brick. The wall's own axis is set rather
    // than measured: the fragment is *on* the wall, which is exactly half a brick
    // out, and a layer of odd depth puts its faces half a brick off this lattice
    // in z.
    vec3 d = q - 0.5;
    d = mix(d, 0.5 * nb, abs(nb));

    // The two axes that run across this wall, and this fragment's place between
    // them. Everything to do with corners is in these coordinates.
    vec3  up = (abs(nb).y > 0.5) ? vec3(0.0, 0.0, 1.0) : vec3(0.0, 1.0, 0.0);
    vec3  t1 = normalize(cross(up, nb));
    vec3  t2 = cross(nb, t1);
    vec2  f  = vec2(dot(d, t1), dot(d, t2));

    // The nearest corner of this brick's face, and where we stand relative to it.
    vec2  rel = f - sign(f) * 0.5;
    float cd  = length(rel);

    // A normal leaning towards that corner. Only the sparks use it, and it is what
    // staggers them: a lamp reaches the four corners of a face at four different
    // moments instead of all at once.
    //
    // Built in brick space, where t1 and t2 live, and carried back out on the way —
    // dotting a brick-space normal against a world-space half-vector is the one
    // mistake this arrangement invites. The 3x3 is a uniform scale times a
    // rotation, so its transpose is that rotation undone.
    vec3 nbTilt  = normalize(nb + (t1 * f.x + t2 * f.y) * uCornerTilt);
    vec3 nCorner = normalize(transpose(toBrick) * nbTilt);

    float orbit = max(max(uBlobRadius.x, uBlobRadius.y), uBlobRadius.z) * uOrbitDist;
    float phase = uTime * uOrbitRate * uOrbitOn;

    // The lamps and their switches. A lamp that is off contributes nothing to any
    // of the four terms below, which is what makes "what is this one doing?"
    // answerable by turning the other two off.
    vec3  lampCol[3] = vec3[3](uLamp0, uLamp1, uLamp2);
    float lampOn[3]  = float[3](uLamp0On, uLamp1On, uLamp2On);

    vec3 lit   = base * uAmbient;
    vec3 gloss = vec3(0.0);
    vec3 spark = vec3(0.0);

    for (int i = 0; i < LAMPS; i++) {
        vec3  toLamp = lampAt(i, orbit, phase) - fragWorld;
        float dist   = length(toLamp);
        vec3  L      = toLamp / max(dist, 1e-4);
        vec3  col    = lampCol[i] * lampOn[i];

        // Falloff measured in orbit radii rather than world units, so the picture
        // does not change when the model does.
        float dn    = dist / max(orbit, 1e-4);
        float atten = 1.0 / (1.0 + dn * dn * uFalloff);

        // Wrapped diffuse: no hard shadow line, because glass has none.
        float diff = max((dot(n, L) + uWrap) / (1.0 + uWrap), 0.0);
        lit += base * col * (diff * uDiffuse * atten);

        // Light through the model, which fires when the lamp is behind what you are
        // looking at. This is what the circling is for.
        float trans = pow(max(dot(-L, v), 0.0), uTransPow);
        lit += base * col * (trans * uTransGain * uTransOn * atten);

        vec3 h = normalize(L + v);
        gloss += col * (pow(max(dot(n, h), 0.0), uSpecPow) * uSpecGain * uSpecOn * atten);

        // The glint, off the corner-leaning normal, so each corner has its own
        // moment.
        spark += col * (pow(max(dot(nCorner, h), 0.0), uGlintPow) * atten);
    }

    // The room, reflected — gathered towards the silhouette, where a transparent
    // thing stops letting you through it and starts handing back what is behind you.
    float fres = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), uFresPow);
    vec3  refl = reflect(-v, n);
    vec3  col  = lit + gloss
               + mix(uGround, uSky, clamp(refl.y * 0.5 + 0.5, 0.0, 1.0))
                 * (fres * uEnvGain * uEnvOn);

    // The rays. A star centred on the nearest corner, narrowed into arms rather
    // than left as a blob, fading with distance from it — and only where a lamp is
    // lined up on that corner, which is what the glint above decides.
    float star = pow(abs(cos(atan(rel.y, rel.x) * 2.0)), uSpike);
    col += spark * (exp(-cd * uRayFall) * star * uSparkGain * uSparkOn);

    // Just enough shade along a brick edge that a run of identically coloured
    // bricks is still a run of bricks. A corner nobody can see is a poor place to
    // hang a spark.
    vec3 e = min(q, 1.0 - q) + abs(nb);
    col *= 1.0 - uEdge * uEdgeOn * (1.0 - smoothstep(0.0, 0.10, min(min(e.x, e.y), e.z)));

    col = vec3(1.0) - exp(-max(col, vec3(0.0)) * uExposure);

    // The artwork's own transparency, taken down. A layer's opacity is baked into
    // the atlas, so tex.a is what the document says and uAlpha is what this mode
    // adds on top of it — a fully transparent pixel stays fully transparent.
    finalColor = vec4(col, tex.a * uAlpha);
}
