// WebGPU first, then WebGL2. Instanced additive quads accumulate in linear
// space, then a blit applies a filmic curve so the nebula glows instead of
// looking like a handful of dim grey CPU dots.

const QUAD = new Float32Array([
  -1, -1,  1, -1,  -1, 1,
  -1,  1,  1, -1,   1, 1,
]);

export function isStrongGpu(name) {
  const s = String(name || "").toLowerCase();
  if (/swiftshader|llvmpipe|softpipe|microsoft basic render/.test(s)) return false;
  if (/adreno(?:\s*\(tm\))?\s*(6\d{2}|7\d{2}|8\d{2})/.test(s)) return true;
  if (/adreno/.test(s) && /(6|7|8)\d{2}/.test(s)) return true;
  if (/mali-g(7|8|9)/.test(s) || /mali-g1[0-9]/.test(s)) return true;
  if (/apple\s+gpu|apple\s+m[1-9]|apple\s+a1[4-9]|apple\s+a[2-9][0-9]/.test(s)) return true;
  if (/nvidia|geforce|rtx |radeon rx|arc\(tm\)|intel\(r\) arc/.test(s)) return true;
  return false;
}

export function isDiscreteName(name) {
  return /nvidia|geforce|rtx |radeon rx|arc\(tm\)|intel\(r\) arc/.test(String(name || "").toLowerCase());
}

export function glRendererString() {
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl2", { failIfMajorPerformanceCaveat: false })
      || c.getContext("webgl");
    if (!gl) return "";
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    if (ext) return String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) || "");
    return String(gl.getParameter(gl.RENDERER) || "");
  } catch {
    return "";
  }
}

export async function probeGpu() {
  const glName = glRendererString();
  let adapter = null;
  let infoName = "";
  if (navigator.gpu) {
    try {
      adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
      const info = adapter && (adapter.info || {});
      infoName = [info.vendor, info.architecture, info.device, info.description]
        .filter(Boolean).join(" ");
    } catch {
      adapter = null;
    }
  }
  const name = infoName || glName || "";
  const strong = isStrongGpu(name) || isStrongGpu(infoName) || isStrongGpu(glName);
  const discrete = isDiscreteName(name) || isDiscreteName(infoName) || isDiscreteName(glName);
  return { adapter, glName, infoName, name, strong, discrete };
}

function packedSprites(data, count) {
  // WebGPU writeBuffer rejects growable WASM memory; a standalone copy is safe
  // for both backends.
  return Float32Array.from(data.subarray(0, count * 8));
}

export async function createGpuRenderer(canvas, prefer) {
  if (prefer === "cpu") return null;
  if (prefer !== "webgl2") {
    try {
      const gpu = await createWebGpu(canvas);
      if (gpu) return gpu;
    } catch (err) {
      console.warn("webgpu init failed", err);
    }
    if (canvas.getContext("webgpu")) {
      const next = canvas.cloneNode(false);
      next.id = canvas.id;
      canvas.replaceWith(next);
      canvas = next;
    }
  }
  if (prefer === "webgpu") return null;
  try {
    return createWebGl2(canvas);
  } catch (err) {
    console.warn("webgl2 init failed", err);
    return null;
  }
}

const WGSL_SPRITES = `
struct Uniforms { res: vec2<f32>, _pad: vec2<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;
struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) local: vec2<f32>,
  @location(1) color: vec4<f32>,
};
@vertex
fn vs(
  @location(0) corner: vec2<f32>,
  @location(1) inst: vec4<f32>,
  @location(2) rgb: vec4<f32>,
) -> VSOut {
  var out: VSOut;
  let off = vec2(corner.x * inst.z / u.res.x * 2.0, corner.y * inst.z / u.res.y * 2.0);
  out.pos = vec4(inst.xy + off, 0.0, 1.0);
  out.local = corner;
  out.color = vec4(rgb.xyz, inst.w);
  return out;
}
@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
  let r2 = dot(in.local, in.local);
  if (r2 > 1.0) { discard; }
  let falloff = exp(-r2 * 3.2) * in.color.a;
  return vec4(in.color.rgb * falloff, falloff);
}
`;

