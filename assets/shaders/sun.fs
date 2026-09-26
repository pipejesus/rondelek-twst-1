#version 330

// The score sun: banded like the toon hero, but lit from the front (the
// camera's side), so the face the kid reads keeps its full colour. The
// extruded pixel sides fall into shade as it whirls, which is what makes the
// spin read as a real 3D turn rather than a flat coin squashing.
in vec2 fragTexCoord;
in vec4 fragColor;
in vec3 fragNormal;

uniform sampler2D texture0;
uniform vec4 colDiffuse;

out vec4 finalColor;

const vec3 lightDir = normalize(vec3(0.3, 0.1, 1.0));
const float BANDS = 3.0;

void main() {
    vec4 base = texture(texture0, fragTexCoord) * fragColor * colDiffuse;
    float n = max(dot(normalize(fragNormal), lightDir), 0.0);
    float band = floor(n * BANDS + 0.5) / BANDS;
    float shade = 0.6 + 0.4 * band;
    finalColor = vec4(base.rgb * shade, base.a);
}
