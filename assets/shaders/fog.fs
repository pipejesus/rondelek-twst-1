#version 330

// Fog for the far mountain layer: mix toward the sky color, a little thicker
// toward the bottom of each block face (valley mist). fogAmount is set per
// layer from Rust; works with raylib's default vertex shader.
in vec2 fragTexCoord;
in vec4 fragColor;

uniform sampler2D texture0;
uniform vec4 colDiffuse;
uniform vec4 fogColor;
uniform float fogAmount;

out vec4 finalColor;

void main() {
    vec4 base = texture(texture0, fragTexCoord) * fragColor * colDiffuse;
    float a = clamp(fogAmount + (1.0 - fragTexCoord.y) * 0.15, 0.0, 1.0);
    finalColor = vec4(mix(base.rgb, fogColor.rgb, a), base.a);
}
