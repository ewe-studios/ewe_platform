@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var result: texture_storage_2d<rgba8unorm, write>;
@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let point = vec2<i32>(id.xy);
    let color = textureLoad(source, point, 0);
    textureStore(result, point, vec4<f32>(color.b, color.g, color.r, color.a));
}
