#version 330

// pathfinder/shaders/tile_clip.vs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

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

void main() {
    vec2 position = mix(vec2(-1.0), vec2(1.0), aTilePosition);

    vMaskTexCoord = aMaskTexCoord;
    vClipTexCoord = aClipTexCoord;
    vMaskBackdrop = float(aMaskBackdrop);
    vClipBackdrop = float(aClipBackdrop);
    gl_Position = vec4(position, 0.0, 1.0);
}
