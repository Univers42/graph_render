/** The part of a 2D context the painter uses, so a test can hand it a recorder. */

/**
 * What a baked sprite is. The renderer hands the image back to `drawImage` and never reads
 * it, so the type is left to the host: a canvas there, a string in a recorder.
 */
export type SpriteImage = object;

export interface Surface2D extends Pick<
  CanvasRenderingContext2D,
  | "setTransform" | "fillRect" | "beginPath" | "moveTo" | "lineTo" | "quadraticCurveTo"
  | "bezierCurveTo" | "arc" | "rect" | "fill" | "stroke" | "createLinearGradient"
  | "fillStyle" | "strokeStyle" | "lineWidth" | "globalAlpha"
> {
  /** Forwarded from a sprite cache, so the argument stays as wide as the sprite is. */
  drawImage(image: SpriteImage, x: number, y: number, width: number, height: number): void;
}

/** The part of a 2D context a label is baked with. */
export interface TextSurface2D {
  font: string;
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  lineJoin: CanvasLineJoin;
  textBaseline: CanvasTextBaseline;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  measureText(text: string): { readonly width: number };
  fillRect(x: number, y: number, width: number, height: number): void;
  strokeText(text: string, x: number, y: number): void;
  fillText(text: string, x: number, y: number): void;
}

/**
 * How a host takes raw RGBA bytes on a sprite surface. The renderer never constructs an
 * ImageData itself: the ImageData constructor is a host's, and a test's surface takes the
 * bytes as they are.
 */
export interface PixelSurface2D {
  putPixels(data: Uint8ClampedArray<ArrayBuffer>, width: number, height: number): void;
}

/**
 * Somewhere to bake a label once. `Image` is what the painter blits; a test bakes onto
 * something that is not a canvas. `pixels` is absent on a surface that only takes text,
 * and the impostor cache bakes nothing on one.
 */
export interface SpriteSurface<Image = SpriteImage> {
  readonly image: Image;
  readonly ctx: TextSurface2D;
  readonly pixels?: PixelSurface2D;
  resize(width: number, height: number): void;
}

/** `null` means the host has no canvas to bake on. */
export type SpriteFactory<Image = SpriteImage> = () => SpriteSurface<Image> | null;
