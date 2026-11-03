"""`tools/lib/local_config.py`: the one answer to where a checkout's gitignored
`delvewright.local.toml` is, and every tool that reads the file held to it."""

import importlib.util
import subprocess
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS / "lib"))
import local_config  # noqa: E402

sys.path.pop(0)


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def _git(*args, cwd):
    subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True)


def _repo_with_worktree(tmp_path):
    main = tmp_path / "main"
    main.mkdir()
    _git("init", "-q", "-b", "main", cwd=main)
    (main / "README").write_text("x\n")
    _git("add", "README", cwd=main)
    _git("commit", "-q", "-m", "init", cwd=main)
    wt = tmp_path / "wt"
    _git("worktree", "add", "-q", "--detach", str(wt), cwd=main)
    return main, wt


def test_a_worktree_without_its_own_reads_the_main_checkouts(tmp_path):
    main, wt = _repo_with_worktree(tmp_path)
    assert local_config.local_config_path(wt) == wt / local_config.LOCAL_CONFIG_FILE
    (main / local_config.LOCAL_CONFIG_FILE).write_text("[i18n]\n")
    assert local_config.local_config_path(wt) == main / local_config.LOCAL_CONFIG_FILE
    (wt / local_config.LOCAL_CONFIG_FILE).write_text("[i18n]\n")
    assert local_config.local_config_path(wt) == wt / local_config.LOCAL_CONFIG_FILE


def test_outside_a_checkout_it_is_the_roots_own_path(tmp_path):
    assert local_config.local_config_path(tmp_path) == tmp_path / local_config.LOCAL_CONFIG_FILE


def test_every_reader_of_the_local_config_resolves_it_here(tmp_path, monkeypatch):
    """`i18n-translate`, `refimg` and `refscore` read the worktree case alike:
    each finds the `[<section>]` only the main checkout's file holds."""
    main, wt = _repo_with_worktree(tmp_path)
    (main / local_config.LOCAL_CONFIG_FILE).write_text(
        '[i18n]\nbase_url = "https://x.test/v1"\nmodel = "m"\napi_key_env = "K"\n'
        '[refimg]\nprovider = "ideogram-v3"\nmodel = "ideogram-v3"\napi_key_env = "K"\n'
        '[refscore]\nbackend = "stub"\n'
    )
    i18n = _load("i18n_translate_lc", TOOLS / "creator" / "i18n-translate.py")
    assert i18n.load_config(root=wt).model == "m"
    refimg = _load("refimg_lc", TOOLS / "creator" / "refimg.py")
    monkeypatch.setattr(refimg, "repo_root", lambda: wt)
    assert refimg.load_config()["provider"] == "ideogram-v3"
    refscore = _load("refscore_lc", TOOLS / "creator" / "refscore.py")
    monkeypatch.setattr(refscore, "repo_root", lambda: wt)
    assert refscore.load_config()["backend"] == "stub"
