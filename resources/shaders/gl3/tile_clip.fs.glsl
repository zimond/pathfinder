#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform sampler2D uMaskTexture;

in vec2 vMaskTexCoord;
in vec2 vClipTexCoord;
in float vMaskBackdrop;
in float vClipBackdrop;

out vec4 oFragColor;

void main(){

    float maskCoverage = abs(texture(uMaskTexture, vMaskTexCoord). r + vMaskBackdrop);
    float clipCoverage = abs(texture(uMaskTexture, vClipTexCoord). r + vClipBackdrop);
    gl_FragColor = vec4(min(maskCoverage, clipCoverage));
}

