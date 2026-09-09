# NPC skins — the face toolchain


Skip unless the design calls for a custom skin. The skin toolchain is a Python
package with dependencies, and `python3 --version` answering is not the same as
the package being importable — a missing skin is a build error, not a silent
skip. Make it a venv here so step 5 does not stop on it:

```sh
python3 -m venv .venv-skin
.venv-skin/bin/pip install -r "$DELVEWRIGHT_ENGINE/tools/skin/requirements.txt"
PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin --help
```

The last line answering is the confirmation. Only the dependencies are
installed; the package itself is reached on `PYTHONPATH`, which leaves no build
artifacts in either repository. Use
`PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin …`
wherever this page says `python -m delve_skin`.
