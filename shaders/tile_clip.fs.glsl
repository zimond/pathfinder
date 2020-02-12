#version 330

// pathfinder/shaders/tile_clip.fs.glsl
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

precision highp float;

uniform sampler2D uMaskTexture;
uniform sampler2D uClipTexture;

in vec2 vMaskTexCoord;
in vec2 vClipTexCoord;
in float vMaskBackdrop;
in float vClipBackdrop;

out vec4 oFragColor;

void main() {
    // FIXME(#266, pcwalton): Clamp and use fill rule.
    float maskCoverage = abs(texture(uMaskTexture, vMaskTexCoord).r + vMaskBackdrop);
    float clipCoverage = abs(texture(uClipTexture, vClipTexCoord).r + vClipBackdrop);
    gl_FragColor = vec4(min(maskCoverage, clipCoverage));
}
