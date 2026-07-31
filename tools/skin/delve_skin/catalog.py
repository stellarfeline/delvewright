"""Catalog card + provenance emission (spec-0009 step 4 / ADR-0013).

A skin catalog card records the character brief, tags (role/style/palette/model),
quality, preview paths and -- always -- ``license: original``. Provenance records
how the artwork was produced so the "original artwork, never downloaded" claim is
auditable.
"""

from __future__ import annotations

import hashlib
import json
from typing import Dict, List

from delve_skin import TOOL_NAME, TOOL_VERSION
from delve_skin.compose import CastEntry


def _palette_tags(entry: CastEntry) -> List[str]:
    return sorted(entry.palette.keys())


def provenance(entry: CastEntry, png_bytes: bytes) -> Dict[str, object]:
    """Auditable record: original programmatic composition, not a download."""
    return {
        "license": "original",
        "origin": "programmatic-composition",
        "tool": {"name": TOOL_NAME, "version": TOOL_VERSION},
        "library": {"name": "skinpy-extended", "version": "1.0.1", "license": "MIT"},
        "deterministic": True,
        "seed": entry.resolved_seed(),
        "model": entry.model,
        "png_sha256": hashlib.sha256(png_bytes).hexdigest(),
        "note": (
            "Original artwork composed pixel-by-pixel from the cast-sheet brief "
            "(ADR-0013). No third-party skin asset was downloaded or copied."
        ),
    }


def catalog_card(
    entry: CastEntry,
    png_bytes: bytes,
    preview_names: List[str],
) -> Dict[str, object]:
    """A spec-0009 skin catalog card (`catalog/skin-<id>.json`)."""
    return {
        "id": f"skin-{entry.texture_id}",
        "texture_id": entry.texture_id,
        "kind": "npc-skin",
        "license": "original",
        "description": entry.style_brief,
        "tags": {
            "role": entry.role,
            "style": entry.features.get("style", ""),
            "palette": _palette_tags(entry),
            "model": entry.model,
        },
        "model": entry.model,
        "hidden_layers": entry.hidden_layers,
        "quality": entry.features.get("quality", 4),
        "texture_path": f"skins/{entry.texture_id}.png",
        "resource_pack_path": f"assets/delvewright/textures/npc/{entry.texture_id}.png",
        "previews": [f"previews/{n}" for n in preview_names],
        "provenance": provenance(entry, png_bytes),
    }


def dumps(card: Dict[str, object]) -> str:
    """Stable JSON serialisation (sorted keys, trailing newline)."""
    return json.dumps(card, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
