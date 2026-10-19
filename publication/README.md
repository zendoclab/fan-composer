# Preprint

**Fan Composer: NPU-Assisted Fan-Aware Soundscapes with Always-On Automatic Volume**

Kyu Lee · cording.ai · October 20, 2026

Published on Zenodo: https://doi.org/10.5281/zenodo.23478732
Record: https://zenodo.org/records/23478732

This engineering feasibility preprint describes software 0.3.0. It is not peer reviewed and does not establish comfort or energy benefits.

- `fan-composer-preprint.pdf`: visually reviewed six-page English preprint.
- `fan-composer.typ`: editable Typst source.
- `evidence.json`: sanitized engineering measurements used in the tables.
- `zenodo-metadata.json`: public deposition metadata; contains no API token.
- `LICENSE`: CC BY 4.0 for this preprint and its evidence. Application/model licensing is MIT separately.

To reproduce the PDF on Windows with Typst and Cambria/Segoe UI/Consolas fonts:

```text
typst compile --root . publication/fan-composer.typ publication/fan-composer-preprint.pdf
```

The source loads the actual screenshot from docs/images/ and the local evidence JSON. Public test output and validation are in reports/public/. All six pages were rendered to PNG and reviewed before publication.
