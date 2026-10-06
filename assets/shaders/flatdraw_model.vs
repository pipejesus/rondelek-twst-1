#version 330

// Copied 1:1 from flat-draw (internal/gpu/modelshader.go, modelVS) at flat-draw commit 92d9ac9
// (2026-09-17): the vertex shader flat-draw's stylised model shaders share,
// Lam::pula's included. It displaces nothing, by design (see flat-draw's note:
// the model is a soup of faces, and moving a vertex would open its edges).

in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 matModel;
uniform mat4 matNormal;

out vec2 fragTexCoord;
out vec3 fragNormal;
out vec3 fragWorld;

void main() {
    fragTexCoord = vertexTexCoord;
    fragWorld    = (matModel * vec4(vertexPosition, 1.0)).xyz;
    fragNormal   = normalize((matNormal * vec4(vertexNormal, 0.0)).xyz);
    gl_Position  = mvp * vec4(vertexPosition, 1.0);
}
