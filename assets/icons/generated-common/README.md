# Common and supplemental 3D command icons

These PNGs complement the user-supplied catalog. They are separate generated
artwork, not crops from the approved sheet. Built-in image generation was used
with one icon per request, silver/grey materials and restrained blue accents.
The prompts and SHA-256 hashes are recorded in `prompts.json`.

Only small runtime versions are committed: fine rendering detail is reduced when
prepared at 128 px and shown at 34 px in the ribbon. Keep silhouettes clear at
this size; do not add engineering detail to common-command symbols.

Reproduce the resource files by copying these versioned PNGs into `approved-3d`:

```sh
python3 assets/icons/prepare_common.py
```

Generation is not needed to build the app. The committed files, rather than a
future stochastic regeneration from prompts, define the shipped appearance.
