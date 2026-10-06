#version 330

// Like base.vs, but turns the normals with the model (matNormal), so a prop
// drawn with a spinning transform — the score sun — gets lit as it turns
// instead of carrying its shading round with it. Only valid for DrawMesh /
// DrawModel draws: raylib doesn't set matNormal for immediate-mode shapes,
// which is why the hero keeps base.vs.
in vec3 vertexPosition;
in vec2 vertexTexCoord;
in vec4 vertexColor;
in vec3 vertexNormal;

uniform mat4 mvp;
uniform mat4 matNormal;

out vec2 fragTexCoord;
out vec4 fragColor;
out vec3 fragNormal;

void main() {
    fragTexCoord = vertexTexCoord;
    fragColor = vertexColor;
    fragNormal = normalize(vec3(matNormal * vec4(vertexNormal, 0.0)));
    gl_Position = mvp * vec4(vertexPosition, 1.0);
}