const WGSL_BLIT = `
@group(0) @binding(0) var hdrTex: texture_2d<f32>;
@group(0) @binding(1) var hdrSamp: sampler;
struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VSOut {
  var out: VSOut;
  let x = f32(i32(i & 1u) * 4 - 1);
  let y = f32(i32(i >> 1u) * 4 - 1);
  out.pos = vec4(x, y, 0.0, 1.0);
  out.uv = vec2((x + 1.0) * 0.5, (1.0 - y) * 0.5);
  return out;
}
fn srgb(v: f32) -> f32 {
  return select(1.055 * pow(max(v, 0.0), 1.0 / 2.4) - 0.055, v * 12.92, v <= 0.0031308);
}
fn tonemap(hdr: vec3<f32>) -> vec3<f32> {
  let exposed = hdr * 1.25;
  let lum = max(dot(exposed, vec3(0.2126, 0.7152, 0.0722)), 1.0e-4);
  return exposed / (1.0 + lum);
}
@fragment
fn fs(in: VSOut) -> @location(0) vec4<f32> {
  let hdr = textureSampleLevel(hdrTex, hdrSamp, in.uv, 0.0).rgb;
  let mapped = tonemap(hdr);
  return vec4(srgb(mapped.r), srgb(mapped.g), srgb(mapped.b), 1.0);
}
`;

