# Marduk artwork

`marduk.svg` is the authoritative source for the application icon and README artwork. It contains two filled vector paths with transparent exterior corners, with no embedded raster image, fonts, or external resources.

Edit the SVG, then regenerate all exports from the project root:

```sh
pnpm icons
```

Tauri generates the desktop files in `src-tauri/icons` and the large PNG at `assets/marduk/2508x2508.png`. The README uses the SVG directly. Tauri also emits `android` and `ios` folders; these are not tracked while the project targets desktop platforms.

The approved concept was generated with the built-in image generation tool, then traced into smooth vector paths and reduced to two solid colors. Its generation prompt is preserved in `marduk.prompt.txt`; future changes should start with the SVG.
