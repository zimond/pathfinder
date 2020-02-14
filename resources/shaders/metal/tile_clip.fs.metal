// Automatically generated from files in pathfinder/shaders/. Do not edit!
#include <metal_stdlib>
#include <simd/simd.h>

using namespace metal;

struct spvDescriptorSetBuffer0
{
    texture2d<float> uMaskTexture [[id(0)]];
    sampler uMaskTextureSmplr [[id(1)]];
};

struct main0_out
{
    float4 _gl_FragColor [[color(0)]];
};

struct main0_in
{
    float2 vMaskTexCoord [[user(locn0)]];
    float2 vClipTexCoord [[user(locn1)]];
    float vMaskBackdrop [[user(locn2)]];
    float vClipBackdrop [[user(locn3)]];
};

fragment main0_out main0(main0_in in [[stage_in]], constant spvDescriptorSetBuffer0& spvDescriptorSet0 [[buffer(0)]])
{
    main0_out out = {};
    float maskCoverage = abs(spvDescriptorSet0.uMaskTexture.sample(spvDescriptorSet0.uMaskTextureSmplr, in.vMaskTexCoord).x + in.vMaskBackdrop);
    float clipCoverage = abs(spvDescriptorSet0.uMaskTexture.sample(spvDescriptorSet0.uMaskTextureSmplr, in.vClipTexCoord).x + in.vClipBackdrop);
    out._gl_FragColor = float4(fast::min(maskCoverage, clipCoverage));
    return out;
}

