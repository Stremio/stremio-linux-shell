#version 300 es
precision highp float;

uniform sampler2D text_uniform;
in vec2 v_texcoord;
out vec4 frag_color;

void main() {
    frag_color = texture(text_uniform, vec2(v_texcoord.x, 1.0 - v_texcoord.y));
}
