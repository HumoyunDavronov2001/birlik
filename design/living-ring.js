/* Omnarc — the living ring (WebGL2, no dependencies).
 *
 * The mark shows how the system actually works:
 *   - every arc is one device; its length is the device's share of model layers;
 *   - particles inside an arc are its compute; the spot a token is passing through lights up;
 *   - gold comets are tokens; as many tokens are in flight as there are devices (continuous pipelining);
 *   - faint sparks ahead of a comet are draft tokens; rejected drafts fade out;
 *   - gaps between arcs are network hops; the top gap is the output, and every finished token sends out a wave.
 *
 * livingRing(canvas, options) -> { frame(ms), addDevice(), reset(), setTheme(), resize(), destroy(), deviceCount() }
 * Returns null when WebGL2 is unavailable, so the caller can fall back to the static SVG mark.
 * ES module; design/build.py strips the `export` keyword when it inlines the file into the concept page.
 */
export function livingRing(cv, O) {
  'use strict';   // also strict when inlined into a classic script
  const gl = cv.getContext('webgl2', { antialias: false, alpha: true, premultipliedAlpha: true });
  if (!gl) return null;

  const TAU = Math.PI * 2;
  const MAX = 8, LAYERS = 64, GAP = 0.13, HOP = 0.075;   // GAP — angle between arcs (rad); HOP — network time (s)
  const PARTICLES = 15000, PARTICLES_RESERVE = 1800, GLOW = 1700, GLOW_RESERVE = 260, TAIL = 28, WAVE = 160;

  /* ---------------- shaders ---------------- */
  const VS_PARTICLES = `#version 300 es
precision highp float;
layout(location=0) in vec4 a_pos;     // u along the arc, radial offset, z offset, arc index
layout(location=1) in vec4 a_meta;    // size, phase, intro delay, kind: 0 core, 1 dust, 2 filament, 3 glow
layout(location=2) in vec3 a_cloud;   // starting point in the cloud (intro)
uniform float u_time, u_intro, u_radius, u_dpr, u_focal, u_hover, u_alpha, u_dust;
uniform vec2 u_viewport, u_tilt;
uniform vec2 u_arcs[8];
uniform float u_flow[8], u_join[8];
uniform vec4 u_tokens[8];
uniform int u_tokenCount;
uniform vec3 u_color0, u_color1, u_color2;
out vec3 v_color; out float v_alpha; out float v_hard;
vec3 rotX(vec3 p, float a) { float c = cos(a), s = sin(a); return vec3(p.x, p.y * c - p.z * s, p.y * s + p.z * c); }
vec3 rotY(vec3 p, float a) { float c = cos(a), s = sin(a); return vec3(p.x * c + p.z * s, p.y, -p.x * s + p.z * c); }
void main() {
  int k = int(a_pos.w + 0.5);
  vec2 arc = u_arcs[k];
  float span = arc.y - arc.x;
  float u = fract(a_pos.x + u_time * u_flow[k]);
  float ang = arc.x + span * u;
  float ends = smoothstep(0.0, 0.045, min(u, 1.0 - u) * span);   // crisp arc ends
  float rr = 1.0 + a_pos.y;
  vec3 ring = vec3(sin(ang) * rr, cos(ang) * rr, a_pos.z);
  float P = min(u_intro, u_join[k]);
  float e = clamp((P * 1.5 - a_meta.z) / 0.6, 0.0, 1.0);
  e = e * e * (3.0 - 2.0 * e);
  float swirl = (1.0 - e) * 1.7;
  vec3 c = vec3(a_cloud.x * cos(swirl) - a_cloud.y * sin(swirl), a_cloud.x * sin(swirl) + a_cloud.y * cos(swirl), a_cloud.z);
  vec3 p = rotY(rotX(mix(c, ring, e) * u_radius, u_tilt.x), u_tilt.y);
  float s = u_focal / (u_focal - p.z);
  gl_Position = vec4(p.xy * s / (u_viewport * 0.5), 0.0, 1.0);
  float g = 0.0;
  for (int i = 0; i < 8; i++) {
    if (i >= u_tokenCount) break;
    float d = mod(abs(ang - u_tokens[i].x), 6.2831853);
    d = min(d, 6.2831853 - d);
    g += u_tokens[i].y * (exp(-d * d / 0.006) + 0.22 * exp(-d * d / 0.06));
  }
  g *= e;
  int kind = int(a_meta.w + 0.5);
  float twinkle = kind == 3 ? 1.0 : 0.78 + 0.22 * sin(u_time * (1.1 + a_meta.y * 0.35) + a_meta.y * 7.0);
  float base = kind == 0 ? 0.40 : kind == 1 ? 0.15 * u_dust : kind == 2 ? 0.62 : 0.055;
  float response = kind == 0 ? 0.5 : kind == 1 ? 0.3 : kind == 2 ? 0.65 : 0.08;
  float hover = abs(float(k) - u_hover) < 0.5 ? (kind == 3 ? 0.05 : 0.3) : 0.0;
  float b = base * twinkle + g * response + hover;
  float visible = smoothstep(0.0, 0.04, span);
  v_color = mix(u_color0, u_color1, clamp((kind == 3 ? 0.2 : b * 1.1), 0.0, 1.0));
  v_color = mix(v_color, u_color2, clamp(g * 0.55, 0.0, 0.7) * (kind == 3 ? 0.3 : 1.0));   // warm where a token passes
  v_alpha = clamp(b + (1.0 - e) * 0.25, 0.0, 1.0) * mix(0.6, 1.0, e) * visible * mix(1.0, ends, e) * u_alpha;
  v_hard = kind == 2 ? 0.35 : 0.0;
  float size = kind == 3 ? a_meta.x * u_radius : a_meta.x;
  gl_PointSize = size * u_dpr * s * (kind == 3 ? 1.0 : 1.0 + 0.45 * g);
}`;
  const VS_POINTS = `#version 300 es
precision highp float;
layout(location=0) in vec4 a_point;   // angle, radius, z, size
layout(location=1) in vec4 a_style;   // alpha, color mix, hardness
uniform float u_radius, u_dpr, u_focal, u_spin, u_alpha;
uniform vec2 u_viewport, u_tilt;
uniform vec3 u_color0, u_color1;
out vec3 v_color; out float v_alpha; out float v_hard;
vec3 rotX(vec3 p, float a) { float c = cos(a), s = sin(a); return vec3(p.x, p.y * c - p.z * s, p.y * s + p.z * c); }
vec3 rotY(vec3 p, float a) { float c = cos(a), s = sin(a); return vec3(p.x * c + p.z * s, p.y, -p.x * s + p.z * c); }
void main() {
  float ang = a_point.x + u_spin;
  vec3 p = rotY(rotX(vec3(sin(ang) * a_point.y, cos(ang) * a_point.y, a_point.z) * u_radius, u_tilt.x), u_tilt.y);
  float s = u_focal / (u_focal - p.z);
  gl_Position = vec4(p.xy * s / (u_viewport * 0.5), 0.0, 1.0);
  gl_PointSize = a_point.w * u_dpr * s;
  v_color = mix(u_color0, u_color1, a_style.y);
  v_alpha = a_style.x * u_alpha;
  v_hard = a_style.z;
}`;
  const FS = `#version 300 es
precision mediump float;
in vec3 v_color; in float v_alpha; in float v_hard;
out vec4 o;
void main() {
  vec2 d = gl_PointCoord * 2.0 - 1.0;
  float r2 = dot(d, d);
  if (r2 > 1.0) discard;
  float a = mix(exp(-r2 * 3.2), 1.0 - smoothstep(0.55, 1.0, r2), v_hard) * v_alpha;
  o = vec4(v_color * a, a);
}`;
  const shader = (type, source) => {
    const s = gl.createShader(type); gl.shaderSource(s, source); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const program = (vs) => {
    const p = gl.createProgram();
    gl.attachShader(p, shader(gl.VERTEX_SHADER, vs)); gl.attachShader(p, shader(gl.FRAGMENT_SHADER, FS));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    const u = {};
    for (let i = 0, n = gl.getProgramParameter(p, gl.ACTIVE_UNIFORMS); i < n; i++) {
      const name = gl.getActiveUniform(p, i).name.replace(/\[0\]$/, '');
      u[name] = gl.getUniformLocation(p, name);
    }
    return { p, u };
  };
  let PP, PQ;
  try { PP = program(VS_PARTICLES); PQ = program(VS_POINTS); } catch (x) { console.warn('living ring:', x); return null; }

  /* ---------------- deterministic random numbers ---------------- */
  let seed = 20261012;
  const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  const gauss = () => Math.sqrt(-2 * Math.log(rand() + 1e-9)) * Math.cos(TAU * rand());

  /* ---------------- state ---------------- */
  const all = O.devices.concat(O.reserve).slice(0, MAX);
  let n = O.devices.length;
  let layers = [];
  const splitLayers = () => {   // largest remainder method: always exactly 64 layers in total
    const power = all.slice(0, n).map(d => d.power), total = power.reduce((a, b) => a + b, 0);
    const raw = power.map(p => LAYERS * p / total), whole = raw.map(Math.floor);
    let left = LAYERS - whole.reduce((a, b) => a + b, 0);
    raw.map((v, i) => [v - whole[i], i]).sort((a, b) => b[0] - a[0]).forEach(([, i]) => { if (left > 0) { whole[i]++; left--; } });
    layers = whole;
  };
  splitLayers();
  const targetArcs = () => {
    const room = TAU - n * GAP; let a = GAP / 2;
    return Array.from({ length: MAX }, (_, k) => {
      if (k >= n) return [TAU - GAP / 2, TAU - GAP / 2];
      const d = room * layers[k] / LAYERS, arc = [a, a + d]; a += d + GAP; return arc;
    });
  };
  let arcs = targetArcs(), target = arcs.map(a => a.slice());
  const join = Array.from({ length: MAX }, (_, k) => (k < n ? 1 : 0));
  let TIME_K = 0;
  const stageTimes = () => all.slice(0, n).map((d, k) => TIME_K * layers[k] / d.power * (d.slowdown || 1));
  TIME_K = (3.9 - n * HOP) / all.slice(0, n).reduce((s, d, k) => s + layers[k] / d.power * (d.slowdown || 1), 0);
  let stage = stageTimes();
  let T = stage.reduce((a, b) => a + b, 0) + n * HOP;
  let phase = Array.from({ length: n }, (_, i) => i / n);
  const tokenAngles = new Float32Array(MAX * 2).fill(0);

  /* ---------------- particles (built once) ---------------- */
  const counts = Array.from({ length: MAX }, (_, k) => (k < n ? Math.round(PARTICLES * layers[k] / LAYERS) : PARTICLES_RESERVE));
  const glowCounts = Array.from({ length: MAX }, (_, k) => (k < n ? Math.round(GLOW * layers[k] / LAYERS) : GLOW_RESERVE));
  const TOTAL = counts.reduce((a, b) => a + b, 0) + glowCounts.reduce((a, b) => a + b, 0);
  const data = new Float32Array(TOTAL * 11);
  let j = 0;
  const delay = (k) => (k < n ? Math.min(0.88, 0.08 + k * 0.11 + rand() * 0.3) : rand() * 0.75);
  const cloud = () => { const az = rand() * TAU, pol = Math.acos(2 * rand() - 1), r = 1.7 + rand() * 1.1; return [r * Math.sin(pol) * Math.cos(az), r * Math.sin(pol) * Math.sin(az), r * Math.cos(pol) * 0.6]; };
  counts.forEach((m, k) => {
    for (let i = 0; i < m; i++) {
      const r = rand(), kind = r < 0.16 ? 1 : r < 0.32 ? 2 : 0;          // dust, filament, core
      const sigma = kind === 1 ? 0.085 : kind === 2 ? 0.0045 : 0.034;
      const size = kind === 1 ? 0.8 + rand() * 0.8 : kind === 2 ? 1.0 + rand() * 0.5 : 1.2 + rand() * 1.3;
      data.set([rand(), gauss() * sigma, gauss() * sigma, k, size, rand() * 6.2832, delay(k), kind, ...cloud()], j); j += 11;
    }
  });
  glowCounts.forEach((m, k) => {                                          // glow: a soft band of light along the arc
    for (let i = 0; i < m; i++) { data.set([(i + 0.5) / m, 0, 0, k, 0.11, rand() * 6.2832, delay(k), 3, ...cloud()], j); j += 11; }
  });
  const particleVao = gl.createVertexArray(); gl.bindVertexArray(particleVao);
  const particleBuf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, particleBuf); gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 44, 0);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 44, 16);
  gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 3, gl.FLOAT, false, 44, 32);

  /* ---------------- point layers: dial, inner ring, world (static) and dynamic ---------------- */
  const pointVao = (max, fixed) => {
    const vao = gl.createVertexArray(); gl.bindVertexArray(vao);
    const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, fixed || new Float32Array(max * 8), fixed ? gl.STATIC_DRAW : gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 32, 0);
    gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 32, 16);
    return { vao, buf };
  };
  const dial = [];
  for (let i = 0; i < 180; i++) {
    const a = i / 180 * TAU, major = i % 5 === 0;
    dial.push(a, 1.17, 0, major ? 1.7 : 1.15, major ? 0.55 : 0.3, 0, 1, 0);
    if (major) dial.push(a, 1.21, 0, 1.4, 0.4, 0, 1, 0);
  }
  const DIAL_N = dial.length / 8;
  const inner = [];
  for (let i = 0; i < 96; i++) inner.push(i / 96 * TAU, 0.72, 0, 1.2, 0.22, 0, 1, 0);
  const world = [];                                                        // distant devices around the ring
  for (let i = 0; i < 520; i++) world.push(rand() * TAU, 1.3 + rand() * 1.05, gauss() * 0.18, 0.8 + rand() * 1.1, 0.08 + rand() * 0.26, 0, 0.6, 0);
  const vDial = pointVao(0, new Float32Array(dial)), vInner = pointVao(0, new Float32Array(inner)), vWorld = pointVao(0, new Float32Array(world));
  const DYN_MAX = MAX * (2 + TAIL + 2) + 4 * WAVE + MAX;
  const vDyn = pointVao(DYN_MAX), dyn = new Float32Array(DYN_MAX * 8);
  const waves = [];

  /* ---------------- colors ---------------- */
  let C = {};
  const rgb = (h) => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16) / 255);
  const setTheme = () => {
    C = O.isLight() ? {
      p0: rgb('#4aa9a3'), p1: rgb('#00524e'), p2: rgb('#d08a1c'), t0: rgb('#d89a2b'), t1: rgb('#a86500'),
      dial: rgb(O.cssVar('--text-3')), alpha: 0.9, dust: 0.45, additive: false,
    } : {
      p0: rgb('#0b7a75'), p1: rgb('#9ff6ef'), p2: rgb(O.cssVar('--gold-mark')), t0: rgb(O.cssVar('--gold-mark')), t1: rgb('#fff6e0'),
      dial: rgb(O.cssVar('--text-3')), alpha: 1, dust: 1, additive: true,
    };
  };
  setTheme();

  /* ---------------- size ---------------- */
  let W = 1, H = 1, dpr = 1, Rpx = 1, F = 1;
  const resize = () => {
    dpr = Math.min(2, devicePixelRatio || 1);
    W = cv.clientWidth || 1; H = cv.clientHeight || W;
    cv.width = Math.round(W * dpr); cv.height = Math.round(H * dpr);
    Rpx = Math.min(W, H) * (W < 420 ? 0.36 : 0.31); F = Rpx * 3.4;
  };
  resize();

  /* ---------------- tilt and pointer ---------------- */
  const BASE_TILT = 0.36;
  let tilt = [BASE_TILT, 0], tiltTarget = [BASE_TILT, 0], hover = -1, pointerIn = false;
  const project = (ang, r, z) => {
    const x = Math.sin(ang) * r * Rpx, y = Math.cos(ang) * r * Rpx, zz = z * Rpx;
    let c = Math.cos(tilt[0]), s = Math.sin(tilt[0]);
    const y2 = y * c - zz * s, z2 = y * s + zz * c;
    c = Math.cos(tilt[1]); s = Math.sin(tilt[1]);
    const x3 = x * c + z2 * s, z3 = -x * s + z2 * c, k = F / (F - z3);
    return [W / 2 + x3 * k, H / 2 - y2 * k];
  };
  const locate = (ex, ey) => {
    const b = cv.getBoundingClientRect(), x = ex - b.left - b.width / 2, y = ey - b.top - b.height / 2;
    return { nx: x / (b.width / 2), ny: y / (b.height / 2), r: Math.hypot(x, y / Math.cos(tilt[0])) / Rpx, a: (Math.atan2(x, -y / Math.cos(tilt[0])) + TAU) % TAU };
  };
  const host = O.host || cv.parentElement;
  const onMove = (e) => {
    const q = locate(e.clientX, e.clientY);
    pointerIn = true;
    tiltTarget = [BASE_TILT + Math.max(-1, Math.min(1, q.ny)) * 0.2, Math.max(-1, Math.min(1, q.nx)) * 0.28];
    hover = -1;
    if (q.r > 0.78 && q.r < 1.32) for (let k = 0; k < n; k++) if (q.a >= arcs[k][0] && q.a <= arcs[k][1]) hover = k;
  };
  const onLeave = () => { pointerIn = false; tiltTarget = [BASE_TILT, 0]; hover = -1; };
  host.addEventListener('pointermove', onMove);
  host.addEventListener('pointerleave', onLeave);

  /* ---------------- labels (HTML) ---------------- */
  const labels = [];
  if (O.labelsEl) {
    O.labelsEl.innerHTML = '';
    all.forEach(() => { const e = document.createElement('span'); O.labelsEl.appendChild(e); labels.push({ e, o: 0.45 }); });
  }
  const labelText = O.labelText || ((d, count) => `${d.backend} · ${count} layers`);
  const writeLabels = () => labels.forEach((l, k) => {
    if (k >= n) { l.e.style.opacity = 0; return; }
    l.e.innerHTML = `<b>${all[k].name}</b>${labelText(all[k], layers[k])}`;
  });
  writeLabels();
  const writeStatus = () => { if (O.statusEl) O.statusEl.textContent = O.statusText(n, LAYERS); };
  writeStatus();

  /* ---------------- token position ---------------- */
  const angleAt = (tau) => {
    tau = ((tau % T) + T) % T;
    for (let k = 0; k < n; k++) {
      if (tau < stage[k]) return arcs[k][0] + (arcs[k][1] - arcs[k][0]) * tau / stage[k];
      tau -= stage[k];
      if (tau < HOP) { const next = k === n - 1 ? arcs[0][0] + TAU : arcs[k + 1][0]; return (arcs[k][1] + (next - arcs[k][1]) * tau / HOP) % TAU; }
      tau -= HOP;
    }
    return arcs[0][0];
  };
  const hash = (x) => { const s = Math.sin(x * 12.9898) * 43758.5453; return s - Math.floor(s); };

  /* ---------------- frame ---------------- */
  let prev = null, start = null, intro = O.reducedMotion ? 1 : 0;
  const joinedAt = Array(MAX).fill(null);
  const put = (i, a, r, z, size, alpha, mix, hard) => { const o = i * 8; dyn[o] = a; dyn[o + 1] = r; dyn[o + 2] = z; dyn[o + 3] = size; dyn[o + 4] = alpha; dyn[o + 5] = mix; dyn[o + 6] = hard; dyn[o + 7] = 0; };
  function frame(ms) {
    const t = ms / 1000;
    if (start === null) start = O.instant ? t - 10 : t;
    const dt = prev === null ? 0 : Math.min(0.05, t - prev); prev = t;
    if (!O.reducedMotion) intro = Math.min(1, (t - start) / 2.6);
    const ease = 1 - Math.exp(-dt * 4.5);
    for (let k = 0; k < MAX; k++) for (let m = 0; m < 2; m++) arcs[k][m] += (target[k][m] - arcs[k][m]) * (O.reducedMotion ? 1 : ease);
    for (let k = 0; k < MAX; k++) if (joinedAt[k] !== null) join[k] = Math.min(1, (t - joinedAt[k]) / 1.9);
    const sway = O.reducedMotion || pointerIn ? 0 : Math.sin(t * 0.37) * 0.045;
    tilt[0] += (tiltTarget[0] + sway - tilt[0]) * (O.reducedMotion ? 1 : 1 - Math.exp(-dt * 3));
    tilt[1] += (tiltTarget[1] + sway * 0.8 - tilt[1]) * (O.reducedMotion ? 1 : 1 - Math.exp(-dt * 3));
    // token phases slowly settle into even spacing
    const tokensOn = intro > 0.82 ? Math.min(1, (intro - 0.82) / 0.18) : 0;
    const order = phase.map((f, i) => [f, i]).sort((a, b) => a[0] - b[0]);
    order.forEach(([f, i], r) => { const goal = (order[0][0] + r / n) % 1; let df = goal - f; df -= Math.round(df); phase[i] = (phase[i] + df * dt * 0.6 + 1) % 1; });
    let m = 0;
    for (let k = 0; k < n - 1; k++) put(m++, (arcs[k][1] + arcs[k + 1][0]) / 2, 1, 0, 3.2, 0.5 * intro, 0, 0.8);   // network gaps
    const GAPS_N = m;
    put(m++, ((arcs[n - 1][1] + arcs[0][0] + TAU) / 2) % TAU, 1, 0, 4.6, 0.8 * intro, 0.2, 0.8);           // output point
    for (let i = 0; i < n; i++) {
      const before = phase[i];
      phase[i] = (phase[i] + (O.reducedMotion ? 0 : dt / T)) % 1;
      if (phase[i] < before && tokensOn > 0.5) waves.push(t);
      const tau = phase[i] * T, a = angleAt(tau), cycle = Math.floor(t / T + i * 0.37);
      tokenAngles[i * 2] = a; tokenAngles[i * 2 + 1] = tokensOn;
      put(m++, a, 1, 0, 30, 0.2 * tokensOn, 0, 0);                        // halo
      for (let q = 0; q < TAIL; q++) {                                       // tail
        const p = q / TAIL;
        put(m++, angleAt(tau - q * 0.012 * T), 1, 0, 1 + 7 * Math.pow(1 - p, 0.8), 0.62 * Math.pow(1 - p, 1.6) * tokensOn, 0.25, 0.3);
      }
      put(m++, a, 1, 0, 9.5, tokensOn, 0.85, 0.35);                          // head
      for (let g = 0; g < 2; g++) {                                          // draft sparks
        const tg = tau + (g + 1) * 0.026 * T, f = (tg % T) / T;
        const kept = hash(cycle * 7 + i * 3 + g) < 0.28 && f > 0.76 ? Math.max(0, 1 - (f - 0.76) / 0.12) : 1;
        put(m++, angleAt(tg), 1, 0, 4.2, 0.5 * kept * tokensOn, 0.6, 0.2);
      }
    }
    while (waves.length && t - waves[0] > 1.3) waves.shift();
    waves.slice(-4).forEach(t0 => {                                          // output wave
      const p = (t - t0) / 1.3, e = 1 - Math.pow(1 - p, 3);
      for (let q = 0; q < WAVE; q++) put(m++, q / WAVE * TAU, 1 + 0.42 * e, 0, 1.5, 0.42 * Math.pow(1 - p, 2), 0.3, 0);
    });

    gl.viewport(0, 0, cv.width, cv.height);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, C.additive ? gl.ONE : gl.ONE_MINUS_SRC_ALPHA);

    // dial, inner ring and the distant world
    gl.useProgram(PQ.p);
    const U = PQ.u;
    gl.uniform1f(U.u_radius, Rpx); gl.uniform1f(U.u_dpr, dpr); gl.uniform1f(U.u_focal, F); gl.uniform2f(U.u_viewport, W, H);
    gl.uniform2f(U.u_tilt, tilt[0], tilt[1]); gl.uniform1f(U.u_alpha, intro);
    gl.uniform3fv(U.u_color0, C.dial); gl.uniform3fv(U.u_color1, C.dial);
    gl.uniform1f(U.u_spin, 0); gl.bindVertexArray(vDial.vao); gl.drawArrays(gl.POINTS, 0, DIAL_N);
    gl.uniform1f(U.u_spin, -t * 0.05); gl.bindVertexArray(vInner.vao); gl.drawArrays(gl.POINTS, 0, 96);
    gl.uniform1f(U.u_alpha, 1); gl.uniform1f(U.u_spin, t * 0.012); gl.bindVertexArray(vWorld.vao); gl.drawArrays(gl.POINTS, 0, 520);

    // particles
    gl.useProgram(PP.p);
    const Z = PP.u;
    gl.uniform1f(Z.u_time, t); gl.uniform1f(Z.u_intro, intro); gl.uniform1f(Z.u_radius, Rpx); gl.uniform1f(Z.u_dpr, dpr);
    gl.uniform1f(Z.u_focal, F); gl.uniform1f(Z.u_hover, hover); gl.uniform1f(Z.u_alpha, C.alpha); gl.uniform1f(Z.u_dust, C.dust);
    gl.uniform2f(Z.u_viewport, W, H); gl.uniform2f(Z.u_tilt, tilt[0], tilt[1]);
    gl.uniform2fv(Z.u_arcs, arcs.flat());
    gl.uniform1fv(Z.u_flow, all.map(d => 0.03 * d.power / 2.2).concat(Array(MAX).fill(0)).slice(0, MAX));
    gl.uniform1fv(Z.u_join, join);
    const tok = new Float32Array(MAX * 4);
    for (let i = 0; i < n; i++) { tok[i * 4] = tokenAngles[i * 2]; tok[i * 4 + 1] = tokenAngles[i * 2 + 1]; }
    gl.uniform4fv(Z.u_tokens, tok); gl.uniform1i(Z.u_tokenCount, n);
    gl.uniform3fv(Z.u_color0, C.p0); gl.uniform3fv(Z.u_color1, C.p1); gl.uniform3fv(Z.u_color2, C.p2);
    gl.bindVertexArray(particleVao); gl.drawArrays(gl.POINTS, 0, TOTAL);

    // network gaps in the dial color, then tokens and waves in gold
    gl.useProgram(PQ.p);
    gl.uniform1f(U.u_spin, 0); gl.uniform1f(U.u_alpha, 1);
    gl.bindVertexArray(vDyn.vao); gl.bindBuffer(gl.ARRAY_BUFFER, vDyn.buf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, dyn, 0, m * 8);
    gl.uniform3fv(U.u_color0, C.dial); gl.uniform3fv(U.u_color1, C.dial);
    if (GAPS_N) gl.drawArrays(gl.POINTS, 0, GAPS_N);
    gl.uniform3fv(U.u_color0, C.t0); gl.uniform3fv(U.u_color1, C.t1);
    gl.drawArrays(gl.POINTS, GAPS_N, m - GAPS_N);

    // labels
    labels.forEach((l, k) => {
      if (k >= n) return;
      const mid = (arcs[k][0] + arcs[k][1]) / 2, [x, y] = project(mid, 1.33, 0);
      let busy = false;
      for (let i = 0; i < n; i++) if (tokenAngles[i * 2] >= arcs[k][0] && tokenAngles[i * 2] <= arcs[k][1]) busy = true;
      const goal = hover === k ? 1 : busy ? 0.95 : 0.5;
      l.o += (goal - l.o) * (O.reducedMotion ? 1 : 1 - Math.exp(-dt * 6));
      const left = Math.sin(mid) < -0.15, centered = Math.abs(Math.sin(mid)) <= 0.15;
      l.e.style.transform = `translate(${x.toFixed(1)}px, ${y.toFixed(1)}px) translate(${left ? '-100%' : centered ? '-50%' : '0'}, -50%)`;
      l.e.style.textAlign = left ? 'right' : centered ? 'center' : 'left';
      const shown = Math.min(1, Math.max(0, (intro - 0.7) / 0.3));
      l.e.style.opacity = (l.o * shown * shown * (3 - 2 * shown)).toFixed(3);
    });
  }

  /* ---------------- controls ---------------- */
  const addDevice = () => {
    if (n >= all.length) return false;
    const k = n; n++;
    splitLayers(); target = targetArcs();
    arcs[k] = [arcs[k - 1][1] + GAP, arcs[k - 1][1] + GAP];
    join[k] = 0; joinedAt[k] = (prev ?? 0);
    stage = stageTimes(); T = stage.reduce((a, b) => a + b, 0) + n * HOP;
    const sorted = phase.slice().sort((a, b) => a - b); let widest = 0, slot = 0;
    sorted.forEach((f, i) => { const next = i === sorted.length - 1 ? sorted[0] + 1 : sorted[i + 1]; if (next - f > widest) { widest = next - f; slot = (f + widest / 2) % 1; } });
    phase.push(slot);
    writeLabels(); writeStatus();
    return n < all.length;
  };
  const reset = () => {
    n = O.devices.length; splitLayers(); target = targetArcs();
    for (let k = n; k < MAX; k++) { join[k] = 0; joinedAt[k] = null; }
    stage = stageTimes(); T = stage.reduce((a, b) => a + b, 0) + n * HOP;
    phase = Array.from({ length: n }, (_, i) => i / n);
    start = null; intro = O.reducedMotion ? 1 : 0; arcs = targetArcs();
    writeLabels(); writeStatus();
  };
  const onLost = (e) => { e.preventDefault(); if (O.onContextLost) O.onContextLost(); };
  cv.addEventListener('webglcontextlost', onLost);
  const destroy = () => {   // release listeners and the GPU context
    host.removeEventListener('pointermove', onMove);
    host.removeEventListener('pointerleave', onLeave);
    cv.removeEventListener('webglcontextlost', onLost);
    gl.getExtension('WEBGL_lose_context')?.loseContext();
  };
  return { frame, addDevice, reset, setTheme, resize, destroy, deviceCount: () => n };
}
