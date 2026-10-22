#version 330 core
uniform sampler2D source;
uniform vec2 texel;
uniform float height_ratio;
uniform float density;
uniform float viewport_height;
uniform float fade_height;
uniform vec3 background;
in vec2 uv;
out vec4 color;
void main() {
    float distance_from_bottom = uv.y * viewport_height;
    float blur_mask = 1.0 - smoothstep(24.0, 72.0, distance_from_bottom);
    float opacity = clamp(distance_from_bottom / fade_height, 0.0, 1.0);
    vec2 center = vec2(uv.x, uv.y * height_ratio);
    vec2 spread = texel * density;
    vec4 sharp = texture(source, center);
    if (blur_mask <= 0.0) {
        color = vec4(mix(background, sharp.rgb, opacity), 1.0);
        return;
    }
    vec4 sum = vec4(0.0);
    float total = 0.0;
    for (int y = -6; y <= 6; ++y) {
        for (int x = -6; x <= 6; ++x) {
            float weight = exp(-float(x*x + y*y) / 18.0);
            sum += texture(source, center + vec2(x,y) * spread) * weight;
            total += weight;
        }
    }
    vec3 softened = mix(sharp.rgb, (sum / total).rgb, blur_mask);
    color = vec4(mix(background, softened, opacity), 1.0);
}
