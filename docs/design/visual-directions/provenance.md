# Initial visual direction comparison

Open `index.html` locally. Full scenes, character details and image enlargement
work without a build or external resources. PNGs are software captures of ANSI
renderer output at 160 × 48, drawn with DejaVu Sans Mono; real terminal fonts and
cell spacing can change their appearance.

- **Classic:** the working renderer before the 11 October movement/art pass. This
  is the baseline, not a completed proposal for additional classic-style polish.
- **Atmospheric:** an isolated renderer experiment with larger line-art figures
  and a cool background palette. It is a concept; its palette does not represent
  a finished lighting system or an installed game setting.
- **Pixel:** the earlier playable close-view prototype. It was an initial art pass,
  not final artwork for every mob or terrain material.

All use the deterministic geometry and entities in
`examples/visual_showcase.rs`. The closer views deliberately show less terrain
to give figures more detail. The user subsequently selected terminal pixel art.
See [the current cross-world refinement](../pixel-refinement/provenance.md); the
images on this initial comparison page have not been replaced.
