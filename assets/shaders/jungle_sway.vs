#version 330

// Vowel Runner's jungle: the vertex stage in front of Lam::pula, putting the
// wind in its palms. The fragment stage is flat-draw's lampula.fs, unchanged.
//
// Each vertex's colour says what the wind may do to its corner of the brick
// lattice (props::Sway, built with the tile):
//
//   r  bend:  how far it goes along with its palm's trunk; nothing at the
//             foot, growing toward the crown, all the way on the fronds
//   g  leaf:  how much it flutters and gives in a gust on top of that;
//             nothing at the crown, the most at a frond's tip
//   b  phase: its palm's own beat, so no two sway in step
//
// Everything else is 0 and stands still. Every vertex on a corner has the
// same colour, so the bricks bend together and never open a crack.
//
// As in the brick water, fragWorld is the brick's *resting* place: Lam::pula
// finds the brick edges and corners by running fragWorld back onto the
// lattice, so a swaying brick keeps its own edges instead of sliding against
// them. (The light hardly notices: nothing moves more than a brick or so.)

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;
in vec4 vertexColor;

uniform mat4 mvp;
uniform mat4 matModel;
uniform mat4 matNormal;

uniform float uTime;   // shared with lampula.fs, which orbits its lamps by it

// Rows of WindParams (game/src/sway.rs).
uniform float uTrunkSway;
uniform float uTrunkGust;
uniform float uSwaySpeed;
uniform float uLeafSway;
uniform float uLeafFlutter;
uniform float uFlutterSpeed;
uniform float uLeafGust;
uniform float uLeafLift;
uniform float uGustFlutter;
uniform float uGustEvery;
uniform float uGustSpeed;
uniform float uGustSharp;
uniform float uBuffet;

out vec2 fragTexCoord;
out vec3 fragNormal;
out vec3 fragWorld;

const float TAU = 6.2831853;

// The wind's strength at world x, 0..1: a gust every uGustEvery seconds,
// soft-edged, rolling along the jungle from the right at uGustSpeed, some
// stronger than others, and buffeting a little while it blows.
float gust(float x) {
    float t = uTime + x / uGustSpeed;
    float wave = max(sin(TAU * t / uGustEvery), 0.0);
    float strength = 0.65 + 0.35 * sin(TAU * t / (uGustEvery * 2.618));
    float buffet = 1.0 - uBuffet + uBuffet * sin(TAU * t * 1.7);
    return pow(wave, uGustSharp) * strength * buffet;
}

void main() {
    vec3 rest = (matModel * vec4(vertexPosition, 1.0)).xyz;
    float bend = vertexColor.r;
    float leaf = vertexColor.g;
    float phase = vertexColor.b * TAU;

    // The whole palm: a slow, uneven sway, leaning away from each gust.
    float g = gust(rest.x);
    float sway = 0.6 * sin(uTime * uSwaySpeed + phase)
               + 0.4 * sin(uTime * uSwaySpeed * 0.61 + phase * 1.7);
    float dx = bend * (uTrunkSway * sway - uTrunkGust * g);

    // The fronds on top, never quite still. A slow rise and fall, and a
    // quicker flutter, much more of it in a gust; both run along a frond as
    // a ripple (their phase follows the brick's place), so each frond moves
    // on its own. A gust also blows them along and lifts them.
    float along = dot(rest, vec3(1.3, 0.4, 0.9));
    float bob = uLeafSway * sin(uTime * uSwaySpeed * 2.3 + phase + 0.5 * along);
    float flutter = uLeafFlutter * sin(uTime * uFlutterSpeed + phase + along)
                  * (1.0 + uGustFlutter * g);
    dx += leaf * (-uLeafGust * g + 0.3 * bob + 0.5 * flutter);
    float dy = leaf * (uLeafLift * g + bob + flutter);

    fragTexCoord = vertexTexCoord;
    fragWorld    = rest;
    fragNormal   = normalize((matNormal * vec4(vertexNormal, 0.0)).xyz);
    gl_Position  = mvp * vec4(vertexPosition + vec3(dx, dy, 0.0), 1.0);
}
