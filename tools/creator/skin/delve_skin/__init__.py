"""Delvewright NPC skin toolchain (spec-0009).

Given a cast-sheet entry (character brief + palette + wide/slim model) this
package composes an *original* 64x64 Minecraft player skin deterministically and
renders headless multi-angle previews for human review. Skins are original
artwork composed programmatically (ADR-0013) -- never downloaded.

Composition uses ``skinpy-extended`` (MIT) part/face pixel addressing; previews
use its deterministic isometric player-model renderer. See README.md.
"""

from delve_skin.compose import compose_skin, CastEntry
from delve_skin.preview import render_previews, PREVIEW_ANGLES

__all__ = ["compose_skin", "CastEntry", "render_previews", "PREVIEW_ANGLES"]

TOOL_NAME = "delve-skin"
TOOL_VERSION = "0.1.0"
