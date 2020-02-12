#version {{version}}
// Automatically generated from files in pathfinder/shaders/. Do not edit!












precision highp float;

uniform vec2 uTileSize;

in vec2 aTilePosition;
in vec2 aMaskTexCoord;
in vec2 aClipTexCoord;
in int aMaskBackdrop;
in int aClipBackdrop;

out vec2 vMaskTexCoord;
out vec2 vClipTexCoord;
out float vMaskBackdrop;
out float vClipBackdrop;

void main(){
    vec2 position = mix(vec2(- 1.0), vec2(1.0), aTilePosition);

    vMaskTexCoord = aMaskTexCoord;
    vClipTexCoord = aClipTexCoord;
    vMaskBackdrop = float(aMaskBackdrop);
    vClipBackdrop = float(aClipBackdrop);
    gl_Position = vec4(position, 0.0, 1.0);
}