async function createWebGpu(canvas) {
  if (!navigator.gpu) return null;
  let adapter;
  try {
    adapter = await navigator.gpu.requestAdapter({ powerPreference: "high-performance" });
  } catch {
    return null;
  }
  if (!adapter) return null;
  const info = adapter.info || {};
  const desc = [info.vendor, info.architecture, info.device, info.description].filter(Boolean).join(" ");
  if (/swiftshader|llvmpipe|softpipe/.test(desc.toLowerCase())) {
    return null;
  }
  let device;
  try {
    device = await adapter.requestDevice();
  } catch {
    return null;
  }
  const ctx = canvas.getContext("webgpu");
  if (!ctx) return null;
  const format = navigator.gpu.getPreferredCanvasFormat();
  try {
    ctx.configure({ device, format, alphaMode: "opaque" });
  } catch {
    return null;
  }
  const spriteMod = device.createShaderModule({ code: WGSL_SPRITES });
  const blitMod = device.createShaderModule({ code: WGSL_BLIT });
  const bindLayout = device.createBindGroupLayout({
    entries: [{ binding: 0, visibility: GPUShaderStage.VERTEX, buffer: { type: "uniform" } }],
  });
  const pipeline = device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [bindLayout] }),
    vertex: {
      module: spriteMod,
      entryPoint: "vs",
      buffers: [
        { arrayStride: 8, attributes: [{ shaderLocation: 0, offset: 0, format: "float32x2" }] },
        {
          arrayStride: 32,
          stepMode: "instance",
          attributes: [
            { shaderLocation: 1, offset: 0, format: "float32x4" },
            { shaderLocation: 2, offset: 16, format: "float32x4" },
          ],
        },
      ],
    },
    fragment: {
      module: spriteMod,
      entryPoint: "fs",
      targets: [{
        format: "rgba8unorm",
        blend: {
          color: { srcFactor: "one", dstFactor: "one", operation: "add" },
          alpha: { srcFactor: "one", dstFactor: "one", operation: "add" },
        },
      }],
    },
    primitive: { topology: "triangle-list" },
  });
  const blitLayout = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: "unfilterable-float" } },
      { binding: 1, visibility: GPUShaderStage.FRAGMENT, sampler: { type: "non-filtering" } },
    ],
  });
  const blitPipeline = device.createRenderPipeline({
    layout: device.createPipelineLayout({ bindGroupLayouts: [blitLayout] }),
    vertex: { module: blitMod, entryPoint: "vs" },
    fragment: {
      module: blitMod,
      entryPoint: "fs",
      targets: [{ format }],
    },
    primitive: { topology: "triangle-list" },
  });
  const sampler = device.createSampler({ magFilter: "nearest", minFilter: "nearest" });
  const quadBuf = device.createBuffer({
    size: QUAD.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(quadBuf, 0, QUAD);
  const uniformBuf = device.createBuffer({
    size: 16,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });
  const bind = device.createBindGroup({
    layout: bindLayout,
    entries: [{ binding: 0, resource: { buffer: uniformBuf } }],
  });
  let instanceBuf = device.createBuffer({
    size: 32 * 4096,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
  });
  let instanceCap = 4096;
  let hdr = null;
  let blitBind = null;
  function ensureHdr(w, h) {
    if (hdr && hdr.width === w && hdr.height === h) return;
    if (hdr) hdr.destroy();
    hdr = device.createTexture({
      size: [w, h],
      format: "rgba8unorm",
      usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING,
    });
    blitBind = device.createBindGroup({
      layout: blitLayout,
      entries: [
        { binding: 0, resource: hdr.createView() },
        { binding: 1, resource: sampler },
      ],
    });
  }
  return {
    api: "webgpu",
    canvas,
    resize() {},
    draw(data, count, width, height, clear) {
      if (count < 1) return;
      ensureHdr(width, height);
      if (count > instanceCap) {
        instanceCap = count;
        instanceBuf.destroy();
        instanceBuf = device.createBuffer({
          size: 32 * instanceCap,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
      }
      device.queue.writeBuffer(instanceBuf, 0, packedSprites(data, count));
      device.queue.writeBuffer(uniformBuf, 0, new Float32Array([width, height, 0, 0]));
      const encoder = device.createCommandEncoder();
      const accum = encoder.beginRenderPass({
        colorAttachments: [{
          view: hdr.createView(),
          clearValue: { r: clear[0], g: clear[1], b: clear[2], a: 1 },
          loadOp: "clear",
          storeOp: "store",
        }],
      });
      accum.setPipeline(pipeline);
      accum.setBindGroup(0, bind);
      accum.setVertexBuffer(0, quadBuf);
      accum.setVertexBuffer(1, instanceBuf);
      accum.draw(6, count);
      accum.end();
      const view = ctx.getCurrentTexture().createView();
      const blit = encoder.beginRenderPass({
        colorAttachments: [{
          view,
          clearValue: { r: 0, g: 0, b: 0, a: 1 },
          loadOp: "clear",
          storeOp: "store",
        }],
      });
      blit.setPipeline(blitPipeline);
      blit.setBindGroup(0, blitBind);
      blit.draw(3);
      blit.end();
      device.queue.submit([encoder.finish()]);
    },
  };
}

function compile(gl, type, src) {
  const sh = gl.createShader(type);
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(sh) || "shader");
  }
  return sh;
}

function link(gl, vsSrc, fsSrc) {
  const prog = gl.createProgram();
  gl.attachShader(prog, compile(gl, gl.VERTEX_SHADER, vsSrc));
  gl.attachShader(prog, compile(gl, gl.FRAGMENT_SHADER, fsSrc));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(prog) || "link");
  }
  return prog;
}

