@group(0) @binding(0) var<storage, read_write> words: array<u32>;
@compute @workgroup_size(1) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    words[id.x] = 2u * words[id.x] + 1u;
}
