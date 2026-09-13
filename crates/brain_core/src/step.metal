#include <metal_stdlib>
using namespace metal;
// One SIMD group owns each row. No atomic float reductions or in-place state updates.
kernel void csr_step(
    constant uint &n [[buffer(0)]], device const uint *rows [[buffer(1)]],
    device const uint *columns [[buffer(2)]], device const float *weights [[buffer(3)]],
    device const float *activity [[buffer(4)]], device const float *input [[buffer(5)]],
    device const float *noise [[buffer(6)]], device float *out [[buffer(7)]],
    constant float &retention [[buffer(8)]], constant float &recurrence [[buffer(9)]], constant float &update_gain [[buffer(10)]],
    uint tid [[thread_position_in_grid]], ushort lane [[thread_index_in_simdgroup]]) {
    uint row = tid / 32;
    if (row >= n) return;
    float total = 0.0f;
    for (uint edge = rows[row] + lane; edge < rows[row + 1]; edge += 32)
        total += weights[edge] * activity[columns[edge]];
    total = simd_sum(total);
    if (lane == 0) {
        float drive = (recurrence * total + input[row]) + noise[row];
        // The reference uses independently rounded float32 coefficients 0.7 and 0.3.
        out[row] = retention * activity[row] + update_gain * tanh(max(drive, 0.0f));
    }
}
