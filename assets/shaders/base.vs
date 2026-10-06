#version 330

// Minimal vertex shader that forwards normals — raylib's default one doesn't,
// and the toon shader needs them for banded lighting. Kept deliberately
// simple; when the dragon GLB arrives, add matNormal handling here.
in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec4 vertexColor;
in vec3 vertexNormal;

uniform mat4 mvp;

out vec2 fragTexCoord;
out vec4 fragColor;
out vec3 fragNormal;

void main() {
    fragTexCoord = vertexTexCoord;
    fragColor = vertexColor;
    fragNormal = vertexNormal;
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
