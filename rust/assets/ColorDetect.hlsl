// A small circular ROI is copied on the GPU. Only this 4-byte result is read back.
Texture2D<float4> Region : register(t0);
RWByteAddressBuffer Result : register(u0);
cbuffer Parameters : register(b0)
{
    uint Width, Height, CenterX, CenterY;
    uint Red, Green, Blue, Tolerance;
    uint Radius, Padding0, Padding1, Padding2;
};
[numthreads(8,8,1)]
void main(uint3 id : SV_DispatchThreadID)
{
    if(id.x>=Width||id.y>=Height)return;
    int2 d=int2(id.xy)-int2(CenterX,CenterY);
    if(dot(d,d)>int(Radius*Radius))return;
    uint3 value=(uint3)round(saturate(Region.Load(int3(id.xy,0)).rgb)*255.0);
    int3 delta=abs(int3(value)-int3(Red,Green,Blue));
    if(all(delta<=int(Tolerance)))
    {
        uint previous;
        Result.InterlockedOr(0,1,previous);
    }
}