function createWebGl2(canvas) {
  const gl = canvas.getContext("webgl2", {
    alpha: false,
    antialias: false,
    depth: false,
    stencil: false,
    premultipliedAlpha: false,
    preserveDrawingBuffer: true,
    powerPreference: "high-performance",
  });
  if (!gl) return null;
  const vs = `#version 300 es
layout(location=0) in vec2 corner;
layout(location=1) in vec4 inst;
layout(location=2) in vec4 rgb;
uniform vec2 uRes;
out vec2 vLocal;
out vec4 vColor;
void main() {
  vec2 off = vec2(corner.x * inst.z / uRes.x * 2.0, corner.y * inst.z / uRes.y * 2.0);
  gl_Position = vec4(inst.xy + off, 0.0, 1.0);
  vLocal = corner;
  vColor = vec4(rgb.xyz, inst.w);
}`;
  const fs = `#version 300 es
precision mediump float;
in vec2 vLocal;
in vec4 vColor;
out vec4 frag;
void main() {
  float r2 = dot(vLocal, vLocal);
  if (r2 > 1.0) discard;
  float falloff = exp(-r2 * 3.2) * vColor.a;
  frag = vec4(vColor.rgb * falloff, falloff);
}`;
  const blitVs = `#version 300 es
layout(location=0) in vec2 pos;
out vec2 vUv;
void main() {
  vUv = pos * 0.5 + 0.5;
  gl_Position = vec4(pos, 0.0, 1.0);
}`;
  const blitFs = `#version 300 es
precision mediump float;
in vec2 vUv;
uniform sampler2D uHdr;
out vec4 frag;
float srgb(float v) {
  return v <= 0.0031308 ? v * 12.92 : 1.055 * pow(max(v, 0.0), 1.0 / 2.4) - 0.055;
}
vec3 tonemap(vec3 hdr) {
  vec3 exposed = hdr * 1.25;
  float lum = max(dot(exposed, vec3(0.2126, 0.7152, 0.0722)), 1.0e-4);
  return exposed / (1.0 + lum);
}
void main() {
  vec3 hdr = texture(uHdr, vUv).rgb;
  vec3 mapped = tonemap(hdr);
  frag = vec4(srgb(mapped.r), srgb(mapped.g), srgb(mapped.b), 1.0);
}`;
  const prog = link(gl, vs, fs);
  const blit = link(gl, blitVs, blitFs);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const quad = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, quad);
  gl.bufferData(gl.ARRAY_BUFFER, QUAD, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const inst = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, inst);
  gl.bufferData(gl.ARRAY_BUFFER, 32 * 4096, gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(1, 4, gl.FLOAT, false, 32, 0);
  gl.vertexAttribDivisor(1, 1);
  gl.enableVertexAttribArray(2);
  gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 32, 16);
  gl.vertexAttribDivisor(2, 1);
  const blitVao = gl.createVertexArray();
  gl.bindVertexArray(blitVao);
  const blitQuad = gl.createBuffer();
  const fullscreen = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);
  gl.bindBuffer(gl.ARRAY_BUFFER, blitQuad);
  gl.bufferData(gl.ARRAY_BUFFER, fullscreen, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
  const uRes = gl.getUniformLocation(prog, "uRes");
  const uHdr = gl.getUniformLocation(blit, "uHdr");
  gl.disable(gl.DEPTH_TEST);
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
  let cap = 4096;
  let target = null;
  const canF16 = !!(gl.getExtension("EXT_color_buffer_float") || gl.getExtension("EXT_color_buffer_half_float"));
  function makeTarget(w, h) {
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    const internal = canF16 ? gl.RGBA16F : gl.RGBA8;
    const type = canF16 ? gl.HALF_FLOAT : gl.UNSIGNED_BYTE;
    gl.texImage2D(gl.TEXTURE_2D, 0, internal, w, h, 0, gl.RGBA, type, null);
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) {
        throw new Error("framebuffer");
      }
    }
    return { tex, fb, w, h };
  }
  function ensureTarget(w, h) {
    if (target && target.w === w && target.h === h) return;
    if (target) {
      gl.deleteTexture(target.tex);
      gl.deleteFramebuffer(target.fb);
    }
    target = makeTarget(w, h);
  }
  return {
    api: "webgl2",
    canvas,
    resize() {},
    draw(data, count, width, height, clear) {
      if (count < 1) return;
      ensureTarget(width, height);
      gl.bindFramebuffer(gl.FRAMEBUFFER, target.fb);
      gl.viewport(0, 0, width, height);
      gl.clearColor(clear[0], clear[1], clear[2], 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE);
      gl.useProgram(prog);
      gl.uniform2f(uRes, width, height);
      gl.bindVertexArray(vao);
      gl.bindBuffer(gl.ARRAY_BUFFER, inst);
      if (count > cap) {
        cap = count;
        gl.bufferData(gl.ARRAY_BUFFER, 32 * cap, gl.DYNAMIC_DRAW);
      }
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, packedSprites(data, count));
      gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, count);
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, width, height);
      gl.disable(gl.BLEND);
      gl.useProgram(blit);
      gl.uniform1i(uHdr, 0);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, target.tex);
      gl.bindVertexArray(blitVao);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
    },
  };
}
