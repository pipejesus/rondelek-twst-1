#version 330

// The hero's comic-style shader slot: banded (posterized) diffuse over the
// base color — chunky cartoon shading on the placeholder brick today, on the
// dragon model's materials tomorrow. Tweak BANDS / lightDir / floor shade to
// taste once the real model is in.
in vec2 fragTexCoord;
in vec4 fragColor;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec4 colDiffuse;

out vec4 finalColor;

const vec3 lightDir = normalize(vec3(0.45, 1.0, 0.65));
const float BANDS = 3.0;

void main() {
    vec4 base = texture(texture0, fragTexCoord) * fragColor * colDiffuse;
    float n = max(dot(normalize(fragNormal), lightDir), 0.0);
    float band = floor(n * BANDS + 0.5) / BANDS;
    float shade = 0.55 + 0.45 * band;
    finalColor = vec4(base.rgb * shade, base.a);
}
