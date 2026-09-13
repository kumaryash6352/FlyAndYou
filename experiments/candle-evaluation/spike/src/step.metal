#include <metal_stdlib>
using namespace metal;

// Experimental fused CSR recurrence. Row ownership avoids global floating atomics.
kernel void csr_serial(
    constant uint &n [[buffer(0)]], device const uint *rows [[buffer(1)]],
    device const uint *cols [[buffer(2)]], device const float *weights [[buffer(3)]],
    device const float *activity [[buffer(4)]], device const float *input [[buffer(5)]],
    device const float *noise [[buffer(6)]], device float *out [[buffer(7)]],
    uint row [[thread_position_in_grid]]) {
    if (row >= n) return;
    float total = 0.0f;
    for (uint edge = rows[row]; edge < rows[row+1]; ++edge)
        total += weights[edge] * activity[cols[edge]];
    float drive = (0.9f * total + input[row]) + noise[row];
    out[row] = 0.7f * activity[row] + 0.3f * tanh(max(drive, 0.0f));
}

kernel void csr_simd(
    constant uint &n [[buffer(0)]], device const uint *rows [[buffer(1)]],
    device const uint *cols [[buffer(2)]], device const float *weights [[buffer(3)]],
    device const float *activity [[buffer(4)]], device const float *input [[buffer(5)]],
    device const float *noise [[buffer(6)]], device float *out [[buffer(7)]],
    uint tid [[thread_position_in_grid]], ushort lane [[thread_index_in_simdgroup]]) {
    uint row = tid / 32;
    if (row >= n) return;
    float total = 0.0f;
    for (uint edge = rows[row] + lane; edge < rows[row+1]; edge += 32)
        total += weights[edge] * activity[cols[edge]];
    total = simd_sum(total);
    if (lane == 0) {
        float drive = (0.9f * total + input[row]) + noise[row];
        out[row] = 0.7f * activity[row] + 0.3f * tanh(max(drive, 0.0f));
    }
}
