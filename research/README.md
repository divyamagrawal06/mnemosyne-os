# Research evidence workspace

The October 2026 research continuation produced:

- [Deep literature and protocol review](C:/Users/ludic/Desktop/mnemosyne/docs/deep-research.md): 16 paper profiles, design decisions, recovery protocol, counterexamples, verification targets, and development gates.
- [Pinned source-reading guide](C:/Users/ludic/Desktop/mnemosyne/docs/source-reading-guide.md): 15 repositories and 49 source entry points.
- [Updated architecture plan](C:/Users/ludic/Desktop/mnemosyne/docs/research-plan.md).
- [Source manifest](C:/Users/ludic/Desktop/mnemosyne/research/source-manifest.json): 87 selected files, immutable GitHub URLs, byte sizes, and SHA-256 digests.

`cache/<owner>__<repo>/inventory.json` records the default-branch commit, retrieval time, and file tree. The corresponding `source/` directory contains selected public files, not a full checkout. Some cache entries also contain a shallow Git metadata checkout used when GitHub's public API was rate limited. A failed discovery attempt can leave an otherwise empty directory; it is not counted as an inspected repository.

No downloaded build, install, benchmark, or infrastructure script was executed. The reports separate source observations and published results from proposed mechanisms. No performance reproduction or proof checking is claimed.

## Collection and source links

The collector pins repository metadata once and reuses that inventory. It does not refresh cached repositories to new main-branch commits automatically.

```powershell
python research/collect_sources.py inventory owner/repository
python research/collect_sources.py files research/source-selection.json
python research/collect_sources.py files research/source-selection-extra.json
python research/collect_sources.py files research/source-selection-os.json
python research/render_source_guide.py
```

Run manifest-writing collection commands sequentially. The renderer checks that each referenced file is in the manifest and that its requested source symbol exists, then computes the line anchor from the cached file. File digests were checked against the manifest after collection.

License texts are retained with the source selections where available. The source guide identifies unresolved project-wide terms and mixed-license repositories. The cache is research material, not a vendored dependency tree or permission to incorporate all of its contents into the project.

Paper links in the deep review point to publisher, author, or arXiv primary sources. Paper profiles summarize selected relevant sections; published results were not independently reproduced. Current repository main branches are distinguished from publication artifacts.
