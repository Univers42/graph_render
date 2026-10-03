/** The few WebGL2 calls the layer repeats: linking a program and wiring an attribute. */

function shaderOf(gl: WebGL2RenderingContext, kind: GLenum, source: string): WebGLShader {
  const shader = gl.createShader(kind);
  if (shader === null) throw new Error("webgl2: createShader returned null (context lost?)");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) === true) return shader;
  const log = gl.getShaderInfoLog(shader) ?? "";
  gl.deleteShader(shader);
  throw new Error(`webgl2: ${kind === gl.VERTEX_SHADER ? "vertex" : "fragment"} shader did not compile: ${log}`);
}

/** A linked program, or an Error carrying the driver's log. */
export function programOf(gl: WebGL2RenderingContext, vertex: string, fragment: string): WebGLProgram {
  const program = gl.createProgram();
  gl.attachShader(program, shaderOf(gl, gl.VERTEX_SHADER, vertex));
  gl.attachShader(program, shaderOf(gl, gl.FRAGMENT_SHADER, fragment));
  gl.linkProgram(program);
  if (gl.getProgramParameter(program, gl.LINK_STATUS) === true) return program;
  const log = gl.getProgramInfoLog(program) ?? "";
  gl.deleteProgram(program);
  throw new Error(`webgl2: program did not link: ${log}`);
}

/** Gives the context's GPU memory back now; the browser would otherwise wait for a collection. */
export function loseContext(gl: WebGL2RenderingContext): void {
  gl.getExtension("WEBGL_lose_context")?.loseContext();
}

/** Uniform locations by name, looked up once per program. */
export type Uniforms = (name: string) => WebGLUniformLocation | null;

export function uniformsOf(gl: WebGL2RenderingContext, program: WebGLProgram): Uniforms {
  const seen = new Map<string, WebGLUniformLocation | null>();
  return (name) => {
    if (!seen.has(name)) seen.set(name, gl.getUniformLocation(program, name));
    return seen.get(name) ?? null;
  };
}

/** One vertex attribute: a float column, or a `uint` one read from 16-bit palette slots. */
export interface Attribute {
  readonly name: string;
  readonly buffer: WebGLBuffer;
  readonly size: number;
  readonly slots?: boolean;
}

/** Wires one attribute into the bound vertex array; one the compiler dropped is skipped. */
export function attribute(gl: WebGL2RenderingContext, program: WebGLProgram, spec: Attribute, divisor: number): void {
  const at = gl.getAttribLocation(program, spec.name);
  if (at < 0) return;
  gl.bindBuffer(gl.ARRAY_BUFFER, spec.buffer);
  gl.enableVertexAttribArray(at);
  if (spec.slots === true) gl.vertexAttribIPointer(at, spec.size, gl.UNSIGNED_SHORT, 0, 0);
  else gl.vertexAttribPointer(at, spec.size, gl.FLOAT, false, 0, 0);
  gl.vertexAttribDivisor(at, divisor);
}
