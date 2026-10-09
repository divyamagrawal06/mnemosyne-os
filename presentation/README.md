# MnemosyneOS presentation package

Current integrated OS demo: [cloud and guest runbook](../docs/cloud-demo.md). The original deck and saved browser evidence retain their earlier baseline.

Open the [editable PowerPoint](output/mnemosyne-os-pitch.pptx) for presenting and the [PDF copy](output/mnemosyne-os-pitch.pdf) for sharing. The deck has 30 slides: slides 1-22 are the main pitch, and slides 23-30 contain technical backup, review questions, the original proposal, and references. All slides include speaker notes with narration and source links. A separate [speaker-notes document](output/mnemosyne-os-speaker-notes.md) is convenient for rehearsal; the viewing PDF does not include notes.

The [full demo guide](demo-runbook.md) covers preparation, what to say, expected output, manual commands, troubleshooting, and a recorded fallback. Plan about 15-20 minutes for the pitch and 5-7 minutes for the narrated demo. These are presentation estimates, not benchmark results.

## Start the demo

Open PowerShell with a readable terminal font:

```powershell
Set-Location 'C:\Users\ludic\Desktop\mnemosyne'
python tools/present.py
```

Press Enter before each act: kernel boot in QEMU, capability/replay tests, and a host Wasmi checkpoint/replay experiment. The script stops if a command fails. For rehearsal use `--auto`; for host tests and replay without QEMU use `--host-only`; for explicitly labeled saved output use `--recorded`.

The current interface is a CLI and serial output. The kernel and host replay experiment are separate. The deck marks process isolation, ChronicleFS, durable recovery, the AI plan verifier, and the benchmark dashboard as proposed work.

## Contents and provenance

| File or directory | Purpose |
| --- | --- |
| `output/mnemosyne-os-pitch.pptx` | Editable text, tables, artwork, and embedded speaker notes |
| `output/mnemosyne-os-pitch.pdf` | Native Microsoft PowerPoint export for viewing |
| `output/mnemosyne-os-speaker-notes.md` | Slide-by-slide narration and full citations |
| `demo-runbook.md` | Complete live and fallback presentation route |
| `evidence/` | Saved command output and a checksum manifest, tied to code commit `44b739d` |
| `build-deck.mjs` | Source for the slide content and layout |
| `export-powerpoint.ps1` | Optional native PDF/PNG export helper for Windows PowerPoint |
| `assets/` | Two generated concept illustrations and their prompt provenance |

The artwork was generated with the built-in image generation tool. It is conceptual artwork, not a screenshot or experimental evidence. The exact prompts are saved in [assets/provenance.json](assets/provenance.json). Implementation results come from the saved logs and report in `evidence/`. Papers and upstream repositories are cited in the notes; code attribution remains in [THIRD_PARTY.md](../THIRD_PARTY.md).

## Edit or rebuild

Normal edits can be made directly in PowerPoint. The JavaScript source requires the Codex bundled `@oai/artifact-tool` runtime and presentation finalizer. It does not use a public npm package installation. To regenerate on the current machine, choose a new output name:

```powershell
& 'C:\Users\ludic\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe' `
  presentation/build-deck.mjs `
  --runtime-modules 'C:\Users\ludic\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\node_modules' `
  --skill-dir 'C:\Users\ludic\.codex\plugins\cache\openai-primary-runtime\presentations\26.1007.11041\skills\presentations' `
  --python 'C:\Users\ludic\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe' `
  --output mnemosyne-os-pitch-v2.pptx
```

The builder regenerates the separate speaker-notes document from its content source. Intermediate renders and build diagnostics stay under ignored `build/pitch/`. Open and review all regenerated slides before using a revised deck.

With desktop PowerPoint installed, export the revised deck using:

```powershell
.\presentation\export-powerpoint.ps1 `
  -InputPath .\presentation\output\mnemosyne-os-pitch-v2.pptx `
  -PdfPath .\presentation\output\mnemosyne-os-pitch-v2.pdf
```

The helper opens the source read-only, exports each slide as a preview PNG, and creates the PDF. It refuses to overwrite an existing PDF. PowerPoint's ordinary File > Export menu can also create the viewing copy.
