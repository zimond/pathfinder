// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct main0_out
{
    float2 vMaskTexCoord [[user(locn0)]];
    float2 vClipTexCoord [[user(locn1)]];
    float vMaskBackdrop [[user(locn2)]];
    float vClipBackdrop [[user(locn3)]];
    float4 gl_Position [[position]];
};

struct main0_in
{
    float2 aTilePosition [[attribute(0)]];
    float2 aMaskTexCoord [[attribute(1)]];
    float2 aClipTexCoord [[attribute(2)]];
    int aMaskBackdrop [[attribute(3)]];
    int aClipBackdrop [[attribute(4)]];
};

vertex main0_out main0(main0_in in [[stage_in]])
{
    main0_out out = {};
    float2 position = mix(float2(-1.0), float2(1.0), in.aTilePosition);
    out.vMaskTexCoord = in.aMaskTexCoord;
    out.vClipTexCoord = in.aClipTexCoord;
    out.vMaskBackdrop = float(in.aMaskBackdrop);
    out.vClipBackdrop = float(in.aClipBackdrop);
    out.gl_Position = float4(position, 0.0, 1.0);
    return out;
}

