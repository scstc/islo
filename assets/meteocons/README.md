# Meteocons reminder artwork

Source: https://github.com/basmilius/meteocons — Bas Milius, MIT (see `LICENSE`).
Original SVG files are from `@meteocons/svg` version `0.1.0`, fill style.

The PNG sheets bake the original SVG/SMIL rotation, translation, opacity and spline timing into a six-second loop: 72 frames at 12 fps, 224 × 224 pixels per frame, eight columns and nine rows (1792 × 2016, within a 2048 texture limit). The animation is sampled after a six-second warm-up to make delayed animations loop continuously. Original gradients and transparency are retained.

islo selects one sheet for each fullscreen reminder and animates its texture coordinates. No browser, network request, Lottie runtime or model call is needed at runtime.
