set -u
bash tools/ci/fmt-workspaces.sh --check > r-fmt.log 2>&1; echo "fmt=$?"
cargo clippy --workspace --all-targets -- -D warnings > r-clippy.log 2>&1; echo "clippy=$?"
cargo test -p delvec --lib > r-lib.log 2>&1; echo "lib=$?"; grep "test result" r-lib.log
cargo test -p delvewright-dsl > r-dsl.log 2>&1; echo "dsl=$?"; grep "test result" r-dsl.log | head -5
python3 -m pytest tools/tests/test_check_party_semantics.py -q > r-pytest.log 2>&1; echo "pytest=$?"; tail -2 r-pytest.log
cargo test -p delvec --test it --no-fail-fast > r-it.log 2>&1; echo "it=$?"; grep "^test .*FAILED\|test result" r-it.log | head -80
python3 tools/ci/check-party-semantics.py --delvec target/debug/delvec > r-sem.log 2>&1; echo "semantics=$?"; head -9 r-sem.log
mkdir -p gallery-prefabs
cargo run --locked --manifest-path gallery/prefab-generator/Cargo.toml -- gallery-prefabs --skins gallery/skins --design gallery/design --textures gallery/textures > r-pre.log 2>&1; echo "prefabs=$?"
target/debug/delvec build gallery -o validation/delve-output-gallery --prefabs gallery-prefabs > r-gbuild.log 2>&1; echo "gallery-build=$?"; grep "^absent-member\|^party binding\|packtest solo" r-gbuild.log
python3 tools/ci/check-gallery-coverage.py --delvec target/debug/delvec --prefabs gallery-prefabs --build-out validation/delve-output-gallery --report r-cov.json --index r-cov.md > r-cov.log 2>&1; echo "coverage=$?"; tail -3 r-cov.log
EULA=TRUE bash validation/packtest-run.sh --project dw-multiplayer-audit-gallery --output ./delve-output-gallery > r-packtest.log 2>&1; echo "packtest=$?"; grep -i "passed\|failed\|party_\|collect_" r-packtest.log | tail -40
