"""Encode the showcase recording into the README's media (run by scripts/showcase.sh, in gm-media).

    python3 deploy/perf/showcase_encode.py        # target/showcase -> docs/media

Reads what deploy/perf/showcase.py wrote: target/showcase/timeline.json (each frame's seconds on
screen, each caption's start) and the PNG stills. Writes under docs/media/:

    showcase.mp4          1280x720, 30 fps, H.264 two-pass to a size budget, captions burnt in
    showcase-preview.webp an animated 640 px loop of every caption's first seconds, for the README
    <still>.webp          each still, 1600 px wide

Exit 0 written and within budget, 1 the video is over BUDGET_BYTES, 2 could not run.

Why a size budget: GitHub plays a video inline only from an upload of at most 10 MB, and a
repository file is shown by link, so one file serves both and must stay under the smaller limit.

Caveat: the bit rate is the budget spread over the whole video, so a long static stretch and a
1M-node zoom get the same average; two-pass x264 moves bits between them but cannot add any, and
a recording much longer than two minutes comes out visibly soft. Shorten the scenes, not the budget.
"""
import json
import os
import subprocess
import sys
import tempfile

SOURCE = "target/showcase"
OUT = "docs/media"
BUDGET_BYTES = 9_500_000
WIDTH, HEIGHT, FPS = 1280, 720, 30
CAPTION_SECONDS, FADE = 4.2, 0.35
PREVIEW = {"width": 640, "fps": 12, "offset": 0.6, "seconds": 1.6}
SKIPPED_STILLS = ("probe-", "discover")


def ffmpeg(*args):
    command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args]
    subprocess.run(command, check=True)


def font(style):
    return subprocess.run(["fc-match", "-f", "%{file}", f"Inter:{style}"], check=True,
                          capture_output=True, text=True).stdout


def concat_list(frames, work):
    """The concat demuxer's list: every frame for its own duration, the last one repeated."""
    kept = [(path, seconds) for path, seconds in frames if seconds > 0]
    lines = ["ffconcat version 1.0"]
    for path, seconds in kept:
        lines += [f"file '/w/{path}'", f"duration {seconds:.6f}"]
    lines.append(f"file '/w/{kept[-1][0]}'")
    listing = os.path.join(work, "frames.ffconcat")
    with open(listing, "w") as file:
        file.write("\n".join(lines) + "\n")
    return listing, sum(seconds for _, seconds in kept)


def windows(marks, total):
    """(title, sub, start, end) per caption: up to CAPTION_SECONDS, never into the next one."""
    starts = [seconds for _, seconds in marks] + [total]
    shown = []
    for (label, start), following in zip(marks, starts[1:]):
        text = json.loads(label)
        end = min(start + CAPTION_SECONDS, following - 0.15, total)
        if end - start > 2 * FADE:
            shown.append((text["title"], text.get("sub", ""), start, end))
    return shown


def drawtext(work, name, text, face, size, y, window):
    """One drawtext that fades in and out over `window`; the text goes through a file, unescaped."""
    path = os.path.join(work, f"{name}.txt")
    with open(path, "w") as file:
        file.write(text)
    start, end = window
    alpha = (f"if(lt(t,{start + FADE}),(t-{start})/{FADE},"
             f"if(gt(t,{end - FADE}),({end}-t)/{FADE},1))")
    return (f"drawtext=fontfile='{face}':textfile='{path}':fontsize={size}:fontcolor=white:"
            f"x=56:y={y}:shadowcolor=black@0.55:shadowx=0:shadowy=2:"
            f"alpha='{alpha}':enable='between(t,{start},{end})'")


def caption_filters(work, captions):
    bold, regular = font("semibold"), font("regular")
    filters = []
    for index, (title, sub, start, end) in enumerate(captions):
        filters.append(drawtext(work, f"t{index}", title, bold, 34, HEIGHT - 128, (start, end)))
        if sub:
            filters.append(drawtext(work, f"s{index}", sub, regular, 20, HEIGHT - 82, (start, end)))
    return filters


def encode_video(listing, filters, seconds, work):
    """Two-pass H.264 at the bit rate that lands the file under BUDGET_BYTES."""
    graph = ",".join([f"fps={FPS}", f"scale={WIDTH}:{HEIGHT}:flags=lanczos", *filters, "format=yuv420p"])
    script = os.path.join(work, "filters.txt")
    with open(script, "w") as file:
        file.write(graph)
    rate = min(6_000_000, int(BUDGET_BYTES * 8 * 0.96 / seconds))
    video = os.path.join(OUT, "showcase.mp4")
    common = ["-f", "concat", "-safe", "0", "-i", listing, "-filter_script:v", script, "-an",
              "-c:v", "libx264", "-preset", "slow", "-profile:v", "high", "-b:v", str(rate),
              "-passlogfile", os.path.join(work, "x264")]
    ffmpeg(*common, "-pass", "1", "-f", "null", "/dev/null")
    ffmpeg(*common, "-pass", "2", "-movflags", "+faststart", video)
    return video, rate


def encode_preview(video, captions):
    """An animated WebP of every caption's opening seconds, cut from the finished video."""
    picks = "+".join(f"between(t,{start + PREVIEW['offset']:.3f},"
                     f"{start + PREVIEW['offset'] + PREVIEW['seconds']:.3f})"
                     for _, _, start, _ in captions)
    graph = (f"fps={PREVIEW['fps']},select='{picks}',setpts=N/{PREVIEW['fps']}/TB,"
             f"scale={PREVIEW['width']}:-2:flags=lanczos")
    preview = os.path.join(OUT, "showcase-preview.webp")
    ffmpeg("-i", video, "-vf", graph, "-an", "-c:v", "libwebp", "-quality", "70",
           "-compression_level", "6", "-loop", "0", preview)
    return preview


def encode_stills():
    folder = os.path.join(SOURCE, "stills")
    written = []
    for name in sorted(os.listdir(folder)):
        if not name.endswith(".png") or name.startswith(SKIPPED_STILLS):
            continue
        still = os.path.join(OUT, name[:-4] + ".webp")
        ffmpeg("-i", os.path.join(folder, name), "-vf", "scale=1600:-2:flags=lanczos",
               "-c:v", "libwebp", "-quality", "88", still)
        written.append(still)
    return written


def main():
    try:
        with open(os.path.join(SOURCE, "timeline.json")) as file:
            timeline = json.load(file)
    except OSError as failure:
        print(f"showcase-encode: no recording ({failure}); run scripts/showcase.sh record", file=sys.stderr)
        return 2
    os.makedirs(OUT, exist_ok=True)
    with tempfile.TemporaryDirectory() as work:
        listing, seconds = concat_list(timeline["frames"], work)
        captions = windows(timeline["marks"], seconds)
        video, rate = encode_video(listing, caption_filters(work, captions), seconds, work)
        preview = encode_preview(video, captions)
    stills = encode_stills()
    size = os.path.getsize(video)
    print(f"video {video} {seconds:.1f}s {rate // 1000} kbit/s {size} bytes (budget {BUDGET_BYTES})")
    print(f"preview {preview} {os.path.getsize(preview)} bytes; stills {len(stills)}")
    return 0 if size <= BUDGET_BYTES else 1


if __name__ == "__main__":
    sys.exit(main())
