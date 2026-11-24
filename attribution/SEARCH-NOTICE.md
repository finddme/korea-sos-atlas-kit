# Search module origin and modifications

This module is derived from **Please read this for me (pleasereadthisforme)**:
https://github.com/pleasereadthisforme/pleasereadthisforme

License: **AGPL-3.0-only**. This is a manually maintained Korea SOS Atlas Kit derivative, not an automatic upstream mirror. Upstream's own code/license notices and contributor identity are not replaced by a claim that Korea SOS Atlas Kit originally authored the imported implementation.

Observed upstream HEAD: `05b5e51ac4944f9927f8aefc84bc339ab3cdb4a8`.
Port date: 2026-11-14. Inputs were read from the local working files; [search-provenance.json](search-provenance.json) records their actual SHA-256 hashes, without assuming the worktree equals HEAD.

## Imported code and changes

All original Python paths below are under `sidecar/src/prtfm/`.

| Destination under src/sos_atlas/search | Original | Modification |
|---|---|---|
| command_io.py | query/command_io.py | Preserve bounded subprocess transport; rename worker labels |
| command_paths.py | query/command_paths.py | Preserve binary resolution/hash checks; rename manifest environment variable |
| file_commands.py | query/file_commands.py | Remove all optional semantic tool branches; remove DB/cache entry points; adjust imports |
| document_agent.py | query/document_agent.py | Remove semantic schema/guidance/registration; use provider-neutral contracts |
| source_snapshot.py | query/source_snapshot.py | Preserve evidence integrity checks; require explicit source reader |
| sources.py | query/sources.py (Source data class) | Preserve coordinates; make source immutable; add bounded Linux text-folder reader |
| contracts.py | query/agent.py, query/tools.py, llm/openai_client.py, llm/usage.py (selected definitions) | Retain event/usage/fence contracts; exclude provider SDK, pricing and network clients |

New integration files are `search_temp.py`, package entry points, CLI, packaging and tests. The scratch directory uses Python TemporaryDirectory, not the original app's persistent lease/reclamation service. Normal exit cleanup is tested; cleanup after forced process termination is not implemented. The folder reader retains a bounded immutable text snapshot in RAM; it does not include upstream's DB/cache/lazy document access machinery.

The query loop retains upstream operational limits (16 turns, cumulative input budget 40,000, final reserve 10,000) for this port. These are not suitable-by-default context settings for a small model; tuning and per-request context enforcement belong to the next local-adapter step.

No embeddings, vector store, semantic-search provider, cloud SDK, document extraction service or model weights are included. Reading a prepared text folder is implemented; PDF/OCR/ZIM adapters and an actual local inference adapter are not yet implemented. Existing NOMAD source is not imported or modified here.

## Attribution and release

Keep this notice, the original license and provenance when distributing this module. Add the actual release revision and provide the corresponding modified source and build instructions when distributing a runnable product. The offline product's information/license UI is still to be implemented; this documentation is not a claim that future image-release obligations have already been fulfilled.
