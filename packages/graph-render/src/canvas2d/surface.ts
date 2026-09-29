/** The part of a 2D context the painter uses, so a test can hand it a recorder. */

export type Surface2D = Pick<
  CanvasRenderingContext2D,
  | "setTransform" | "fillRect" | "beginPath" | "moveTo" | "lineTo" | "quadraticCurveTo"
  | "bezierCurveTo" | "arc" | "rect" | "fill" | "stroke" | "drawImage"
  | "fillStyle" | "strokeStyle" | "lineWidth" | "globalAlpha"
>;

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
  strokeText(text: string, x: number, y: number): void;
  fillText(text: string, x: number, y: number): void;
}

/**
 * Somewhere to bake a label once. `Image` is what the painter blits; a test bakes onto
 * something that is not a canvas.
 */
export interface SpriteSurface<Image = CanvasImageSource> {
  readonly image: Image;
  readonly ctx: TextSurface2D;
  resize(width: number, height: number): void;
}

/** `null` means the host has no canvas to bake on. */
export type SpriteFactory<Image = CanvasImageSource> = () => SpriteSurface<Image> | null;
