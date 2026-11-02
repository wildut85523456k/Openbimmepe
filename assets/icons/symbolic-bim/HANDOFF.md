# Handoff: symbolic icon collection and PNG integration

PR: see the pull request for branch assets/symbolic-bim-icons in this repository
Repository: wildut85523456k/Openbimmepe (public)
Branch: assets/symbolic-bim-icons
Directory: assets/icons/symbolic-bim/

The earlier PR #115 was created in the separate revit-alike repository by mistake. This package is now delivered to Openbimmepe, the active repository. PR numbers are repository-specific.

## Fetch the work
```sh
git remote -v
git fetch origin assets/symbolic-bim-icons
git switch --track origin/assets/symbolic-bim-icons
```
If the local branch exists, switch to it instead. A maintainer may merge the PR or apply the branch to an integration branch.

## Produce the PNG assets the current Rust loader expects
```sh
python -m pip install cairosvg
python assets/icons/symbolic-bim/export-png.py --check-only
python assets/icons/symbolic-bim/export-png.py --size 128
```
CairoSVG requires a Cairo runtime; follow its installation instructions for the host platform. Output is 96 transparent PNGs in the package's png/ directory. Names use underscores, with name.png for light and name.dark.png for dark, matching the existing loader's filename convention. Smaller physical pixel sizes can be exported into separate folders with --size and --output. There is no need to redraw the SVG artwork to obtain PNGs.

## What remains
The current loader in apps/desktop/src/icons.rs uses include_bytes! for assets/icons/rust/*.png. This asset PR does not change it. Review the generated PNGs, map asset IDs to actual commands, then copy approved outputs into the loader's asset directory and register both themes. Some IDs need deliberate mapping: luminaire -> lighting_fixture, automation -> visual_programming; pad_footing/strip_footing may need distinct command definitions. Do not assume all 48 asset IDs are existing commands.

The reported 17 component fallbacks have not been identified in this handoff. List the exact 17 command IDs from the target branch before claiming coverage. A generic component fallback prevents a missing texture but does not provide a distinct symbol for each command. Add specific geometry for any missing commands before replacing their fallback.

## Validation status
The SVG exporter ran successfully and all 96 exported SVG files were XML-parsed. The PNG source checker verifies matching theme pairs without dependencies. PNG conversion requires CairoSVG/Cairo and must be run and visually reviewed on a configured host; no claim of completed PNG rendering or app integration is made here.

