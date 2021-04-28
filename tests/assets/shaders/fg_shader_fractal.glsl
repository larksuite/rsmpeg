#version 330
out vec4 color;

uniform float iTime;
uniform ivec2 iResolution;

void main()
{
    float m;
    float cx = (0.5 + 0.5 * sin(iTime/3));
    float cy = (0.5 + 0.5 * sin(iTime/11));
    float x = gl_FragCoord.x / iResolution.y;
    float y = gl_FragCoord.y / iResolution.y;
    for (int i = 0; i < 23; ++i) {
        x = abs(x);
        y = abs(y);
        m = x * x  + y * y;
        x = x/m - cx;
        y = y/m - cy;
    }
    color = vec4(m, m, m, 1.);
}